//! Tests for the blob renderer's walk over HTML (`comrak::blob::render_html_blob`): Hacker News'
//! comment HTML as the blob.
//!
//! Every expectation is the blob for the same comment written as markdown, read off `render_blob`
//! on 22 Oct 2026, and `parity` parses each of a table of comments both ways and asserts the two
//! read the same, so the walks cannot drift apart. The plain-text gate is syncdown's, tested there.
//!
//! - `nothing`    : no blob for unchanged text, an empty one for HTML with no words
//! - `paragraphs` : a bare `<p>` opens every paragraph but the first; whitespace collapses; a `<br>`
//!   is a newline, a run of them a blank line at most
//! - `entities`   : decoded after the tags are read, in UTF-16 positions
//! - `quotes`     : a paragraph opening on `>` is a quote, as the site's posters write one
//! - `links`      : the address resolved, the domain added where the words lack it, pictures
//! - `emphasis`   : italic and bold
//! - `code`       : preformatted code kept as written
//! - `words`      : markdown's characters, tags no site rule reads, comments, typography
//! - `tags`       : what a site's rules make a tag: greentext, a strike, a spoiler
//! - `site links` : relative links resolved against the page, the app's own pages its links
//! - `typed addresses` : addresses among the words linked, as comrak's autolink links them
//! - `whole`      : a real comment
//! - `parity`     : the same comments as markdown and as HTML, read the same

use crate::benchmarks::production_options;
use crate::blob::sites;
use crate::blob::{
    APP_LINK, BOLD, CODE_BLOCK, GREENTEXT, IMAGE, ITALIC, LEMMY_SPOILER_CONTENT, LEMMY_SPOILER_TITLE, LINK,
    LINK_SIZE, QUOTE, STRIKETHROUGH, render_blob, render_html_blob,
};
use crate::parse_document_zerocopy;

/// The thread page 4chan's comments here sit on, which their relative links resolve against.
const THREAD: &str = "https://boards.4chan.org/g/thread/100";

/// The walk under test, by the name the tests were written against, read as Hacker News.
fn parse(html: &str) -> Option<Vec<u8>> {
    render_html_blob(html, sites::HACKER_NEWS, None)
}

/// The comment's text and spans, read as 4chan on its thread.
fn chan(html: &str) -> (String, Vec<Span>) {
    let blob = render_html_blob(html, sites::CHAN, Some(THREAD));
    let b = read(&blob.unwrap_or_else(|| panic!("no blob for {html:?}")));
    (b.text, b.spans)
}

// ── helpers ──────────────────────────────────────────────────────────────

/// A span as the blob holds it; a link's or picture's data is its address.
#[derive(Debug, PartialEq)]
enum Data {
    Int(i32),
    Url(String),
}

#[derive(Debug, PartialEq)]
struct Span(i32, i32, i32, Data);

fn span(start: i32, end: i32, t: i32) -> Span {
    Span(start, end, t, Data::Int(0))
}

fn quote(start: i32, end: i32, depth: i32) -> Span {
    Span(start, end, QUOTE, Data::Int(depth))
}

fn url(start: i32, end: i32, t: i32, url: &str) -> Span {
    Span(start, end, t, Data::Url(url.to_string()))
}

/// A link's two spans, as comrak writes them.
fn link(start: i32, end: i32, address: &str) -> [Span; 2] {
    [url(start, end, LINK, address), span(start, end, LINK_SIZE)]
}

struct Blob {
    text: String,
    ascii: bool,
    spans: Vec<Span>,
}

/// Reads a blob back: the header, the text, the spans past the padding and their addresses.
fn read(blob: &[u8]) -> Blob {
    let int = |at: usize| i32::from_le_bytes(blob[at..at + 4].try_into().unwrap());
    let text_len = int(0) as usize;
    let header = int(4) as u32;
    let count = (header & 0xFF_FFFF) as usize;
    let text = std::str::from_utf8(&blob[8..8 + text_len]).unwrap().to_string();
    let spans_at = 8 + text_len + (4 - text_len % 4) % 4;
    let urls = &blob[spans_at + count * 16..];
    let spans = (0..count)
        .map(|i| {
            let at = spans_at + i * 16;
            let (start, end, t, data) = (int(at), int(at + 4), int(at + 8), int(at + 12));
            let data = if t == LINK || t == APP_LINK || t == IMAGE {
                let (offset, len) = ((data >> 12) as usize, (data & 0xFFF) as usize);
                Data::Url(std::str::from_utf8(&urls[offset..offset + len]).unwrap().to_string())
            } else {
                Data::Int(data)
            };
            Span(start, end, t, data)
        })
        .collect();
    Blob {
        text,
        ascii: header >> 24 & 1 == 1,
        spans,
    }
}

/// The comment's blob, which must exist.
fn blob(html: &str) -> Blob {
    read(&parse(html).unwrap_or_else(|| panic!("no blob for {html:?}")))
}

/// The comment's text, with no spans.
fn plain(html: &str) -> String {
    let b = blob(html);
    assert_eq!(b.spans, vec![], "spans for {html:?}");
    b.text
}

/// The comment's text and spans, in the blob's order: start ascending, end descending.
fn spanned(html: &str) -> (String, Vec<Span>) {
    let b = blob(html);
    (b.text, b.spans)
}

// ── nothing ──────────────────────────────────────────────────────────────

/// No text, no blob.
#[test]
fn nothing_empty_is_none() {
    assert!(parse("").is_none());
}

/// A `<` that opens no tag is a word, and words read back unchanged need no blob.
#[test]
fn nothing_unchanged_text_is_none() {
    assert!(parse("a < b").is_none());
}

/// HTML with no words is a blob of no text, never `None`, which would show its tags.
#[test]
fn nothing_no_words_is_empty_text() {
    assert_eq!(plain("<p>"), "");
}

// ── paragraphs ───────────────────────────────────────────────────────────

/// The site opens each paragraph after the first with a bare `<p>` and closes none.
#[test]
fn p_opens_a_paragraph() {
    assert_eq!(plain("one<p>two<p>three"), "one\n\ntwo\n\nthree");
}

/// A comment opening on a `<p>`, as one starting with code does, has no gap before it.
#[test]
fn p_at_the_start_adds_nothing() {
    assert_eq!(plain("<p>one"), "one");
}

/// Two `<p>` with nothing between are one gap.
#[test]
fn p_empty_paragraph_adds_no_second_gap() {
    assert_eq!(plain("one<p><p>two"), "one\n\ntwo");
}

/// A closing `</p>`, which the site does not write but HTML allows, ends nothing more.
#[test]
fn p_closed_is_the_same_paragraph() {
    assert_eq!(plain("<p>one</p><p>two</p>"), "one\n\ntwo");
}

/// A newline in the HTML is a space between words, as markdown's soft break is.
#[test]
fn whitespace_newline_is_a_space() {
    assert_eq!(plain("a\nb"), "a b");
}

/// A run of spaces and newlines is one space.
#[test]
fn whitespace_runs_collapse() {
    assert_eq!(plain("a  \n  b<p>c"), "a b\n\nc");
}

/// Space at a paragraph's start or end is no text.
#[test]
fn whitespace_at_a_paragraph_edge_is_dropped() {
    assert_eq!(plain("<p> one <p> two "), "one\n\ntwo");
}

/// A `<br>` is a newline within the paragraph.
#[test]
fn br_is_a_newline() {
    assert_eq!(plain("one<br>two"), "one\ntwo");
}

/// Two breaks are a blank line, and more are no more than one: a site breaks lines with the tag.
#[test]
fn br_run_is_a_blank_line_at_most() {
    assert_eq!(plain("one<br><br>two"), "one\n\ntwo");
    assert_eq!(plain("one<br><br><br><br>two"), "one\n\ntwo");
}

/// A newline some boards write after a break tag is the next line's leading whitespace, no text.
#[test]
fn br_raw_newline_after_it_is_dropped() {
    assert_eq!(plain("one<br>\ntwo"), "one\ntwo");
    assert_eq!(plain("one <br> two"), "one\ntwo");
}

/// Breaks before the first words open nothing.
#[test]
fn br_before_any_words_is_dropped() {
    assert_eq!(plain("<br><br>one"), "one");
}

/// A word-break hint inside a long word is no character.
#[test]
fn wbr_joins_the_words() {
    assert_eq!(plain("long<wbr>word<wbr/>s"), "longwords");
}

// ── entities ─────────────────────────────────────────────────────────────

/// The site writes an apostrophe and a slash as hex entities.
#[test]
fn entities_hex_are_decoded() {
    assert_eq!(plain("I didn&#x27;t &#x2F; won&#x27;t"), "I didn't / won't");
}

/// The named entities read as their characters, a `<` among them staying a word.
#[test]
fn entities_named_are_decoded() {
    assert_eq!(plain("&quot;a&quot; &amp; b &lt; c"), "\"a\" & b < c");
}

/// An escaped tag is words, since entities are decoded after the tags are read.
#[test]
fn entities_escaped_tag_stays_words() {
    assert_eq!(plain("&lt;i&gt;not italic&lt;/i&gt;"), "<i>not italic</i>");
}

/// An entity past the basic plane is two UTF-16 units, so a span after it starts at 3, and the
/// text is flagged as not ASCII.
#[test]
fn entities_past_the_basic_plane_count_two_units() {
    let b = blob("&#x1F600; <i>x</i>");
    assert_eq!(b.text, "😀 x");
    assert_eq!(b.spans, vec![span(3, 4, ITALIC)]);
    assert!(!b.ascii);
}

/// ASCII text is flagged so, letting the reader skip its own scan.
#[test]
fn entities_ascii_text_is_flagged() {
    assert!(blob("I didn&#x27;t").ascii);
}

// ── quotes ───────────────────────────────────────────────────────────────

/// A paragraph opening on `>` is a quote, its mark and the space after it no text.
#[test]
fn quote_paragraph_opening_on_a_mark() {
    assert_eq!(
        spanned("&gt; On October 17, 2026"),
        ("On October 17, 2026".to_string(), vec![quote(0, 19, 0)])
    );
}

/// The quote is its paragraph alone.
#[test]
fn quote_is_its_paragraph_alone() {
    assert_eq!(
        spanned("before<p>&gt; quoted<p>after"),
        ("before\n\nquoted\n\nafter".to_string(), vec![quote(8, 14, 0)])
    );
}

/// Two quoted paragraphs are two quotes.
#[test]
fn quote_each_paragraph_its_own() {
    assert_eq!(
        spanned("&gt; one<p>&gt; two"),
        ("one\n\ntwo".to_string(), vec![quote(0, 3, 0), quote(5, 8, 0)])
    );
}

/// The mark needs no space after it.
#[test]
fn quote_mark_without_a_space() {
    assert_eq!(spanned("&gt;implying"), ("implying".to_string(), vec![quote(0, 8, 0)]));
}

/// Two marks are a quote within a quote, the inner one a level deeper.
#[test]
fn quote_two_marks_nest() {
    assert_eq!(
        spanned("&gt;&gt; nested"),
        ("nested".to_string(), vec![quote(0, 6, 1), quote(0, 6, 0)])
    );
}

/// Marks with a space between them nest as two marks together do.
#[test]
fn quote_spaced_marks_nest() {
    assert_eq!(
        spanned("&gt; &gt; nested"),
        ("nested".to_string(), vec![quote(0, 6, 1), quote(0, 6, 0)])
    );
}

/// Space before the mark is HTML's, collapsed and meaning nothing, so the paragraph is still a
/// quote however much of it there is.
#[test]
fn quote_space_before_the_mark() {
    assert_eq!(spanned("    &gt; x"), ("x".to_string(), vec![quote(0, 1, 0)]));
}

/// A mark opening a code block's line is code, never a quote.
#[test]
fn quote_mark_in_code_is_code() {
    assert_eq!(
        spanned("<pre><code>  &gt; quoted</code></pre>"),
        ("  > quoted".to_string(), vec![span(0, 10, CODE_BLOCK)])
    );
}

/// A mark inside a paragraph is a word.
#[test]
fn quote_mark_mid_paragraph_is_a_word() {
    assert_eq!(plain("a &gt; b"), "a > b");
}

/// A quote keeps the spans inside it.
#[test]
fn quote_holds_its_emphasis() {
    assert_eq!(
        spanned("&gt; a <i>b</i>"),
        ("a b".to_string(), vec![quote(0, 3, 0), span(2, 3, ITALIC)])
    );
}

// ── links ────────────────────────────────────────────────────────────────

/// The address is the site's, its slashes written as entities in the attribute, and words that
/// name the domain get no suffix.
#[test]
fn a_keeps_its_address_decoded() {
    let html = "<a href=\"https:&#x2F;&#x2F;github.com&#x2F;mitmproxy&#x2F;mitmproxy&#x2F;pull&#x2F;8503\" rel=\"nofollow\">\
                https:&#x2F;&#x2F;github.com&#x2F;mitmproxy&#x2F;mitmproxy&#x2F;pull&#x2F;8503</a> adds the AAA root path.";
    assert_eq!(
        spanned(html),
        (
            "https://github.com/mitmproxy/mitmproxy/pull/8503 adds the AAA root path.".to_string(),
            link(0, 48, "https://github.com/mitmproxy/mitmproxy/pull/8503").into()
        )
    );
}

/// The site shortens a long address's words with an ellipsis; the link still goes to the whole.
#[test]
fn a_shortened_words_keep_the_whole_address() {
    let html = "<a href=\"https:&#x2F;&#x2F;en.wikipedia.org&#x2F;wiki&#x2F;EMudhra#2026_compromise\">\
                https:&#x2F;&#x2F;en.wikipedia.org&#x2F;wiki&#x2F;EMudhra#2026_comp...</a>";
    assert_eq!(
        spanned(html),
        (
            "https://en.wikipedia.org/wiki/EMudhra#2026_comp...".to_string(),
            link(0, 50, "https://en.wikipedia.org/wiki/EMudhra#2026_compromise").into()
        )
    );
}

/// Words that don't name the domain are followed by it.
#[test]
fn a_words_get_the_domain() {
    assert_eq!(
        spanned("<a href=\"https://boards.4chan.org/g/thread/200#p201\">post</a>"),
        (
            "post (boards.4chan.org)".to_string(),
            link(0, 4, "https://boards.4chan.org/g/thread/200#p201").into()
        )
    );
}

/// The domain loses its `www.`, and the spaces around the link stay outside it.
#[test]
fn a_mid_sentence() {
    assert_eq!(
        spanned("see <a href=\"https://www.example.com/a\">the docs</a> now"),
        ("see the docs (example.com) now".to_string(), link(4, 12, "https://www.example.com/a").into())
    );
}

/// The address is resolved as comrak resolves one: a short YouTube link is the watch page.
#[test]
fn a_address_is_resolved() {
    assert_eq!(
        spanned("<a href=\"https://youtu.be/abc\">https://youtu.be/abc</a>"),
        (
            "https://youtu.be/abc (youtube.com)".to_string(),
            link(0, 20, "https://www.youtube.com/watch?v=abc").into()
        )
    );
}

/// An address in single quotes reads the same.
#[test]
fn a_single_quoted_address() {
    assert_eq!(
        spanned("<a href='https://example.com/'>x</a>"),
        ("x (example.com)".to_string(), link(0, 1, "https://example.com/").into())
    );
}

/// An anchor with no address is its words.
#[test]
fn a_without_an_address_is_words() {
    assert_eq!(plain("<a name=\"top\">x</a> y"), "x y");
}

/// A link whose words are its own address, to a picture, is the picture, as comrak's autolink.
#[test]
fn a_to_a_picture_named_by_its_address_is_the_picture() {
    assert_eq!(
        spanned("<a href=\"https://i.imgur.com/abc.png\">https://i.imgur.com/abc.png</a>"),
        ("\u{1}".to_string(), vec![url(0, 1, IMAGE, "https://i.imgur.com/abc.png")])
    );
}

/// The picture stands apart from the words around it.
#[test]
fn a_to_a_picture_mid_sentence() {
    assert_eq!(
        spanned("look <a href=\"https://i.imgur.com/abc.png\">https://i.imgur.com/abc.png</a> here"),
        ("look \n\n\u{1}\n\n here".to_string(), vec![url(7, 8, IMAGE, "https://i.imgur.com/abc.png")])
    );
}

/// A link to a picture with words of its own stays a link.
#[test]
fn a_to_a_picture_with_words_is_a_link() {
    assert_eq!(
        spanned("<a href=\"https://i.imgur.com/abc.png\">a pic</a>"),
        ("a pic (i.imgur.com)".to_string(), link(0, 5, "https://i.imgur.com/abc.png").into())
    );
}

// ── emphasis ─────────────────────────────────────────────────────────────

/// The site's one emphasis is italics.
#[test]
fn i_is_italic() {
    assert_eq!(
        spanned("i <i>havent</i> updated"),
        ("i havent updated".to_string(), vec![span(2, 8, ITALIC)])
    );
}

/// `<em>` is italic too.
#[test]
fn em_is_italic() {
    assert_eq!(spanned("<em>x</em>"), ("x".to_string(), vec![span(0, 1, ITALIC)]));
}

/// `<b>` and `<strong>` are bold.
#[test]
fn b_and_strong_are_bold() {
    assert_eq!(
        spanned("a <b>bold</b> <strong>b</strong>"),
        ("a bold b".to_string(), vec![span(2, 6, BOLD), span(7, 8, BOLD)])
    );
}

/// A tag's name reads in any case.
#[test]
fn tags_read_in_any_case() {
    assert_eq!(spanned("<I>x</I>"), ("x".to_string(), vec![span(0, 1, ITALIC)]));
}

// ── code ─────────────────────────────────────────────────────────────────

/// Code keeps its lines and their indent, a paragraph's gap before it.
#[test]
fn pre_keeps_its_lines() {
    assert_eq!(
        spanned("logs:<p><pre><code>  Audit Logs\n  Oct 21, 2026</code></pre>"),
        ("logs:\n\n  Audit Logs\n  Oct 21, 2026".to_string(), vec![span(7, 34, CODE_BLOCK)])
    );
}

/// The block's own last newline is no text, and the words after it are a new paragraph.
#[test]
fn pre_ending_on_a_newline() {
    assert_eq!(
        spanned("<pre><code>  x = 1\n</code></pre>\nafter"),
        ("  x = 1\n\nafter".to_string(), vec![span(0, 7, CODE_BLOCK)])
    );
}

/// Markdown's characters in code are code.
#[test]
fn pre_markdown_characters_are_code() {
    assert_eq!(
        spanned("<pre><code>  a *b* [c]</code></pre>"),
        ("  a *b* [c]".to_string(), vec![span(0, 11, CODE_BLOCK)])
    );
}

/// Entities in code are decoded.
#[test]
fn pre_entities_are_decoded() {
    assert_eq!(
        spanned("<pre><code>a &lt; b</code></pre>"),
        ("a < b".to_string(), vec![span(0, 5, CODE_BLOCK)])
    );
}

/// Code is as written: no typographic symbols, no collapsed spaces.
#[test]
fn pre_has_no_typography() {
    assert_eq!(
        spanned("<pre><code>(c)  +-</code></pre>"),
        ("(c)  +-".to_string(), vec![span(0, 7, CODE_BLOCK)])
    );
}

// ── words ────────────────────────────────────────────────────────────────

/// Markdown's characters are words: nothing reads them, so none is escaped.
#[test]
fn words_markdown_characters_stay() {
    assert_eq!(
        spanned("<i>x</i> a *b* _c_ `d` ~e"),
        ("x a *b* _c_ `d` ~e".to_string(), vec![span(0, 1, ITALIC)])
    );
}

/// Tags no rule reads, 4chan's greentext span and strike among them, leave their words.
#[test]
fn words_unread_tags_keep_their_words() {
    assert_eq!(plain("<span class=\"quote\">a</span> <s>b</s>"), "a b");
}

/// A comment is no text.
#[test]
fn words_comments_are_dropped() {
    assert_eq!(plain("a<!-- hidden -->b"), "ab");
}

/// The four symbol codes are their symbols, as comrak's blob writes them.
#[test]
fn words_typographic_symbols() {
    let b = blob("(c) 2026 +- 1<p>x");
    assert_eq!(b.text, "\u{a9} 2026 \u{b1} 1\n\nx");
    assert!(!b.ascii);
}

/// Typed curly quotes and dashes are their ASCII forms.
#[test]
fn words_prefer_ascii() {
    assert_eq!(plain("it&#x2019;s &#x201C;quoted&#x201D; &#x2014; dash&#x2026;"), "it's \"quoted\" - dash...");
}

// ── tags ─────────────────────────────────────────────────────────────────

/// A tag with a class among others still matches.
#[test]
fn tags_class_among_others_matches() {
    assert_eq!(
        chan("<span class=\"other quote\">&gt;a</span>"),
        (">a".to_string(), vec![span(0, 2, GREENTEXT)])
    );
}

/// Greentext is its words, chevron and all, under one span.
#[test]
fn tags_greentext_covers_its_words() {
    assert_eq!(
        chan("<span class=\"quote\">&gt;implying</span>"),
        (">implying".to_string(), vec![span(0, 9, GREENTEXT)])
    );
}

/// Each greentext line is its own span; the poster's own line after it is not green.
#[test]
fn tags_greentext_line_by_line() {
    assert_eq!(
        chan("<span class=\"quote\">&gt;a</span><br><span class=\"quote\">&gt;b</span><br>reply"),
        (">a\n>b\nreply".to_string(), vec![span(0, 2, GREENTEXT), span(3, 5, GREENTEXT)])
    );
}

/// Greentext after words on the same line opens its own line.
#[test]
fn tags_greentext_opens_its_own_line() {
    assert_eq!(
        chan("x<span class=\"quote\">&gt;y</span>"),
        ("x\n>y".to_string(), vec![span(2, 4, GREENTEXT)])
    );
}

/// A quotelink inside greentext keeps its link: wider, the greentext sorts first and the link's
/// colour is drawn over it.
#[test]
fn tags_greentext_holds_a_link() {
    let (text, spans) = chan("<span class=\"quote\">&gt;see <a href=\"#p5\">&gt;&gt;5</a></span>");
    assert_eq!(text, ">see >>5");
    assert_eq!(spans[0], span(0, 8, GREENTEXT));
    assert!(spans.iter().any(|s| s.2 == APP_LINK && s.0 == 5));
}

/// A dead link is struck through, its chevrons words.
#[test]
fn tags_strike() {
    assert_eq!(
        chan("<span class=\"deadlink\">&gt;&gt;123</span>"),
        (">>123".to_string(), vec![span(0, 5, STRIKETHROUGH)])
    );
}

/// Without the quote marks rule, a paragraph opening on `>` is words.
#[test]
fn tags_no_quote_marks_leaves_chevrons_as_words() {
    assert_eq!(chan("&gt;&gt;123 agreed"), (">>123 agreed".to_string(), vec![]));
}

/// A spoiler is Lemmy's block: its title bold at a link's size, then its hidden words, each a
/// paragraph of its own.
#[test]
fn tags_spoiler_is_a_spoiler_block() {
    assert_eq!(
        chan("a <s>b</s> c"),
        (
            "a\n\nSpoiler\n\nb\n\nc".to_string(),
            vec![
                span(3, 10, BOLD),
                span(3, 10, LINK_SIZE),
                span(3, 10, LEMMY_SPOILER_TITLE),
                span(11, 13, LEMMY_SPOILER_CONTENT),
            ]
        )
    );
}

/// A tag the rules name, of a class they do not, is words, and its end closes nothing else.
#[test]
fn tags_unmatched_class_closes_nothing_else() {
    assert_eq!(
        chan("<span class=\"deadlink\">a<span class=\"other\">b</span>c</span>"),
        ("abc".to_string(), vec![span(0, 3, STRIKETHROUGH)])
    );
}

// ── site links ───────────────────────────────────────────────────────────

/// A link's words and its one address, read by 4chan's rules.
fn chan_link(html: &str) -> (String, String) {
    let (text, spans) = chan(html);
    let links: Vec<String> = spans
        .into_iter()
        .filter_map(|s| match s {
            Span(_, _, t, Data::Url(url)) if t == LINK || t == APP_LINK => Some(url),
            _ => None,
        })
        .collect();
    assert_eq!(links.len(), 1, "one link in {html:?}");
    (text, links[0].clone())
}

/// Whether the comment's one link is the site's own, an `APP_LINK`, or a `LINK`.
fn own(html: &str) -> bool {
    let (_, spans) = chan(html);
    let kinds: Vec<i32> = spans.iter().map(|s| s.2).filter(|&t| t == LINK || t == APP_LINK).collect();
    assert_eq!(kinds.len(), 1, "one link in {html:?}");
    kinds[0] == APP_LINK
}

/// The site's own links are APP_LINKs, its other pages and other sites LINKs, the address the same.
#[test]
fn site_link_own_pages_are_app_links() {
    assert!(own("<a href=\"#p123\" class=\"quotelink\">&gt;&gt;123</a>"));
    assert!(own("<a href=\"//boards.4channel.org/po/thread/570368#p570370\">x</a>"));
    assert!(own("see https://boards.4chan.org/g/thread/123#p456 now"));
    assert!(!own("<a href=\"//boards.4chan.org/po/archive\">x</a>"));
    assert!(!own("<a href=\"https://example.com/g/thread/1\">x</a>"));
}

/// A quotelink's anchor is on the thread's page, so it is the app's link to that post.
#[test]
fn site_link_anchor_on_the_page_is_the_apps() {
    assert_eq!(
        chan_link("<a href=\"#p123\" class=\"quotelink\">&gt;&gt;123</a>"),
        (">>123".to_string(), "https://boards.4chan.org/g/thread/100#p123".to_string())
    );
}

/// A path on the site resolves against the page's host.
#[test]
fn site_link_path_resolves_against_the_host() {
    assert_eq!(
        chan_link("<a href=\"/g/thread/200#p201\" class=\"quotelink\">&gt;&gt;201</a>"),
        (">>201".to_string(), "https://boards.4chan.org/g/thread/200#p201".to_string())
    );
}

/// An address with no scheme takes the page's.
#[test]
fn site_link_without_a_scheme_takes_the_pages() {
    assert_eq!(
        chan_link("<a href=\"//boards.4chan.org/v/thread/300#p301\" class=\"quotelink\">&gt;&gt;&gt;/v/301</a>"),
        (">>>/v/301".to_string(), "https://boards.4chan.org/v/thread/300#p301".to_string())
    );
    assert_eq!(
        chan_link("<a href=\"//example.com/g/thread/1\">x</a>"),
        ("x (example.com)".to_string(), "https://example.com/g/thread/1".to_string())
    );
}

/// A post's page written in full is the app's all the same, the title after its number dropped.
#[test]
fn site_link_in_full_is_the_apps() {
    assert_eq!(
        chan_link("<a href=\"https://boards.4chan.org/g/thread/200#p201\">here</a>"),
        ("here".to_string(), "https://boards.4chan.org/g/thread/200#p201".to_string())
    );
    assert_eq!(
        chan_link("<a href=\"https://boards.4chan.org/g/thread/200/a-title#p201\">here</a>"),
        ("here".to_string(), "https://boards.4chan.org/g/thread/200/a-title#p201".to_string())
    );
}

/// A board's index and its catalog, a search after `#`, are the app's, as a thread is: where a
/// `>>>/po/` or `>>>/po/lft` quotelink goes.
#[test]
fn site_link_boards_and_catalogs_are_the_apps() {
    assert!(own("<a href=\"/wsr/\">board</a>"));
    assert!(own("<a href=\"//boards.4chan.org/po/catalog#s=lft\" class=\"quotelink\">&gt;&gt;&gt;/po/lft</a>"));
    assert_eq!(
        chan_link("<a href=\"//boards.4chan.org/po/catalog#s=lft\" class=\"quotelink\">&gt;&gt;&gt;/po/lft</a>"),
        (">>>/po/lft".to_string(), "https://boards.4chan.org/po/catalog#s=lft".to_string())
    );
}

/// A page no rule names stays an address on the site, with no domain after its words: the site's
/// reader knows where they are.
#[test]
fn site_link_other_pages_stay_addresses() {
    assert!(!own("<a href=\"/g/archive\">archive</a>"));
    assert_eq!(
        chan_link("<a href=\"/g/archive\">archive</a>"),
        ("archive".to_string(), "https://boards.4chan.org/g/archive".to_string())
    );
}

/// A link elsewhere is as written.
#[test]
fn site_link_elsewhere_is_as_written() {
    assert_eq!(
        chan_link("<a href=\"https://example.com/a\">x</a>"),
        ("x (example.com)".to_string(), "https://example.com/a".to_string())
    );
}

/// A relative path resolves beside the page; a query replaces the page's own; a fragment keeps
/// the page's query. Read with a base and no app links, so resolving is all that happens.
#[test]
fn site_link_relative_path_query_and_fragment() {
    let base = Some("https://example.com/a/page?q=1#top");
    let address = |html: &str| {
        let b = read(&render_html_blob(html, sites::HACKER_NEWS, base).unwrap());
        match b.spans.into_iter().find(|s| s.2 == LINK || s.2 == APP_LINK) {
            Some(Span(_, _, _, Data::Url(url))) => url,
            _ => panic!("no link in {html:?}"),
        }
    };
    assert_eq!(address("<a href=\"other\">x</a>"), "https://example.com/a/other");
    assert_eq!(address("<a href=\"?q=2\">x</a>"), "https://example.com/a/page?q=2");
    assert_eq!(address("<a href=\"#end\">x</a>"), "https://example.com/a/page?q=1#end");
    assert_eq!(address("<a href=\"/root\">x</a>"), "https://example.com/root");
}

/// A relative link that resolves onto an app page is the app's.
#[test]
fn site_link_relative_onto_an_app_page_is_the_apps() {
    assert_eq!(chan_link("<a href=\"201#p5\">x</a>").1, "https://boards.4chan.org/g/thread/201#p5");
}

// ── typed addresses ──────────────────────────────────────────────────────

/// Every link in a comment: its words and its address.
fn chan_links(html: &str) -> Vec<(String, String)> {
    let (text, spans) = chan(html);
    spans
        .into_iter()
        .filter_map(|s| match s {
            Span(start, end, t, Data::Url(url)) if t == LINK || t == APP_LINK => {
                let units: Vec<u16> = text.encode_utf16().collect();
                Some((String::from_utf16(&units[start as usize..end as usize]).unwrap(), url))
            }
            _ => None,
        })
        .collect()
}

/// A post's address typed among the words is the app's link to it, the address its words.
#[test]
fn typed_post_address_is_the_apps() {
    assert_eq!(
        chan_links("see https://boards.4chan.org/g/thread/123#p456 now"),
        vec![(
            "https://boards.4chan.org/g/thread/123#p456".to_string(),
            "https://boards.4chan.org/g/thread/123#p456".to_string()
        )]
    );
    assert_eq!(chan("see https://boards.4chan.org/g/thread/123#p456 now").0, "see https://boards.4chan.org/g/thread/123#p456 now");
}

/// An address the site broke with a `<wbr>` is read whole.
#[test]
fn typed_address_broken_by_wbr_is_read_whole() {
    assert_eq!(
        chan_links("https://boards.4chan.org/g/thr<wbr>ead/123"),
        vec![("https://boards.4chan.org/g/thread/123".to_string(), "https://boards.4chan.org/g/thread/123".to_string())]
    );
}

/// Several in a comment are each a link; one inside other markup is still one.
#[test]
fn typed_addresses_several_and_in_markup() {
    assert_eq!(
        chan_links("https://boards.4chan.org/g/thread/1 and <b>https://boards.4chan.org/v/thread/2</b>").len(),
        2
    );
}

/// An address that is already a link's words is that one link.
#[test]
fn typed_address_as_a_links_words_is_one_link() {
    assert_eq!(
        chan_links("<a href=\"https://example.com/a\">https://boards.4chan.org/g/thread/1</a>"),
        vec![("https://boards.4chan.org/g/thread/1".to_string(), "https://example.com/a".to_string())]
    );
}

/// Code is as written: an address in it is no link.
#[test]
fn typed_address_in_code_is_words() {
    assert!(chan_links("<pre class=\"prettyprint\">https://example.com/a</pre>").is_empty());
}

/// Another page on the site, or another site, is a link as written, its punctuation outside it.
#[test]
fn typed_address_elsewhere_is_a_link() {
    assert_eq!(
        chan_links("(see https://example.com/a)."),
        vec![("https://example.com/a".to_string(), "https://example.com/a".to_string())]
    );
    assert_eq!(
        chan_links("https://boards.4chan.org/po/archive"),
        vec![("https://boards.4chan.org/po/archive".to_string(), "https://boards.4chan.org/po/archive".to_string())]
    );
}

/// A `www.` address is a link over http, as comrak's autolink makes one.
#[test]
fn typed_www_address_is_a_link() {
    assert_eq!(
        chan_links("go to www.example.com today"),
        vec![("www.example.com".to_string(), "http://www.example.com".to_string())]
    );
}

/// A picture's address typed among the words is the picture.
#[test]
fn typed_picture_address_is_the_picture() {
    let (text, spans) = chan("look https://i.imgur.com/abc.png here");
    assert_eq!(text, "look \n\n\u{1}\n\n here");
    assert_eq!(spans, vec![url(7, 8, IMAGE, "https://i.imgur.com/abc.png")]);
}

// ── whole ────────────────────────────────────────────────────────────────

/// A real comment, as Algolia returns it: entities, a paragraph and a link whose words are its
/// address.
#[test]
fn whole_real_comment() {
    let html = "Ha, I didn&#x27;t expect this to show up on HN!<p>Release is out now, including a blog post at \
                <a href=\"https:&#x2F;&#x2F;www.mitmproxy.org&#x2F;posts&#x2F;releases&#x2F;mitmproxy-12.3&#x2F;\" rel=\"nofollow\">\
                https:&#x2F;&#x2F;www.mitmproxy.org&#x2F;posts&#x2F;releases&#x2F;mitmproxy-12.3&#x2F;</a>. :)";
    assert_eq!(
        spanned(html),
        (
            "Ha, I didn't expect this to show up on HN!\n\nRelease is out now, including a blog post at \
             https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/. :)"
                .to_string(),
            link(89, 145, "https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/").into()
        )
    );
}

// ── parity ───────────────────────────────────────────────────────────────

/// The same comment written as markdown, through the walk over comrak's tree, as the plain site.
fn markdown(md: &str) -> Option<Vec<u8>> {
    markdown_as(md, sites::PLAIN)
}

/// The same comment written as markdown, read as `site`.
fn markdown_as(md: &str, site: u8) -> Option<Vec<u8>> {
    let opts = production_options();
    parse_document_zerocopy(md, &opts, |root| render_blob(root, md, site))
}

/// What a reader shows: the blob read back, or, with none, its source as it is.
fn shown(blob: Option<Vec<u8>>, source: &str) -> (String, bool, Vec<Span>) {
    match blob {
        Some(blob) => {
            let b = read(&blob);
            (b.text, b.ascii, b.spans)
        }
        None => (source.to_string(), source.is_ascii(), vec![]),
    }
}

/// Each comment as the site sends it and as markdown, read through both walks: the same text, the
/// same ASCII flag and the same spans with the same addresses. Markdown answers no blob where its
/// text is unchanged; HTML must still answer one, its tags being no text, so the two compare as
/// shown.
#[test]
fn parity_html_reads_as_its_markdown() {
    let pairs: &[(&str, &str)] = &[
        ("one<p>two<p>three", "one\n\ntwo\n\nthree"),
        ("one<br>two", "one\\\ntwo"),
        ("I didn&#x27;t &#x2F; won&#x27;t", "I didn't / won't"),
        ("&#x1F600; <i>x</i>", "\u{1F600} *x*"),
        ("&gt; On October 17, 2026", "> On October 17, 2026"),
        ("before<p>&gt; quoted<p>after", "before\n\n> quoted\n\nafter"),
        ("&gt; one<p>&gt; two", "> one\n\n> two"),
        ("&gt;&gt; nested", ">> nested"),
        ("&gt; a <i>b</i>", "> a *b*"),
        ("i <i>havent</i> updated", "i *havent* updated"),
        ("a <b>bold</b> <strong>b</strong>", "a **bold** **b**"),
        ("<i>x</i> a *b* _c_", "*x* a \\*b\\* \\_c\\_"),
        ("(c) 2026 +- 1<p>x", "(c) 2026 +- 1\n\nx"),
        (
            "it&#x2019;s &#x201C;quoted&#x201D; &#x2014; dash&#x2026;",
            "it\u{2019}s \u{201C}quoted\u{201D} \u{2014} dash\u{2026}",
        ),
        (
            "<a href=\"https:&#x2F;&#x2F;github.com&#x2F;mitmproxy&#x2F;mitmproxy&#x2F;pull&#x2F;8503\" rel=\"nofollow\">\
             https:&#x2F;&#x2F;github.com&#x2F;mitmproxy&#x2F;mitmproxy&#x2F;pull&#x2F;8503</a> adds the AAA root path.",
            "[https://github.com/mitmproxy/mitmproxy/pull/8503](https://github.com/mitmproxy/mitmproxy/pull/8503) adds the AAA root path.",
        ),
        (
            "<a href=\"https:&#x2F;&#x2F;en.wikipedia.org&#x2F;wiki&#x2F;EMudhra#2026_compromise\">\
             https:&#x2F;&#x2F;en.wikipedia.org&#x2F;wiki&#x2F;EMudhra#2026_comp...</a>",
            "[https://en.wikipedia.org/wiki/EMudhra#2026\\_comp...](https://en.wikipedia.org/wiki/EMudhra#2026_compromise)",
        ),
        (
            "<a href=\"https://boards.4chan.org/g/thread/200#p201\">post</a>",
            "[post](https://boards.4chan.org/g/thread/200#p201)",
        ),
        (
            "see <a href=\"https://www.example.com/a\">the docs</a> now",
            "see [the docs](https://www.example.com/a) now",
        ),
        ("<a href=\"https://youtu.be/abc\">https://youtu.be/abc</a>", "[https://youtu.be/abc](https://youtu.be/abc)"),
        (
            "<a href=\"https://i.imgur.com/abc.png\">https://i.imgur.com/abc.png</a>",
            "[https://i.imgur.com/abc.png](https://i.imgur.com/abc.png)",
        ),
        (
            "look <a href=\"https://i.imgur.com/abc.png\">https://i.imgur.com/abc.png</a> here",
            "look [https://i.imgur.com/abc.png](https://i.imgur.com/abc.png) here",
        ),
        ("<a href=\"https://i.imgur.com/abc.png\">a pic</a>", "[a pic](https://i.imgur.com/abc.png)"),
        (
            "logs:<p><pre><code>  Audit Logs\n  Oct 21, 2026</code></pre>",
            "logs:\n\n```\n  Audit Logs\n  Oct 21, 2026\n```",
        ),
        ("<pre><code>  x = 1\n</code></pre>\nafter", "```\n  x = 1\n```\nafter"),
        ("<pre><code>  a *b* [c]</code></pre>", "```\n  a *b* [c]\n```"),
        ("<pre><code>a &lt; b</code></pre>", "```\na < b\n```"),
        (
            "Ha, I didn&#x27;t expect this to show up on HN!<p>Release is out now, including a blog post at \
             <a href=\"https:&#x2F;&#x2F;www.mitmproxy.org&#x2F;posts&#x2F;releases&#x2F;mitmproxy-12.3&#x2F;\" rel=\"nofollow\">\
             https:&#x2F;&#x2F;www.mitmproxy.org&#x2F;posts&#x2F;releases&#x2F;mitmproxy-12.3&#x2F;</a>. :)",
            "Ha, I didn't expect this to show up on HN!\n\nRelease is out now, including a blog post at \
             [https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/](https://www.mitmproxy.org/posts/releases/mitmproxy-12.3/). :)",
        ),
    ];
    for (html, md) in pairs {
        assert_eq!(
            shown(parse(html), html),
            shown(markdown(md), md),
            "{html:?} against {md:?}"
        );
    }
}

/// 4chan's tags where markdown can say what the site shows, as `ChanHtml` wrote them for
/// comrak: a dead link, a spoiler, breaks and typed addresses. Greentext has no markdown.
#[test]
fn parity_chan_tags_read_as_their_markdown() {
    let pairs: &[(&str, &str)] = &[
        ("<span class=\"deadlink\">&gt;&gt;123</span>", "~~\\>>123~~"),
        ("a <s>b</s> c", "a \n\n::: spoiler Spoiler\nb\n:::\n\n c"),
        ("one<br>two<br><br>three", "one  \ntwo  \n  \nthree"),
        ("see https://example.com/a now", "see https://example.com/a now"),
        ("go to www.example.com today", "go to www.example.com today"),
        ("(see https://example.com/a).", "(see https://example.com/a)."),
        ("look https://i.imgur.com/abc.png here", "look https://i.imgur.com/abc.png here"),
    ];
    for (html, md) in pairs {
        assert_eq!(
            shown(render_html_blob(html, sites::CHAN, Some(THREAD)), html),
            shown(markdown_as(md, sites::CHAN), md),
            "{html:?} against {md:?}"
        );
    }
    // A post's address typed among the words is the app's either way
    let (html, md) = (
        "see https://boards.4chan.org/g/thread/123#p456 now",
        "see https://boards.4chan.org/g/thread/123#p456 now",
    );
    assert_eq!(
        shown(render_html_blob(html, sites::CHAN, Some(THREAD)), html),
        shown(markdown_as(md, sites::CHAN), md)
    );
    for (html, md) in pairs {
        assert_eq!(
            shown(render_html_blob(html, sites::CHAN, Some(THREAD)), html),
            shown(markdown(md), md),
            "{html:?} against {md:?}"
        );
    }
}
