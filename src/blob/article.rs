//! A web page's article into the blob: [`smoothie`] picks the article out of the page's
//! [`tree`], and the walk here writes it through the same [`BlobWriter`](super::BlobWriter), each
//! tag as the blob writer writes the markdown it stands for. Not [`super::html`], which reads a
//! site's comment HTML in one pass and knows no blocks.

mod smoothie;
mod tree;

use super::html::{collapse, is_space, relative};
use super::*;
use tree::{Id, Tree};

/// The article on the page `html`, at `base_url`, which its relative links and pictures resolve
/// against: its title, a first level heading, centred, its authors as the page's JSON-LD names
/// them on the line under it, centred at the body's size, then its words, every picture the
/// article shows among them; the page's own byline element is no part of it. None where nothing on
/// the page reads as one.
pub fn render_article_blob(html: &str, base_url: &str) -> Option<Vec<u8>> {
    let article = smoothie::extract(html)?;
    let title = collapse(title(&article, base_url));
    let byline = article.byline.as_deref().unwrap_or_default();
    let mut out = BlobWriter::new(
        title.len() + byline.len() + 3 + article.tree.char_count(article.root),
        false,
        sites::READER,
    );
    if !title.is_empty() {
        let start = out.pos();
        out.write_prose(&title);
        out.span(HEADINGS[0], start);
        out.span(CENTERED, start);
        if !byline.is_empty() {
            out.nl(1);
            let start = out.pos();
            out.write_prose(byline);
            out.span(CENTERED, start);
        }
        out.nl(2);
    }
    Walk {
        tree: &article.tree,
        base_url,
        out: &mut out,
        lists: Vec::new(),
        items: Vec::new(),
        block_start: true,
        space: false,
        rule: usize::MAX,
    }
    .walk(article.root);
    Some(out.into_blob())
}

/// The title shown over the article: the page's JSON-LD headline where it reads as the page's own
/// title, else the page's title without the site's name at either end. A headline is a summary on
/// some sites (Wikipedia's), so it is taken only where most of its words are the title's, as
/// Readability.js takes a JSON-LD title; a site's name is its `og:site_name`, its host, or the host's
/// own name, the label before its ending, as `nullpt.rs •`, `| Quanta Magazine`, `CircuitHub -` and
/// `- Wikipedia` (en.wikipedia.org) stand in titles.
fn title<'a>(article: &'a smoothie::Article, base_url: &str) -> &'a str {
    let page = article.title.trim();
    if let Some(headline) = &article.headline
        && similar(page, headline)
    {
        return headline.trim();
    }
    let host = extract_domain(base_url);
    let host = host.as_deref().map(|h| h.strip_prefix("www.").unwrap_or(h));
    let names = [
        article.site.as_deref(),
        host,
        host.and_then(|h| h.rsplit('.').nth(1)),
    ];
    names
        .into_iter()
        .flatten()
        .find_map(|name| without_site(page, name.trim()))
        .unwrap_or(page)
}

/// `title` without `name` at its start or end where a separator stands between them, or none.
fn without_site<'t>(title: &'t str, name: &str) -> Option<&'t str> {
    const SEPARATORS: &[char] = &['|', '-', '–', '—', '•', '·', ':', '»', '/', '\\', '~'];
    let at = title.len().checked_sub(name.len())?;
    let rest = if title.get(at..)?.eq_ignore_ascii_case(name) {
        title[..at].trim_end().strip_suffix(SEPARATORS)
    } else if title.get(..name.len())?.eq_ignore_ascii_case(name) {
        title[name.len()..].trim_start().strip_prefix(SEPARATORS)
    } else {
        None
    }?
    .trim();
    (!name.is_empty() && !rest.is_empty()).then_some(rest)
}

/// Whether most of `b`'s words are `a`'s, as Readability.js measures it: one less the share of
/// `b`'s text in words `a` lacks, above three quarters.
fn similar(a: &str, b: &str) -> bool {
    fn words(s: &str) -> impl Iterator<Item = &str> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
    }
    let (mut total, mut unique) = (0, 0);
    for w in words(b) {
        total += w.len() + 1;
        if !words(a).any(|x| x.eq_ignore_ascii_case(w)) {
            unique += w.len() + 1;
        }
    }
    total > 0 && unique * 4 < total
}

/// What closes when a tag the walk went into ends.
enum Close<'t> {
    /// A block, the newlines after it: where it starts and the spans over it, as a heading's.
    Block(usize, [i32; 2]),
    /// A line of its own, as a table's row.
    Line,
    /// A span or two over its words, from where they start.
    Spans(usize, [i32; 2]),
    /// A link: where its span and its words' bytes start, and its `href`.
    Link(usize, usize, &'t str),
    List,
    Item,
    Quote(usize),
}

struct Walk<'t, 'a, 'o> {
    tree: &'t Tree<'a>,
    base_url: &'o str,
    out: &'o mut BlobWriter,
    /// Each list the walk is inside: its next item's number, 0 for bullets.
    lists: Vec<i32>,
    /// Each item the walk is inside: where it starts and its span's data, until its span is
    /// written, before a list nested in it or at its end.
    items: Vec<Option<(usize, i32)>>,
    /// Nothing of the current block is written yet.
    block_start: bool,
    /// Whitespace read since the last word, written as one space before the next.
    space: bool,
    /// Where the text ended at the last rule written: a rule with nothing written since is none.
    rule: usize,
}

impl<'t, 'a, 'o> Walk<'t, 'a, 'o> {
    /// Writes everything under `root`, in document order, holding the tags still open on a stack
    /// of its own so a deep page costs heap and not call stack.
    fn walk(mut self, root: Id) {
        let t = self.tree;
        let mut open: Vec<(Id, Close)> = Vec::new();
        let mut next = t.first_child(root);
        loop {
            if let Some(n) = next {
                match self.enter(n) {
                    Some(close) => {
                        open.push((n, close));
                        next = t.first_child(n);
                    }
                    None => next = t.next_sibling(n),
                }
                continue;
            }
            let Some((n, close)) = open.pop() else { break };
            self.leave(close);
            next = t.next_sibling(n);
        }
    }

    /// Writes a node's opening, and answers what closes it where its children are to be walked.
    fn enter(&mut self, n: Id) -> Option<Close<'t>> {
        let t = self.tree;
        if let Some(text) = t.text_of(n) {
            self.text(text);
            return None;
        }
        let name = t.name(n)?;
        match name {
            "p" | "div" | "section" | "article" | "main" | "header" | "footer" | "aside"
            | "figure" | "figcaption" | "details" | "summary" | "address" | "dl" | "dt" | "dd"
            | "center" | "nav" | "form" | "fieldset" | "table" => {
                self.end(true);
                Some(Close::Block(self.out.pos(), [-1, -1]))
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.end(true);
                let level = (name.as_bytes()[1] - b'1') as usize;
                Some(Close::Block(self.out.pos(), [HEADINGS[level], -1]))
            }
            "tr" => {
                self.end(false);
                Some(Close::Line)
            }
            "ul" | "ol" => {
                // An item's own words end where a list nested in it begins
                if let Some(item) = self.items.last_mut()
                    && let Some((start, data)) = item.take()
                {
                    self.out.span_data(LIST_ITEM, start, data);
                }
                self.end(true);
                self.out.list_depth += 1;
                let first = t
                    .attr(n, "start")
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(1);
                self.lists.push(if name == "ol" {
                    first.clamp(1, 0xFFFF)
                } else {
                    0
                });
                Some(Close::List)
            }
            "li" => {
                self.end(false);
                let indent = self.lists.len().saturating_sub(1) as i32;
                let number = match self.lists.last_mut() {
                    Some(n) if *n > 0 => {
                        *n += 1;
                        (*n - 1).min(0xFFFF)
                    }
                    _ => 0,
                };
                self.items
                    .push(Some((self.out.pos(), (indent << 16) | number)));
                Some(Close::Item)
            }
            "blockquote" => {
                self.end(true);
                self.out.quote_depth += 1;
                Some(Close::Quote(self.out.pos()))
            }
            "pre" => {
                self.end(true);
                self.pre(n);
                self.end(true);
                None
            }
            "hr" => {
                // Two rules with nothing left between them, a share bar taken out say, are one
                if self.out.len == self.rule {
                    return None;
                }
                self.end(true);
                let start = self.out.pos();
                self.out.write_text("\u{0001}");
                self.out.span(HRULE, start);
                self.rule = self.out.len;
                self.end(true);
                None
            }
            "br" => {
                self.end(false);
                None
            }
            "img" => {
                let (width, height) = size(t, n);
                // Both sides stated at the icon's size or under: a footnote's arrow back up the page,
                // a licence's mark, never the article's picture
                if let Some(src) = self.picture(n)
                    && !(width > 0 && width.max(height) <= MAX_ICON_SIDE)
                {
                    self.end(true);
                    let url = relative(src, self.base_url);
                    self.out.emit_image(
                        &resolve_url(url.as_deref().unwrap_or(src)),
                        width,
                        height,
                        t.attr(n, "alt").unwrap_or(""),
                    );
                    self.block_start = true;
                }
                None
            }
            "td" | "th" => {
                // Cells read across their row until tables are written as tables
                if !self.block_start {
                    self.out.write_text(" | ");
                    self.space = false;
                }
                let spans = if name == "th" { [BOLD, -1] } else { [-1, -1] };
                Some(Close::Spans(self.out.pos(), spans))
            }
            "b" | "strong" => Some(self.style([BOLD, -1])),
            "i" | "em" | "cite" => Some(self.style([ITALIC, -1])),
            "s" | "del" | "strike" => Some(self.style([STRIKETHROUGH, -1])),
            "sup" => Some(self.style([SUPERSCRIPT, SUPERSCRIPT_SIZE])),
            "sub" => Some(self.style([SUBSCRIPT, SUBSCRIPT_SIZE])),
            "code" | "kbd" | "samp" | "tt" => Some(self.style([CODE, -1])),
            "a" => {
                // An anchor on the page, or a script the page no longer has, is words
                let href = t.attr(n, "href").map(str::trim).unwrap_or("");
                if href.is_empty()
                    || href.starts_with('#')
                    || href
                        .get(..11)
                        .is_some_and(|s| s.eq_ignore_ascii_case("javascript:"))
                {
                    return Some(Close::Spans(self.out.pos(), [-1, -1]));
                }
                self.begin();
                Some(Close::Link(self.out.pos(), self.out.blob.len(), href))
            }
            "svg" | "math" | "button" | "input" | "select" | "textarea" | "source" | "video"
            | "audio" | "canvas" | "map" => None,
            _ => Some(Close::Spans(self.out.pos(), [-1, -1])),
        }
    }

    fn leave(&mut self, close: Close) {
        match close {
            Close::Block(start, spans) => {
                self.spans(start, spans);
                self.end(true);
            }
            Close::Line => self.end(false),
            Close::Spans(start, spans) => self.spans(start, spans),
            Close::Link(start, text_start, href) => {
                // A link around a picture is the picture, as markdown's is
                let words = &self.out.blob[text_start..];
                if words.is_empty() || words.contains(&1) {
                    return;
                }
                let url = relative(href, self.base_url);
                self.out.link(
                    start,
                    text_start,
                    &resolve_url(url.as_deref().unwrap_or(href)),
                );
            }
            Close::List => {
                self.lists.pop();
                self.out.list_depth -= 1;
                self.end(true);
            }
            Close::Item => {
                if let Some(Some((start, data))) = self.items.pop() {
                    self.out.span_data(LIST_ITEM, start, data);
                }
                self.end(false);
            }
            Close::Quote(start) => {
                self.out.quote_depth -= 1;
                self.out
                    .span_data(QUOTE, start, self.out.quote_depth as i32);
                self.end(true);
            }
        }
    }

    fn spans(&mut self, start: usize, spans: [i32; 2]) {
        for t in spans {
            if t >= 0 {
                self.out.span(t, start);
            }
        }
    }

    /// Opens a style over the words to come, any space before them outside it.
    fn style(&mut self, spans: [i32; 2]) -> Close<'t> {
        self.begin();
        Close::Spans(self.out.pos(), spans)
    }

    /// Words, their whitespace one space, none at a block's start.
    fn text(&mut self, raw: &str) {
        let words = raw.trim_matches(is_space);
        if words.len() < raw.len() && raw.starts_with(is_space) {
            self.space = true;
        }
        if words.is_empty() {
            return;
        }
        self.begin();
        let words = collapse(words);
        self.out.write_prose(&words);
        self.space = raw.ends_with(is_space);
    }

    /// Before a block's first word: nothing; before any other, the space read since the last.
    fn begin(&mut self) {
        if self.block_start {
            self.block_start = false;
        } else if self.space && self.out.blob.last() != Some(&b' ') {
            self.out.write_text(" ");
        }
        self.space = false;
    }

    /// Ends the line, or with `block` the block: a blank line before whatever follows, outside a
    /// list.
    fn end(&mut self, block: bool) {
        if self.out.pos() > 0 {
            self.out
                .nl(if block && self.items.is_empty() { 2 } else { 1 });
        }
        self.block_start = true;
        self.space = false;
    }

    /// A `<pre>`'s words as written, its `<br>`s newlines, as one code block.
    fn pre(&mut self, n: Id) {
        let t = self.tree;
        let mut code = String::new();
        for d in t.descendants(n) {
            if let Some(text) = t.text_of(d) {
                code.push_str(text);
            } else if t.is(d, "br") {
                code.push('\n');
            }
        }
        // HTML drops the newline straight after `<pre>`
        let code = code.strip_prefix('\n').unwrap_or(&code).trim_end();
        if !code.is_empty() {
            let start = self.out.pos();
            self.out.write_text(code);
            self.out.span(CODE_BLOCK, start);
        }
    }

    /// Where an `<img>` is: its `src`, or the widest of its `srcset`; none for an inline `data:`
    /// picture.
    fn picture(&self, n: Id) -> Option<&'t str> {
        let t = self.tree;
        t.attr(n, "src")
            .map(str::trim)
            .filter(|s| !s.is_empty() && !s.starts_with("data:"))
            .or_else(|| widest(t.attr(n, "srcset")?))
    }
}

/// A side under this many pixels is a lazy page's placeholder, not the picture's.
const MIN_STATED_SIDE: u16 = 16;

/// A picture whose stated sides are both this many pixels or under is an icon.
const MAX_ICON_SIDE: u16 = 32;

/// The size an `<img>` states in its `width` and `height`, else Blogger's `data-original-width`
/// and `data-original-height` beside a display width alone, as pixels with or without `px`, a
/// fraction rounded; 0 by 0 where it states none: a share of the page, `auto` or another unit says
/// nothing of the picture's, a side under `MIN_STATED_SIDE` is a placeholder's, and one side alone
/// has no shape.
fn size(t: &Tree, n: Id) -> (u16, u16) {
    let side = |k| {
        let v = t.attr(n, k)?.trim();
        let v: f32 = v.strip_suffix("px").unwrap_or(v).parse().ok()?;
        (MIN_STATED_SIDE as f32..=u16::MAX as f32)
            .contains(&v)
            .then(|| v.round() as u16)
    };
    let pair = |w, h| side(w).zip(side(h));
    pair("width", "height")
        .or_else(|| pair("data-original-width", "data-original-height"))
        .unwrap_or((0, 0))
}

/// The widest picture a `srcset` offers, by its `w` or `x` descriptor; the first where none has one.
fn widest(srcset: &str) -> Option<&str> {
    let mut best: Option<(f32, &str)> = None;
    let mut rest = srcset;
    loop {
        rest = rest.trim_start_matches(|c: char| c == ',' || c.is_ascii_whitespace());
        if rest.is_empty() {
            break;
        }
        let url_end = rest
            .find(|c: char| c.is_ascii_whitespace())
            .unwrap_or(rest.len());
        let (mut url, tail) = rest.split_at(url_end);
        let descriptor = if let Some(u) = url.strip_suffix(',') {
            url = u;
            rest = tail;
            ""
        } else {
            let end = tail.find(',').unwrap_or(tail.len());
            rest = &tail[end..];
            tail[..end].trim()
        };
        let size = descriptor
            .strip_suffix(['w', 'x'])
            .and_then(|d| d.parse::<f32>().ok())
            .unwrap_or(0.0);
        if !url.is_empty() && !url.starts_with("data:") && best.is_none_or(|(s, _)| size > s) {
            best = Some((size, url));
        }
    }
    best.map(|(_, url)| url)
}
