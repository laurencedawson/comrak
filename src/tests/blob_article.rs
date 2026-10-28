//! Tests for the blob renderer's walk over a web page's article (`comrak::blob::render_article_blob`).
//!
//! - `extraction` : the article kept, the page around it dropped, title and byline read
//! - `tree`       : the closes HTML implies, scripts, styles and templates no part of the page
//! - `blocks`     : paragraphs, headings, lists, quotes, code, rules, table rows
//! - `inline`     : emphasis, code, superscript, whitespace
//! - `links`      : resolved against the page, the page's own host without its domain, anchors plain
//! - `pictures`   : `src`, the widest of a `srcset`, a lazy picture's real address, a linked picture,
//!   the page's own lead picture

use crate::blob::{
    BOLD, CODE, CODE_BLOCK, HEADINGS, HRULE, IMAGE, ITALIC, LINK, LINK_SIZE, LIST_ITEM, QUOTE,
    SUPERSCRIPT, render_article_blob,
};
use crate::tests::blob_read::{Blob, Span, read};

const PAGE: &str = "https://example.com/blog/post";

/// Prose long enough that Readability takes the block holding it for the article.
const PROSE: &str = "This paragraph carries the article, with commas, clauses, and enough words \
     that the scoring counts it as content rather than chrome, which is what a reader wants.";

/// `body` as a page's, under `title`.
fn page(title: &str, body: &str) -> String {
    format!("<!DOCTYPE html><html><head><title>{title}</title></head><body>{body}</body></html>")
}

/// The article's blob, which must exist: a page with no title, so its words are the blob's.
fn article(body: &str) -> Blob {
    let html = format!("<!DOCTYPE html><html><body>{body}</body></html>");
    whole(&html)
}

/// The blob for the whole page `html`, which must have an article.
fn whole(html: &str) -> Blob {
    read(&render_article_blob(html, PAGE).unwrap_or_else(|| panic!("no article in {html:?}")))
}

/// The page's title, as the heading the blob opens with.
fn title(html: &str) -> String {
    let b = whole(html);
    words(&b, of(&b, HEADINGS[0])[0]).to_string()
}

/// `body` set among enough prose to be the article.
fn within(body: &str) -> Blob {
    article(&format!(
        "<article><p>{PROSE}</p>{body}<p>{PROSE}</p></article>"
    ))
}

/// The spans of type `typ`.
fn of(b: &Blob, typ: i32) -> Vec<&Span> {
    b.span_iter().filter(|s| s.typ == typ).collect()
}

/// The words a span covers.
fn words<'b>(b: &'b Blob, s: &Span) -> &'b str {
    let utf16: Vec<u16> = b.text.encode_utf16().collect();
    let start = String::from_utf16(&utf16[..s.start as usize])
        .unwrap()
        .len();
    let end = String::from_utf16(&utf16[..s.end as usize]).unwrap().len();
    &b.text[start..end]
}

// extraction

#[test]
fn a_short_page_keeps_what_it_has() {
    // No attempt reaches Readability's 500 characters, so the longest wins, sidebar and all
    let b = article(&format!(
        "<div class=\"sidebar\"><p>Subscribe, follow us, and read more.</p></div><article><p>{PROSE}</p></article>"
    ));
    assert_eq!(
        b.text,
        format!("Subscribe, follow us, and read more.\n\n{PROSE}")
    );
}

#[test]
fn keeps_the_article_and_drops_the_page_around_it() {
    let b = article(&format!(
        "<nav class=\"menu\"><a href=\"/\">Home</a> <a href=\"/about\">About</a></nav>\
         <div class=\"sidebar\"><p>Subscribe, follow us, and read more from the archive.</p></div>\
         <article><p>{PROSE}</p><p>{PROSE}</p><p>{PROSE}</p><p>{PROSE}</p></article>\
         <footer><p>Copyright, all rights reserved, terms and privacy.</p></footer>"
    ));
    assert_eq!(b.text, [PROSE; 4].join("\n\n"));
}

#[test]
fn reads_the_title_without_its_site() {
    let html = page(
        "How ordered and nested lists work | The Example Blog",
        &format!("<p>{PROSE}</p>"),
    );
    assert_eq!(title(&html), "How ordered and nested lists work");
}

#[test]
fn keeps_a_short_title_whole() {
    let html = page(
        "How lists work | The Example Blog",
        &format!("<p>{PROSE}</p>"),
    );
    assert_eq!(title(&html), "How lists work | The Example Blog");
}

#[test]
fn prefers_the_metadata_title() {
    let html = format!(
        "<html><head><title>Ignored | Site</title><meta property=\"og:title\" content=\"The real one\"></head>\
         <body><p>{PROSE}</p></body></html>"
    );
    assert_eq!(title(&html), "The real one");
}

#[test]
fn takes_the_byline_out_of_the_words() {
    let html = page(
        "A title long enough to keep",
        &format!("<div class=\"byline\">By Ada Lovelace</div><p>{PROSE}</p>"),
    );
    let b = whole(&html);
    assert_eq!(words(&b, of(&b, ITALIC)[0]), "By Ada Lovelace");
    assert_eq!(
        b.text.matches("Ada").count(),
        1,
        "once, at the top: {:?}",
        b.text
    );
}

#[test]
fn reads_as_one_page_title_byline_picture_words() {
    let html = format!(
        "<html><head><title>A title long enough to keep</title>\
         <meta name=\"author\" content=\"Grace Hopper\">\
         <meta property=\"og:image\" content=\"https://cdn.example.com/lead.jpg\"></head>\
         <body><article><p>{PROSE}</p></article></body></html>"
    );
    let b = whole(&html);
    assert_eq!(
        b.text,
        format!("A title long enough to keep\nGrace Hopper\n\n\u{1}\n\n{PROSE}")
    );
    assert_eq!(
        words(&b, of(&b, HEADINGS[0])[0]),
        "A title long enough to keep"
    );
    assert_eq!(words(&b, of(&b, BOLD)[0]), "A title long enough to keep");
    assert_eq!(words(&b, of(&b, ITALIC)[0]), "Grace Hopper");
    assert_eq!(
        of(&b, IMAGE)[0].url.as_deref(),
        Some("https://cdn.example.com/lead.jpg")
    );
}

#[test]
fn reads_the_byline_from_metadata_and_leaves_the_words() {
    let html = format!(
        "<html><head><meta name=\"author\" content=\"Grace Hopper\"></head>\
         <body><p>{PROSE}</p></body></html>"
    );
    let b = whole(&html);
    assert_eq!(words(&b, of(&b, ITALIC)[0]), "Grace Hopper");
    assert_eq!(b.text.matches("Grace").count(), 1);
}

#[test]
fn a_page_with_no_words_has_no_article() {
    assert!(render_article_blob(&page("Just a moment...", "<div></div>"), PAGE).is_none());
    assert!(render_article_blob("", PAGE).is_none());
}

// tree

#[test]
fn a_block_closes_an_open_paragraph() {
    let b = within("<p>one<div>two</div>");
    assert!(b.text.contains("one\n\ntwo"), "{:?}", b.text);
}

#[test]
fn an_item_closes_the_one_before() {
    let b = within("<ul><li>one<li>two</ul>");
    let items: Vec<_> = of(&b, LIST_ITEM).iter().map(|s| words(&b, s)).collect();
    assert_eq!(items, ["one", "two"]);
}

#[test]
fn scripts_styles_and_templates_are_no_part_of_the_page() {
    let b = within(
        "<script>var hidden = \"</p>\";</script><style>p { color: red }</style>\
         <template><template><p>inner</p></template><p>outer</p></template><p>after</p>",
    );
    for gone in ["hidden", "color", "inner", "outer"] {
        assert!(!b.text.contains(gone), "{gone} in {:?}", b.text);
    }
    assert!(b.text.contains("after"));
}

#[test]
fn entities_are_decoded() {
    let b = within("<p>Fish &amp; chips &lt;3 caf&eacute;</p>");
    assert!(b.text.contains("Fish & chips <3 café"), "{:?}", b.text);
}

// blocks

#[test]
fn headings() {
    let b = within("<h2>Second</h2><h3>Third</h3>");
    let [h2, h3] = [HEADINGS[1], HEADINGS[2]].map(|t| of(&b, t)[0].clone());
    assert_eq!((words(&b, &h2), words(&b, &h3)), ("Second", "Third"));
    assert_eq!(of(&b, BOLD).len(), 2);
    assert!(b.text.contains("\n\nSecond\n\nThird\n\n"));
}

#[test]
fn an_h1_in_the_article_is_an_h2() {
    let b = within("<h1>Section</h1>");
    assert_eq!(words(&b, of(&b, HEADINGS[1])[0]), "Section");
    assert!(of(&b, HEADINGS[0]).is_empty());
}

#[test]
fn nested_ordered_lists_number_and_indent() {
    let b = within("<ol start=\"3\"><li>three<ul><li>inner</li></ul></li><li>four</li></ol>");
    let items: Vec<_> = of(&b, LIST_ITEM)
        .iter()
        .map(|s| (words(&b, s), s.indent(), s.number()))
        .collect();
    assert_eq!(items, [("three", 0, 3), ("inner", 1, 0), ("four", 0, 4)]);
    assert!(b.text.contains("three\ninner\nfour"), "{:?}", b.text);
}

#[test]
fn quotes_nest() {
    let b = within("<blockquote><p>outer</p><blockquote><p>inner</p></blockquote></blockquote>");
    let quotes: Vec<_> = of(&b, QUOTE)
        .iter()
        .map(|s| (words(&b, s), s.data))
        .collect();
    assert_eq!(quotes, [("outer\n\ninner", 0), ("inner", 1)]);
}

#[test]
fn preformatted_code_is_kept_as_written() {
    let b = within("<pre><code>\nfn main() {\n    run();\n}\n</code></pre>");
    assert_eq!(
        words(&b, of(&b, CODE_BLOCK)[0]),
        "fn main() {\n    run();\n}"
    );
    assert!(
        of(&b, CODE).is_empty(),
        "the code inside a pre is the block's"
    );
}

#[test]
fn a_rule() {
    let b = within("<hr>");
    assert_eq!(words(&b, of(&b, HRULE)[0]), "\u{1}");
}

#[test]
fn table_rows_read_across() {
    let b =
        within("<table><tr><th>Name</th><th>Age</th></tr><tr><td>Ada</td><td>36</td></tr></table>");
    assert!(b.text.contains("Name | Age\nAda | 36"), "{:?}", b.text);
}

// inline

#[test]
fn emphasis_and_code() {
    let b = within("<p>a <b>bold</b>, <em>italic</em> and <code>code</code> word</p>");
    assert!(
        b.text.contains("a bold, italic and code word"),
        "{:?}",
        b.text
    );
    assert_eq!(words(&b, of(&b, BOLD)[0]), "bold");
    assert_eq!(words(&b, of(&b, ITALIC)[0]), "italic");
    assert_eq!(words(&b, of(&b, CODE)[0]), "code");
}

#[test]
fn superscript() {
    let b = within("<p>E = mc<sup>2</sup></p>");
    assert_eq!(words(&b, of(&b, SUPERSCRIPT)[0]), "2");
}

#[test]
fn whitespace_collapses_and_none_opens_a_block() {
    let b = within("<p>\n   spaced \t\n  out  <i> here </i>  </p>");
    assert!(b.text.contains("\n\nspaced out here\n\n"), "{:?}", b.text);
}

// links

#[test]
fn a_link_on_the_page_is_resolved_and_needs_no_domain() {
    let b = within("<p>see <a href=\"../other\">the other post</a></p>");
    let link = of(&b, LINK)[0];
    assert_eq!(
        link.url.as_deref(),
        Some("https://example.com/blog/../other")
    );
    assert_eq!(words(&b, link), "the other post");
    assert_eq!(words(&b, of(&b, LINK_SIZE)[0]), "the other post");
    assert!(b.text.contains("see the other post\n"), "{:?}", b.text);
}

#[test]
fn a_link_elsewhere_names_its_domain() {
    let b = within("<p>see <a href=\"https://rust-lang.org/learn\">the book</a></p>");
    assert!(b.text.contains("the book (rust-lang.org)"), "{:?}", b.text);
}

#[test]
fn an_anchor_on_the_page_is_words() {
    let b = within("<p>see <a href=\"#notes\">the notes</a></p>");
    assert!(of(&b, LINK).is_empty());
    assert!(b.text.contains("see the notes"));
}

#[test]
fn a_script_link_is_words() {
    let b = within(
        "<p>tap <a href=\"JavaScript:void(0)\">here</a> or <a href=\"javascript:\"></a>there</p>",
    );
    assert!(of(&b, LINK).is_empty());
    assert!(b.text.contains("tap here or there"), "{:?}", b.text);
}

// pictures

#[test]
fn a_picture_by_its_src() {
    let b = within("<figure><img src=\"/img/a.jpg\" alt=\"A\"></figure>");
    assert_eq!(
        of(&b, IMAGE)[0].url.as_deref(),
        Some("https://example.com/img/a.jpg")
    );
}

#[test]
fn the_widest_of_a_srcset() {
    let b = within(
        "<img srcset=\"https://cdn.example.com/a.jpg?w=320&amp;crop=0%2C0 320w, \
         https://cdn.example.com/a.jpg?w=1200 1200w, https://cdn.example.com/a.jpg?w=640 640w\">",
    );
    assert_eq!(
        of(&b, IMAGE)[0].url.as_deref(),
        Some("https://cdn.example.com/a.jpg?w=1200")
    );
}

#[test]
fn a_lazy_picture_by_its_real_address() {
    let b = within(
        "<img class=\"lazy\" src=\"data:image/gif;base64,R0lGODlhAQABAAAAACw=\" \
         data-src=\"https://cdn.example.com/real.png\">",
    );
    assert_eq!(
        of(&b, IMAGE)[0].url.as_deref(),
        Some("https://cdn.example.com/real.png")
    );
}

#[test]
fn a_linked_picture_is_the_picture() {
    let b = within("<a href=\"/big.jpg\"><img src=\"/small.jpg\"></a>");
    assert_eq!(
        of(&b, IMAGE)[0].url.as_deref(),
        Some("https://example.com/small.jpg")
    );
    assert!(of(&b, LINK).is_empty());
}

#[test]
fn the_page_leads_with_its_picture() {
    let html = format!(
        "<html><head><meta property=\"og:image\" content=\"https://cdn.example.com/lead.jpg?w=1200&amp;q=90\">\
         </head><body><article><p>{PROSE}</p></article></body></html>"
    );
    let b = whole(&html);
    let image = of(&b, IMAGE)[0];
    assert_eq!(
        image.url.as_deref(),
        Some("https://cdn.example.com/lead.jpg?w=1200&q=90")
    );
    assert_eq!(image.start, 0);
    assert_eq!(b.text, format!("\u{1}\n\n{PROSE}"));
}

#[test]
fn the_lead_picture_once_where_the_article_shows_it() {
    let html = format!(
        "<html><head><meta property=\"og:image\" content=\"https://cdn.example.com/lead.jpg?w=1200\">\
         </head><body><article><p>{PROSE}</p><img src=\"https://cdn.example.com/lead.jpg?w=640\"></article></body></html>"
    );
    let b = whole(&html);
    let images: Vec<_> = of(&b, IMAGE)
        .iter()
        .map(|s| s.url.clone().unwrap())
        .collect();
    assert_eq!(images, ["https://cdn.example.com/lead.jpg?w=640"]);
}
