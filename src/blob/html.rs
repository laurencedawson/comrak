//! A site's comment HTML into the blob, with no markdown between: a walk over the HTML in place of
//! the walk over comrak's tree, writing through the same [`BlobWriter`](super::BlobWriter). Not
//! [`crate::html`], which writes markdown out as HTML.
//!
//! One pass, no tree: each tag is written as the blob writer writes the markdown it stands for, so a
//! comment reads the same from either.

mod mdream;

use super::*;
use crate::image_url::is_image_url;
use crate::parser::autolink::{url_in, www_in};
use crate::parser::url::resolve_url;
use mdream::entities::{decode_html_attribute_entities, decode_html_entities_into};
use mdream::scan::{is_whitespace, process_comment_or_doctype, scan_tag};

/// How a site's HTML reads, as data, so no site's rules live here: each app builds its own.
#[derive(Debug, Clone, Copy, Default)]
pub struct HtmlRules<'r> {
    /// A paragraph opening on `>` is a quote, its marks no text, as Hacker News' posters write one.
    pub quote_marks: bool,
    /// What a tag means, by its name and, where the rule names one, a class it has.
    pub tags: &'r [TagRule<'r>],
    /// Addresses typed among the words are links, `scheme://` and `www.` ones as comrak's autolink
    /// reads them, one broken by a `<wbr>` read whole.
    pub autolink: bool,
    /// The page the HTML sits on, which a relative link resolves against, as a browser would.
    pub base: Option<&'r str>,
    /// The site's pages the app opens itself, as addresses whose `*` stands for one path segment:
    /// a link matching one becomes `app-link://`, the path it matched and its fragment, for the
    /// app's router, the rest of its path dropped.
    pub app_links: &'r [&'r str],
}

/// A tag that means something on a site, as `<span class="quote">` is a quoted line on 4chan.
#[derive(Debug, Clone, Copy)]
pub struct TagRule<'r> {
    /// The tag's name, in any case.
    pub tag: &'r str,
    /// A class the tag must have, or none for every tag of that name.
    pub class: Option<&'r str>,
    /// What its words read as.
    pub reads_as: Reads<'r>,
}

/// What a tag's words read as, each written as the blob writer writes its markdown.
#[derive(Debug, Clone, Copy)]
pub enum Reads<'r> {
    /// A quoted line, its words as written, chevron and all: a quote that opens its own line and
    /// runs on through the quoted lines straight after it, as markdown's `>` lines.
    LineQuote,
    /// Struck through, as markdown's `~~`.
    Strike,
    /// Hidden behind a spoiler under this title, as Lemmy's `::: spoiler` block.
    Spoiler(&'r str),
}

/// Render a site's HTML into a binary blob, read by `rules`.
///
/// Returns `None` if the document has no spans and text is unchanged from input
/// (caller should use the raw input string directly).
pub fn render_html_blob(input: &str, rules: &HtmlRules<'_>) -> Option<Vec<u8>> {
    // ASCII input is only a hint, as in render_blob: a decoded entity or a typographic symbol can
    // write non-ASCII, so one scan of the output catches that and the rare violating document is
    // re-rendered with the scanning writer, every span offset redone.
    let mut b = if input.is_ascii() {
        BlobWriter::new_ascii(input.len())
    } else {
        BlobWriter::new(input.len())
    };
    visit(input, rules, &mut b);
    b.append_footnotes();
    if b.fast_path_ascii && !b.text().is_ascii() {
        b = BlobWriter::new(input.len());
        visit(input, rules, &mut b);
        b.append_footnotes();
    }
    if b.spans.is_empty() && b.text() == input {
        None
    } else {
        Some(b.into_blob())
    }
}

/// A tag open around words still to come.
enum Open<'a> {
    /// Bold, italic or struck: the span it closes with, the tag that closes it, where it starts.
    Style(i32, &'a str, usize),
    /// A link: where its span and its words' bytes start, and its address, borrowed from the
    /// input unless an entity was decoded in it.
    Link(usize, usize, Cow<'a, str>),
    /// A quoted line's tag; the quote itself runs on past it, through the quoted lines after it.
    LineQuote(&'a str),
    /// A spoiler's tag, and where its hidden words start.
    Spoiler(&'a str, usize),
    /// A tag a rule names that means nothing here, held so its end closes nothing else.
    Plain(&'a str),
}

struct Walk<'a, 'o> {
    html: &'a str,
    rules: &'o HtmlRules<'o>,
    out: &'o mut BlobWriter,
    open: Vec<Open<'a>>,
    /// The words between two tags with their entities decoded, one buffer for the whole walk.
    decoded: String,
    /// Nothing of the current paragraph is written yet.
    block_start: bool,
    /// The `>` marks read at the paragraph's start, before its first word.
    marks: usize,
    /// Where the paragraph's quote starts and how many levels it opened.
    quote: Option<(usize, usize)>,
    /// Whitespace read since the last word, written as one space before the next.
    space: bool,
    /// A `<br>` was the last thing read: the next line's leading whitespace is no text.
    broken: bool,
    /// A `<pre>`'s words so far, written whole at its end.
    pre: Option<String>,
    /// Where the quoted lines being read started, until something not quoted is written.
    line_quote: Option<usize>,
}

pub(crate) fn visit(html: &str, rules: &HtmlRules<'_>, out: &mut BlobWriter) {
    let mut w = Walk {
        html,
        rules,
        out,
        open: Vec::new(),
        decoded: String::new(),
        block_start: true,
        marks: 0,
        quote: None,
        space: false,
        broken: false,
        pre: None,
        line_quote: None,
    };
    let bytes = html.as_bytes();
    // Words a `<wbr>` broke, read as the one run they are
    let mut joined = String::new();
    let (mut i, mut from) = (0, 0);
    while let Some(at) = memchr::memchr(b'<', &bytes[from..]).map(|at| from + at) {
        let next = bytes.get(at + 1).copied().unwrap_or(0);
        // A `<` that opens no tag is a word
        if !(next.is_ascii_alphabetic() || next == b'!' || next == b'/' && bytes.get(at + 2).is_some_and(u8::is_ascii_alphabetic)) {
            from = at + 1;
            continue;
        }
        if let Some(past) = wbr(bytes, at) {
            joined.push_str(&html[i..at]);
            (i, from) = (past, past);
            continue;
        }
        w.run(&mut joined, &html[i..at]);
        i = w.tag(at);
        from = i;
    }
    w.run(&mut joined, &html[i..]);
    w.end();
}

/// Where the words after a `<wbr>` at `at` begin, or none for any other tag.
fn wbr(bytes: &[u8], at: usize) -> Option<usize> {
    let name = bytes.get(at + 1..at + 4)?;
    let after = *bytes.get(at + 4)?;
    if !name.eq_ignore_ascii_case(b"wbr") || !(after == b'>' || after == b'/' || is_whitespace(after)) {
        return None;
    }
    Some(at + 4 + memchr::memchr(b'>', &bytes[at + 4..])? + 1)
}

/// The next address typed in `text` from `from`: where it starts and ends, and its link.
fn next_address(text: &str, from: usize) -> Option<(usize, usize, Cow<'_, str>)> {
    let bytes = text.as_bytes();
    let mut at = from;
    while let Some(i) = memchr::memchr2(b':', b'w', &bytes[at..]).map(|i| at + i) {
        if bytes[i] == b':' {
            if let Some((start, end)) = url_in(text, i)
                && start >= from
            {
                return Some((start, end, Cow::Borrowed(&text[start..end])));
            }
        } else if let Some(end) = www_in(text, i) {
            return Some((i, end, Cow::Owned(format!("http://{}", &text[i..end]))));
        }
        at = i + 1;
    }
    None
}

/// HTML's whitespace: what a run of it between words collapses from.
fn is_space(c: char) -> bool {
    c.is_ascii() && is_whitespace(c as u8)
}

/// A run of words with its whitespace one space each, as comrak's `collapse_whitespace` makes a
/// text node's; borrowed when it already is, which one pass decides.
fn collapse(s: &str) -> Cow<'_, str> {
    let bytes = s.as_bytes();
    let clean = !bytes
        .iter()
        .enumerate()
        .any(|(i, &c)| is_whitespace(c) && (c != b' ' || bytes.get(i + 1) == Some(&b' ')));
    if clean {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if !is_space(c) {
            out.push(c);
        } else if !out.ends_with(' ') {
            out.push(' ');
        }
    }
    Cow::Owned(out)
}

impl<'a, 'o> Walk<'a, 'o> {
    /// The words before a tag, with any a `<wbr>` broke off before them.
    fn run(&mut self, joined: &mut String, raw: &str) {
        if joined.is_empty() {
            self.text(raw);
        } else {
            joined.push_str(raw);
            self.text(joined);
            joined.clear();
        }
    }

    /// Reads the tag opening at `at`, and answers where the words after it begin. A tag the input
    /// ends inside is no text, as HTML drops it.
    fn tag(&mut self, at: usize) -> usize {
        let (html, bytes) = (self.html, self.html.as_bytes());
        if bytes[at + 1] == b'!' {
            let comment = process_comment_or_doctype(html, at);
            return if comment.complete { comment.new_position } else { html.len() };
        }
        let closing = bytes[at + 1] == b'/';
        let name_start = at + 1 + closing as usize;
        let name_end = bytes[name_start..]
            .iter()
            .position(|&c| !c.is_ascii_alphanumeric())
            .map_or(bytes.len(), |n| name_start + n);
        let name = &html[name_start..name_end];
        if closing {
            let Some(end) = memchr::memchr(b'>', &bytes[name_end..]) else {
                return html.len();
            };
            self.close(name);
            return name_end + end + 1;
        }
        let (mut href, mut class) = (None, None);
        let scan = scan_tag(html, name_end, &["href", "class"], |name, value| {
            if name.eq_ignore_ascii_case("href") {
                href = Some(value);
            } else {
                class = Some(value);
            }
        });
        if !scan.complete {
            return html.len();
        }
        self.start(name, href, class, scan.new_position)
    }

    /// What the rules say a tag means: the first rule for its name whose class it has, or, where
    /// rules name it but none matches, nothing, held so its end closes nothing else.
    fn rule(&self, name: &str, class: Option<&str>) -> Option<Option<Reads<'o>>> {
        let mut named = false;
        for rule in self.rules.tags {
            if rule.tag.eq_ignore_ascii_case(name) {
                named = true;
                let has = |want: &str| class.is_some_and(|c| c.split(is_space).any(|c| c == want));
                if rule.class.is_none_or(has) {
                    return Some(Some(rule.reads_as));
                }
            }
        }
        named.then_some(None)
    }

    /// Opens a tag, answering where the words after it begin: past the link for a picture.
    fn start(&mut self, name: &'a str, href: Option<&'a str>, class: Option<&str>, after: usize) -> usize {
        let is = |tag: &str| name.eq_ignore_ascii_case(tag);
        if let Some(pre) = &mut self.pre {
            if is("br") {
                pre.push('\n');
            }
            return after;
        }
        if let Some(reads) = self.rule(name, class) {
            match reads {
                Some(Reads::LineQuote) => self.start_line_quote(name),
                Some(Reads::Strike) => {
                    self.begin();
                    self.open.push(Open::Style(STRIKETHROUGH, name, self.out.pos()));
                }
                Some(Reads::Spoiler(title)) => self.start_spoiler(name, title),
                None => self.open.push(Open::Plain(name)),
            }
        } else if is("p") {
            self.end_block();
        } else if is("br") {
            // Each break is a newline, a run of them a blank line at most; none before the first words
            if self.out.len > 0 {
                self.out.nl((self.out.p + 1).min(2));
            }
            self.space = false;
            self.broken = true;
        } else if is("pre") {
            self.end_block();
            self.pre = Some(String::new());
        } else if is("b") || is("strong") || is("i") || is("em") {
            self.begin();
            let span = if is("b") || is("strong") { BOLD } else { ITALIC };
            self.open.push(Open::Style(span, name, self.out.pos()));
        } else if is("a")
            && let Some(href) = href
            && !self.open.iter().any(|o| matches!(o, Open::Link(..)))
        {
            self.begin();
            let href = self.address(decode_html_attribute_entities(href));
            if let Some(past) = self.picture(&href, after) {
                return past;
            }
            self.open.push(Open::Link(self.out.pos(), self.out.blob.len(), href));
        }
        after
    }

    /// Where a link goes: resolved against the page, and the app's own where a rule says so.
    fn address(&self, href: Cow<'a, str>) -> Cow<'a, str> {
        let href = match self.rules.base {
            Some(base) => match relative(&href, base) {
                Some(resolved) => Cow::Owned(resolved),
                None => href,
            },
            None => href,
        };
        match self.rules.app_links.iter().find_map(|pattern| app_link(&href, pattern)) {
            Some(app) => Cow::Owned(app),
            None => href,
        }
    }

    /// A link whose only words are an address, to a picture, is the picture, as comrak's autolink
    /// is. Answers where the words after its `</a>` begin.
    fn picture(&mut self, href: &str, after: usize) -> Option<usize> {
        let bytes = self.html.as_bytes();
        let lt = after + memchr::memchr(b'<', &bytes[after..])?;
        let close = bytes.get(lt..lt + 3)?;
        if !close.eq_ignore_ascii_case(b"</a") {
            return None;
        }
        let raw = &self.html[after..lt];
        let words = if decode_html_entities_into(raw, false, false, &mut self.decoded) { &self.decoded } else { raw };
        if !(words.starts_with("http://") || words.starts_with("https://")) {
            return None;
        }
        let url = resolve_url(href);
        if !is_image_url(&url) {
            return None;
        }
        let gt = lt + memchr::memchr(b'>', &bytes[lt..])?;
        self.out.emit_image(&url);
        Some(gt + 1)
    }

    fn close(&mut self, name: &str) {
        let is = |tag: &str| name.eq_ignore_ascii_case(tag);
        if self.pre.is_some() {
            if is("pre") {
                self.end_pre();
            }
            return;
        }
        if is("p") {
            self.end_block();
        } else if is("a") {
            if let Some(at) = self.open.iter().rposition(|o| matches!(o, Open::Link(..))) {
                let link = self.open.remove(at);
                self.end_open(link);
            }
        } else if let Some(at) = self.open.iter().rposition(|o| match o {
            Open::Style(_, tag, _) | Open::LineQuote(tag) | Open::Spoiler(tag, _) | Open::Plain(tag) => is(tag),
            Open::Link(..) => false,
        }) {
            let open = self.open.remove(at);
            self.end_open(open);
        }
    }

    fn end_open(&mut self, open: Open<'a>) {
        match open {
            Open::Style(span, _, start) => self.out.span(span, start),
            Open::LineQuote(_) | Open::Plain(_) => {}
            Open::Spoiler(_, start) => {
                // The hidden words are a paragraph of their own, the spoiler a block after them
                self.end_block();
                self.out.span(LEMMY_SPOILER_CONTENT, start);
                self.out.nl(2);
            }
            Open::Link(start, text_start, href) => {
                let url = resolve_url(&href);
                self.out.span_url(LINK, start, &url);
                self.out.span(LINK_SIZE, start);
                self.out.append_domain_suffix(text_start, &url);
            }
        }
    }

    /// The words between two tags, their entities decoded and their whitespace one space.
    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        if memchr::memchr(b'&', raw.as_bytes()).is_none() {
            return self.words(raw);
        }
        // Taken out while the words are written, so they can borrow it
        let mut decoded = std::mem::take(&mut self.decoded);
        if decode_html_entities_into(raw, false, false, &mut decoded) {
            self.words(&decoded);
        } else {
            self.words(raw);
        }
        self.decoded = decoded;
    }

    /// Words with their entities decoded, written with their whitespace one space.
    fn words(&mut self, text: &str) {
        if let Some(pre) = &mut self.pre {
            // HTML drops the newline straight after `<pre>`
            let text = if pre.is_empty() { text.strip_prefix('\n').unwrap_or(&text) } else { &text };
            pre.push_str(text);
            return;
        }
        let mut run: &str = &text;
        if self.block_start && self.rules.quote_marks {
            // A paragraph opening on `>` is a quote, the marks no text
            loop {
                run = run.trim_start_matches(is_space);
                let rest = run.trim_start_matches('>');
                if rest.len() == run.len() {
                    break;
                }
                self.marks += run.len() - rest.len();
                run = rest;
            }
        }
        let words = run.trim_matches(is_space);
        if words.len() < run.len() && run.starts_with(is_space) {
            self.space = true;
        }
        if !words.is_empty() {
            self.begin();
            let words = collapse(words);
            if self.rules.autolink && !self.open.iter().any(|o| matches!(o, Open::Link(..))) {
                self.autolinked(&words);
            } else {
                self.out.write_prose(&words);
            }
            self.space = run.ends_with(is_space);
        }
    }

    /// Words with the addresses typed among them linked, each as comrak's autolink writes one: a
    /// picture's address the picture, any other its words linked, the domain after them.
    fn autolinked(&mut self, text: &str) {
        let mut from = 0;
        while let Some((start, end, url)) = next_address(text, from) {
            if start > from {
                self.out.write_prose(&text[from..start]);
            }
            let words = &text[start..end];
            let href = self.address(Cow::Owned(url.into_owned()));
            let url = resolve_url(&href);
            if (words.starts_with("http://") || words.starts_with("https://")) && is_image_url(&url) {
                self.out.emit_image(&url);
            } else {
                let (start, text_start) = (self.out.pos(), self.out.blob.len());
                self.out.write_prose(words);
                self.out.span_url(LINK, start, &url);
                self.out.span(LINK_SIZE, start);
                self.out.append_domain_suffix(text_start, &url);
            }
            from = end;
        }
        if from < text.len() {
            self.out.write_prose(&text[from..]);
        }
    }

    /// A quoted line opens its own line, and the quote runs on from any quoted line just before it.
    fn start_line_quote(&mut self, name: &'a str) {
        if self.line_quote.is_none() {
            if self.out.len > 0 && self.out.p == 0 {
                self.out.nl(1);
            }
            self.block_start = false;
            self.space = false;
            self.broken = false;
            self.out.quote_depth += 1;
            self.line_quote = Some(self.out.pos());
        }
        self.open.push(Open::LineQuote(name));
    }

    /// Ends the quoted lines once anything not quoted is to be written.
    fn end_line_quote(&mut self) {
        if let Some(start) = self.line_quote.take() {
            self.out.quote_depth -= 1;
            self.out.span_data(QUOTE, start, self.out.quote_depth as i32);
        }
    }

    /// A spoiler, as Lemmy's block: its title in bold at the size of a link, then its hidden words.
    fn start_spoiler(&mut self, name: &'a str, title: &str) {
        self.end_block();
        let start = self.out.pos();
        self.out.write_text(title);
        self.out.span(BOLD, start);
        self.out.span(LINK_SIZE, start);
        self.out.span(LEMMY_SPOILER_TITLE, start);
        // The hidden words' span owns one leading newline, so the blank line collapses with it
        self.out.nl(1);
        self.open.push(Open::Spoiler(name, self.out.pos()));
        self.out.nl(2);
    }

    /// Before anything of a paragraph is written: opens its quote, or writes the space before it.
    fn begin(&mut self) {
        if self.line_quote.is_some() && !self.open.iter().any(|o| matches!(o, Open::LineQuote(_))) {
            self.end_line_quote();
        }
        if self.block_start {
            self.block_start = false;
            if self.marks > 0 {
                self.out.quote_depth += self.marks;
                self.quote = Some((self.out.pos(), self.marks));
            }
        } else if self.space && !self.broken {
            self.out.write_text(" ");
        }
        self.space = false;
        self.broken = false;
    }

    /// Ends the paragraph: closes its quote, and leaves a blank line before whatever follows.
    fn end_block(&mut self) {
        self.end_line_quote();
        if let Some((start, levels)) = self.quote.take() {
            // Innermost first, as comrak closes them
            for _ in 0..levels {
                self.out.quote_depth -= 1;
                self.out.span_data(QUOTE, start, self.out.quote_depth as i32);
            }
        }
        if self.out.pos() > 0 {
            self.out.nl(2);
        }
        self.block_start = true;
        self.marks = 0;
        self.space = false;
    }

    fn end_pre(&mut self) {
        let pre = self.pre.take().unwrap_or_default();
        let code = pre.trim_end();
        if !code.is_empty() {
            let start = self.out.pos();
            self.out.write_text(code);
            self.out.span(CODE_BLOCK, start);
        }
        self.end_block();
    }

    /// Closes whatever the input left open, as HTML does at its end.
    fn end(mut self) {
        if self.pre.is_some() {
            self.end_pre();
        }
        while let Some(open) = self.open.pop() {
            self.end_open(open);
        }
        self.end_block();
    }
}

/// `href` resolved against `base` where it is relative, as a browser resolves it; none where it
/// already names its scheme.
fn relative(href: &str, base: &str) -> Option<String> {
    let scheme_end = |s: &str| {
        let colon = s.find(':')?;
        let scheme = &s[..colon];
        (!scheme.is_empty() && scheme.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.')))
            .then_some(colon)
    };
    if scheme_end(href).is_some() {
        return None;
    }
    let colon = scheme_end(base)?;
    if let Some(rest) = href.strip_prefix("//") {
        return Some(format!("{}//{rest}", &base[..=colon]));
    }
    let authority = colon + 3;
    let host_end = base[authority..].find(['/', '?', '#']).map_or(base.len(), |i| authority + i);
    if href.starts_with('/') {
        return Some(format!("{}{href}", &base[..host_end]));
    }
    let cut = |at: &[char]| base.find(at).unwrap_or(base.len());
    if href.starts_with('#') {
        return Some(format!("{}{href}", &base[..cut(&['#'])]));
    }
    if href.starts_with('?') {
        return Some(format!("{}{href}", &base[..cut(&['?', '#'])]));
    }
    let path = &base[..cut(&['?', '#'])];
    let dir = path[host_end..].rfind('/').map_or(host_end, |i| host_end + i);
    Some(format!("{}/{href}", &path[..dir]))
}

/// `url` as the app's own link where it matches `pattern`, each `*` one path segment: the path it
/// matched, after the host, and the url's fragment.
fn app_link(url: &str, pattern: &str) -> Option<String> {
    let (mut at, mut pieces) = (0, pattern.split('*').peekable());
    while let Some(piece) = pieces.next() {
        if !url[at..].starts_with(piece) {
            return None;
        }
        at += piece.len();
        if pieces.peek().is_some() {
            let segment = url[at..].find(['/', '?', '#']).unwrap_or(url.len() - at);
            if segment == 0 {
                return None;
            }
            at += segment;
        }
    }
    let host = url.find("://")? + 3;
    let path_start = host + url[host..].find('/')? + 1;
    let fragment = url[at..].find('#').map_or("", |i| &url[at + i..]);
    Some(format!("app-link://{}{fragment}", &url[path_start..at]))
}
