//! Tests for the blob renderer's walk over a web page's article (`comrak::blob::render_article_blob`).
//!
//! - `extraction` : the article kept, the page around it dropped, the title first and centred, the
//!   byline out
//! - `tree`       : the closes HTML implies, scripts, styles and templates no part of the page
//! - `blocks`     : paragraphs, headings, lists, quotes, code, rules, table rows
//! - `inline`     : emphasis, code, superscript, whitespace
//! - `links`      : resolved against the page, the page's own host without its domain, anchors plain
//! - `pictures`   : `src`, the widest of a `srcset`, a lazy picture's real address, a linked picture,
//!   the page's lead picture kept where the article shows it, a size and alt

use crate::blob::{
    BOLD, CENTERED, CODE, CODE_BLOCK, HEADINGS, HRULE, IMAGE, ITALIC, LINK, LINK_SIZE, LIST_ITEM,
    QUOTE, SUPERSCRIPT, render_article_blob,
};
use crate::tests::blob_read::{Blob, Picture, Span, read};

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

/// The title is the blob's first line, a first level heading with a `CENTERED` span over the same
/// words; no heading of the article's own is centred.
#[test]
fn opens_on_its_title_centred() {
    let html = page(
        "A title long enough to keep",
        &format!("<article><h2>A section</h2><p>{PROSE}</p></article>"),
    );
    let b = whole(&html);
    assert_eq!(
        b.text,
        format!("A title long enough to keep\n\nA section\n\n{PROSE}")
    );
    let title: Vec<_> = of(&b, HEADINGS[0]).iter().map(|s| words(&b, s)).collect();
    let centred: Vec<_> = of(&b, CENTERED).iter().map(|s| words(&b, s)).collect();
    assert_eq!(title, ["A title long enough to keep"]);
    assert_eq!(centred, title);
}

/// A page with JSON-LD in its head naming `author`, and `body` after the title.
fn ld_page(ld: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><title>A title long enough to keep</title>\
         <script type=\"application/ld+json\">{ld}</script></head><body>{body}</body></html>"
    )
}

/// The authors the page's JSON-LD names stand on the line under the title, centred at the body's
/// size, however the JSON writes them: a person, several, or a bare string.
#[test]
fn names_the_author_under_the_title() {
    let b = whole(&ld_page(
        r#"{"@context":"https://schema.org","@type":"NewsArticle","headline":"x","author":{"@type":"Person","name":"Yasemin Saplakoglu"},"publisher":{"@type":"Organization","name":"Quanta"}}"#,
        &format!("<article><p>{PROSE}</p></article>"),
    ));
    assert_eq!(
        b.text,
        format!("A title long enough to keep\nYasemin Saplakoglu\n\n{PROSE}")
    );
    let centred: Vec<_> = of(&b, CENTERED).iter().map(|s| words(&b, s)).collect();
    assert_eq!(
        centred,
        ["A title long enough to keep", "Yasemin Saplakoglu"]
    );
    let headings: Vec<_> = of(&b, HEADINGS[0]).iter().map(|s| words(&b, s)).collect();
    assert_eq!(headings, ["A title long enough to keep"]);
}

#[test]
fn names_every_author() {
    let b = whole(&ld_page(
        r#"{"@type":"NewsArticle","author":[{"@type":"Person","name":"Dhruv Mehrotra","url":"https://x/a"},{"@type":"Person","name":"Maddy Varner"},{"@type":"Person","name":"Dell Cameron"}]}"#,
        &format!("<p>{PROSE}</p>"),
    ));
    assert!(b.text.starts_with(
        "A title long enough to keep\nDhruv Mehrotra, Maddy Varner, Dell Cameron\n\n"
    ));
}

#[test]
fn names_an_author_written_as_a_string_or_in_a_graph() {
    let b = whole(&ld_page(
        r#"{"@type":"BlogPosting","author":"Paul Walsh"}"#,
        &format!("<p>{PROSE}</p>"),
    ));
    assert!(
        b.text
            .starts_with("A title long enough to keep\nPaul Walsh\n\n")
    );
    let b = whole(&ld_page(
        r##"{"@graph":[{"@type":"WebPage","name":"page"},{"@type":"Article","author":{"@id":"#p","name":"Liviu Stirb"},"about":{"name":"not an author"}},{"@type":"Person","@id":"#p","name":"Liviu Stirb"}]}"##,
        &format!("<p>{PROSE}</p>"),
    ));
    assert!(
        b.text
            .starts_with("A title long enough to keep\nLiviu Stirb\n\n")
    );
}

/// Escapes are read, and `"author"` inside a string is a word, not a key.
#[test]
fn reads_the_authors_name_through_its_escapes() {
    let b = whole(&ld_page(
        r#"{"@type":"Article","description":"the \"author\" of it all","author":{"name":"Simson Garfinkel ’87, PhD ’05 \"Si\""}}"#,
        &format!("<p>{PROSE}</p>"),
    ));
    // The blob writes typographic quotes as typed, so the curly apostrophes land straight
    assert!(
        b.text
            .starts_with("A title long enough to keep\nSimson Garfinkel '87, PhD '05 \"Si\"\n\n"),
        "{:?}",
        b.text.lines().nth(1)
    );
}

/// The page's own byline element goes whatever the JSON-LD says, and a page with no JSON-LD author
/// opens on its title alone.
#[test]
fn the_pages_byline_element_goes_and_names_no_one() {
    let b = whole(&ld_page(
        r#"{"@type":"Article","author":{"name":"Ada Lovelace"}}"#,
        &format!("<div class=\"byline\">By Ada Lovelace · 4 min read</div><p>{PROSE}</p>"),
    ));
    assert_eq!(
        b.text,
        format!("A title long enough to keep\nAda Lovelace\n\n{PROSE}")
    );
    let b = whole(&page(
        "A title long enough to keep",
        &format!("<div class=\"byline\">By Ada Lovelace</div><p>{PROSE}</p>"),
    ));
    assert_eq!(b.text, format!("A title long enough to keep\n\n{PROSE}"));
}

/// A page with `head` for its head and enough words to be an article.
fn headed(head: &str) -> String {
    format!("<html><head>{head}</head><body><article><p>{PROSE}</p></article></body></html>")
}

/// The JSON-LD script giving an object of `kind` its `key`.
fn ld(kind: &str, key: &str, value: &str) -> String {
    format!(
        "<script type=\"application/ld+json\">{{\"@context\":\"https://schema.org\",\"@type\":\"{kind}\",\"{key}\":\"{value}\"}}</script>"
    )
}

/// An article's JSON-LD headline is its title, the site's name nowhere in it.
#[test]
fn heads_the_article_with_its_json_ld_headline() {
    let html = headed(&format!(
        "<meta property=\"og:title\" content=\"Scaling Hierarchical Networks | Cartesia\">{}",
        ld("BlogPosting", "headline", "Scaling Hierarchical Networks")
    ));
    assert_eq!(title(&html), "Scaling Hierarchical Networks");
}

/// A headline's entities are decoded and its tags dropped, as the page's own title's are; WordPress
/// writes its JSON-LD as HTML (Gizmodo, 2026-10).
#[test]
fn decodes_the_json_ld_headline() {
    for headline in [
        "We Asked Starbucks Why Their &#8216;Sugar-Free&#8217; Drinks Have Sugar and Now We&#8217;re Just More Confused",
        "We <em>Asked</em> Starbucks Why Their Sugar-Free Drinks Have Sugar and Now We Are Just More Confused",
    ] {
        let html = headed(&format!(
            "<title>Starbucks Sugar-Free Drinks Have Sugar and Now We Are Just More Confused</title>{}",
            ld("NewsArticle", "headline", headline)
        ));
        assert!(title(&html).starts_with("We Asked Starbucks"), "{headline}");
        assert!(!title(&html).contains(['&', '<']), "{headline}");
    }
}

/// A title's tags escaped in its `<meta>` are dropped, not shown (Animation Studies Journal, 2026-10).
#[test]
fn drops_the_tags_a_meta_title_escapes() {
    let html = headed(
        "<meta property=\"og:title\" content=\"A Pig, the State, and War: &lt;i&gt;Porco Rosso&lt;/i&gt; (&lt;i&gt;Kurenai no Buta&lt;/i&gt;)\">",
    );
    assert_eq!(
        title(&html),
        "A Pig, the State, and War: Porco Rosso (Kurenai no Buta)"
    );
}

/// A headline that is a summary rather than the title, as Wikipedia's is, gives way to the title.
#[test]
fn passes_over_a_headline_that_is_no_title() {
    let html = headed(&format!(
        "<meta property=\"og:title\" content=\"Oseberg tapestry fragments - Wikipedia\">\
         <meta property=\"og:site_name\" content=\"Wikipedia\">{}",
        ld(
            "Article",
            "headline",
            "fragments of tapestry discovered in the Oseberg ship burial in Norway"
        )
    ));
    assert_eq!(title(&html), "Oseberg tapestry fragments");
}

/// A web page's JSON-LD name is the page's title, site and all, so it heads nothing.
#[test]
fn reads_no_title_from_a_web_pages_name() {
    let html = headed(&format!(
        "<meta property=\"og:title\" content=\"CNC Stone Carving\">{}",
        ld("WebPage", "name", "Something else - Carmelite Gothic")
    ));
    assert_eq!(title(&html), "CNC Stone Carving");
}

/// The site's name comes off either end of the title, as its metadata names it, as its host, or as
/// the host's name alone, whatever the separator and the case.
#[test]
fn takes_the_sites_name_off_the_title() {
    for (head, expected) in [
        (
            "<meta property=\"og:title\" content=\"CNC Stone Carving - Carmelite Gothic\">\
             <meta property=\"og:site_name\" content=\"Carmelite Gothic\">",
            "CNC Stone Carving",
        ),
        (
            "<meta property=\"og:title\" content=\"example.com • Why Is This Script Creating Bioweapons?\">",
            "Why Is This Script Creating Bioweapons?",
        ),
        (
            "<meta property=\"og:title\" content=\"A Health Update · Example\">",
            "A Health Update",
        ),
    ] {
        assert_eq!(title(&headed(head)), expected, "{head}");
    }
}

/// A title that is the site's name alone keeps it.
#[test]
fn keeps_a_title_that_is_the_sites_name() {
    let html = headed("<meta property=\"og:title\" content=\"Example\">");
    assert_eq!(title(&html), "Example");
}

#[test]
fn takes_the_byline_out_of_the_words() {
    let html = page(
        "A title long enough to keep",
        &format!("<div class=\"byline\">By Ada Lovelace</div><p>{PROSE}</p>"),
    );
    assert_eq!(
        whole(&html).text,
        format!("A title long enough to keep\n\n{PROSE}")
    );
}

#[test]
fn takes_the_byline_out_of_the_words_where_the_metadata_names_the_author() {
    let b = whole(&format!(
        "<html><head><meta name=\"author\" content=\"Ada Lovelace\"></head><body>\
         <p class=\"cover__authors\">Ada Lovelace</p><p>{PROSE}</p></body></html>"
    ));
    assert_eq!(b.text, PROSE);
}

/// A byline stands ahead of the body, like a dateline: an element named for an author past the
/// first paragraph is the article's, a box about one in a profile piece.
#[test]
fn keeps_an_author_box_in_the_body() {
    let b = article(&format!(
        "<p class=\"byline\">By Ada Lovelace</p><p>{PROSE}</p>\
         <div class=\"author-box\">Grace Hopper wrote the first compiler.</div>"
    ));
    assert_eq!(
        b.text,
        format!("{PROSE}\n\nGrace Hopper wrote the first compiler.")
    );
}

#[test]
fn drops_a_share_button_named_with_a_hyphen() {
    let b = article(&format!(
        "<article><label class=\"button share-toggle__button\">Share</label><p>{PROSE}</p></article>"
    ));
    assert_eq!(b.text, PROSE);
}

#[test]
fn drops_a_date_its_class_names() {
    let b = article(&format!(
        "<article><p class=\"mobile-date\">31st December 2024</p><p>{PROSE}</p></article>"
    ));
    assert_eq!(b.text, PROSE);
}

#[test]
fn drops_a_time_with_its_label() {
    let b = article(&format!(
        "<article><div>Updated: <time datetime=\"2026-10-28\">28 October 2026 - 14:38</time></div><p>{PROSE}</p></article>"
    ));
    assert_eq!(b.text, PROSE);
}

#[test]
fn drops_an_eyebrows_category_with_its_date() {
    let b = article(&format!(
        "<article><div class=\"eyebrow\"><span class=\"cover-date\">October 27, 2026</span>\
         <span class=\"category\">Models</span></div><p>{PROSE}</p></article>"
    ));
    assert_eq!(b.text, PROSE);
}

#[test]
fn drops_a_date_line_with_its_reading_time_and_tags() {
    let b = article(&format!(
        "<article><p><span class=\"date\">2026-10-27</span> · 15 min · <a href=\"/tags/computing\">#computing</a></p>\
         <p>{PROSE}</p></article>"
    ));
    assert_eq!(b.text, PROSE);
}

#[test]
fn keeps_the_sentence_a_dated_span_sits_in() {
    let b = article(&format!(
        "<article><p>Posted <span class=\"date\">3 March</span> and fixed since.</p><p>{PROSE}</p></article>"
    ));
    assert_eq!(
        b.text,
        format!("Posted 3 March and fixed since.\n\n{PROSE}")
    );
}

/// A dateline is what stands between the title and the first paragraph: past that, a date is the
/// article's, however its element is named.
#[test]
fn keeps_a_dated_element_in_the_body() {
    let b = article(&format!(
        "<article><p>{PROSE}</p><p class=\"prep-time\">Prep time 20 minutes</p><p>{PROSE}</p></article>"
    ));
    assert_eq!(
        b.text,
        format!("{PROSE}\n\nPrep time 20 minutes\n\n{PROSE}")
    );
}

#[test]
fn keeps_a_timed_span_in_a_sentence() {
    let b = article(&format!(
        "<article><p>{PROSE}</p><p>She won in <span class=\"time\">2:03:45</span> flat.</p></article>"
    ));
    assert_eq!(b.text, format!("{PROSE}\n\nShe won in 2:03:45 flat."));
}

#[test]
fn keeps_a_release_table_in_the_body() {
    let b = article(&format!(
        "<article><p>{PROSE}</p><table><tr><td class=\"date\">2026-10-01</td><td>v1.2</td></tr>\
         <tr><td class=\"date\">2026-09-01</td><td>v1.1</td></tr></table></article>"
    ));
    assert_eq!(
        b.text,
        format!("{PROSE}\n\n2026-10-01 | v1.2\n2026-09-01 | v1.1")
    );
}

#[test]
fn drops_the_dateline_ahead_of_the_body_alone() {
    let b = article(&format!(
        "<article><p class=\"updated\">Updated 28 Oct 2026 · 4 min read</p><p>{PROSE}</p>\
         <p class=\"updated\">Updated 28 Oct 2026</p></article>"
    ));
    assert_eq!(b.text, format!("{PROSE}\n\nUpdated 28 Oct 2026"));
}

#[test]
fn keeps_a_time_in_a_sentence() {
    let sentence =
        "On <time datetime=\"2026-03-03\">3 March</time> we shipped the fix everyone asked for.";
    let b = article(&format!(
        "<article><p>{sentence}</p><p>{PROSE}</p></article>"
    ));
    assert_eq!(
        b.text,
        format!("On 3 March we shipped the fix everyone asked for.\n\n{PROSE}")
    );
}

#[test]
fn leaves_the_byline_and_lead_picture_out() {
    let html = format!(
        "<html><head><title>A title long enough to keep</title>\
         <meta name=\"author\" content=\"Grace Hopper\">\
         <meta property=\"og:image\" content=\"https://cdn.example.com/lead.jpg\"></head>\
         <body><article><p>{PROSE}</p></article></body></html>"
    );
    let b = whole(&html);
    assert_eq!(b.text, format!("A title long enough to keep\n\n{PROSE}"));
    assert!(of(&b, IMAGE).is_empty());
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

/// In a list a picture takes the item's lines, as in markdown: no blank line either side, the
/// item opening on its picture when the picture comes first.
#[test]
fn a_picture_in_an_item_takes_the_items_lines() {
    let pic = "<img src=\"https://example.com/a.png\">";
    for (li, item) in [
        (format!("two {pic} and more"), "two\n\u{1}\nand more"),
        (format!("two {pic}"), "two\n\u{1}"),
        (format!("{pic} and more"), "\u{1}\nand more"),
    ] {
        let b = within(&format!("<ul><li>one</li><li>{li}</li><li>three</li></ul>"));
        let items: Vec<_> = of(&b, LIST_ITEM).iter().map(|s| words(&b, s)).collect();
        assert_eq!(items, ["one", item, "three"], "{li:?}");
        assert!(
            b.text.contains(&format!("one\n{item}\nthree")),
            "{:?}",
            b.text
        );
    }
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
    assert!(of(&b, BOLD).is_empty());
    assert!(b.text.contains("\n\nSecond\n\nThird\n\n"));
}

/// A heading no span crosses carries the byte it starts on, where a long body may be cut, a link over
/// it alone included; one in a quote, a list item or a link opening ahead of it carries 0. The "é"
/// ahead of them sets bytes apart from characters.
#[test]
fn a_free_heading_carries_its_byte() {
    let b = within(
        "<p>Café.</p><h2>Free</h2><blockquote><h3>Quoted</h3></blockquote>\
         <ul><li><h3>Listed</h3></li></ul><a href=\"/x\"><h3>Own link</h3></a>\
         <a href=\"/y\"><p>Lead.</p><h3>Linked</h3></a>",
    );
    let byte = |s: &Span| {
        let mut units = 0;
        b.text
            .char_indices()
            .find(|(_, c)| {
                units += c.len_utf16() as i32;
                units > s.start
            })
            .unwrap()
            .0 as i32
    };
    for s in of(&b, HEADINGS[1]).into_iter().chain(of(&b, HEADINGS[2])) {
        let free = matches!(words(&b, s), "Free" | "Own link");
        let expected = if free { byte(s) } else { 0 };
        assert_eq!(s.data, expected, "{:?}", words(&b, s));
    }
    assert_ne!(byte(of(&b, HEADINGS[1])[0]), of(&b, HEADINGS[1])[0].start);
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

/// Two rules with nothing left between them are one: a page rules off its share bar, which goes.
#[test]
fn rules_with_nothing_between_are_one() {
    let b = within("<hr><div class=\"share-bar\"><a href=\"#\">Share</a></div><hr><hr>");
    assert_eq!(of(&b, HRULE).len(), 1, "{:?}", b.text);
    let b = within("<hr><p>between</p><hr>");
    assert_eq!(of(&b, HRULE).len(), 2, "{:?}", b.text);
}

#[test]
fn a_rule() {
    let b = within("<hr>");
    assert_eq!(words(&b, of(&b, HRULE)[0]), "\u{2}");
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
    assert!(b.text.contains("see the other post\n"), "{:?}", b.text);
}

#[test]
fn a_link_elsewhere_is_its_words_alone_at_their_size() {
    let b = within("<p>see <a href=\"https://rust-lang.org/learn\">the book</a></p>");
    assert_eq!(words(&b, of(&b, LINK)[0]), "the book");
    assert!(of(&b, LINK_SIZE).is_empty());
    assert!(b.text.contains("see the book\n"), "{:?}", b.text);
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

/// The picture's payload: the size its `width` and `height` state, and its alt.
fn picture(img: &str) -> Picture {
    of(&within(img), IMAGE)[0].picture.clone().unwrap()
}

#[test]
fn a_picture_states_its_size_and_alt() {
    assert_eq!(
        picture("<img src=\"/a.jpg\" width=\"1200\" height=\"800\" alt=\"A cat\">"),
        Picture {
            flags: 0,
            width: 1200,
            height: 800,
            alt: "A cat".into(),
        }
    );
}

#[test]
fn a_size_in_pixels_is_stated() {
    let p = picture("<img src=\"/a.jpg\" width=\" 1200px\" height=\"800px \">");
    assert_eq!((p.width, p.height), (1200, 800));
}

/// Only pixels state a size: a share of the page, `auto` or an `em` says nothing of the picture's.
#[test]
fn a_size_in_other_units_is_not_stated() {
    for (w, h) in [("100%", "auto"), ("50em", "30em")] {
        let p = picture(&format!(
            "<img src=\"/a.jpg\" width=\"{w}\" height=\"{h}\">"
        ));
        assert_eq!((p.width, p.height), (0, 0), "{w} x {h}");
    }
}

/// A fraction of a pixel is rounded, as a page scaling its pictures writes them.
#[test]
fn a_size_in_fractions_of_a_pixel_is_rounded() {
    let p = picture("<img src=\"/a.jpg\" width=\"725\" height=\"543.75\">");
    assert_eq!((p.width, p.height), (725, 544));
}

/// Blogger states the picture's own size in `data-original-width` and `data-original-height`,
/// beside the width it is shown at alone.
#[test]
fn a_bloggers_original_size_is_stated() {
    let p = picture(
        "<img data-original-height=\"1536\" data-original-width=\"2048\" width=\"280\" src=\"/a.jpg\">",
    );
    assert_eq!((p.width, p.height), (2048, 1536));
}

/// A lazy page's placeholder states a pixel or two, not the picture's size.
#[test]
fn a_placeholders_size_is_not_stated() {
    let p = picture("<img src=\"/a.jpg\" width=\"1\" height=\"1\">");
    assert_eq!((p.width, p.height), (0, 0));
    let p = picture("<img src=\"/a.jpg\" width=\"1200\" height=\"15\">");
    assert_eq!((p.width, p.height), (0, 0));
}

/// One side alone gives no shape, so neither is stated.
#[test]
fn one_side_alone_states_no_size() {
    let p = picture("<img src=\"/a.jpg\" width=\"1200\">");
    assert_eq!((p.width, p.height), (0, 0));
}

#[test]
fn a_picture_without_alt_or_size_has_neither() {
    assert_eq!(picture("<img src=\"/a.jpg\">"), Picture::default());
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

/// The page `body` under `og:image` `lead`.
fn led_by(lead: &str, body: &str) -> Blob {
    whole(&format!(
        "<html><head><meta property=\"og:image\" content=\"{lead}\"></head>\
         <body><article>{body}</article></body></html>"
    ))
}

/// The pictures a blob shows, by address.
fn pictures(b: &Blob) -> Vec<String> {
    of(b, IMAGE)
        .iter()
        .map(|s| s.url.clone().unwrap())
        .collect()
}

/// The page's lead picture stays where the article shows it, the reader having no header of its own
/// to show it in.
#[test]
fn keeps_the_lead_picture_where_the_article_shows_it() {
    let b = led_by(
        "https://cdn.example.com/lead.jpg",
        &format!(
            "<figure><img src=\"https://cdn.example.com/lead.jpg\"><figcaption>Photo by Someone</figcaption></figure>\
             <p>{PROSE}</p>"
        ),
    );
    assert_eq!(pictures(&b), ["https://cdn.example.com/lead.jpg"]);
}

#[test]
fn keeps_every_picture_without_a_lead() {
    let b = article(&format!(
        "<img src=\"https://cdn.example.com/a.jpg\"><p>{PROSE}</p>"
    ));
    assert_eq!(pictures(&b), ["https://cdn.example.com/a.jpg"]);
}

/// A picture that states both its sides at 32 pixels or under is an icon and goes: a footnote's
/// arrow back up the page, as Epoch AI ends each note on, a licence's mark. One a pixel over stays,
/// and so does a lazy page's placeholder, whose pixel or two is no size.
#[test]
fn drops_an_icon() {
    let backref = "<a href=\"#fnref-1\" data-footnote-backref class=\"data-footnote-backref\">\
        <img class=\"footnote-icon\" src=\"/icons/arrow.svg\" width=\"16\" height=\"16\" alt=\"Return\"/></a>";
    let b = within(&format!(
        "<ol><li><p>A note. {backref}</p></li><li><p>Another.</p></li></ol>"
    ));
    assert!(pictures(&b).is_empty(), "{:?}", b.text);
    let items: Vec<_> = of(&b, LIST_ITEM).iter().map(|s| words(&b, s)).collect();
    assert_eq!(items, ["A note.", "Another."]);
    for icon in [
        "<img src=\"https://e.com/ring.svg\" width=\"24\" height=\"24\">",
        "<img src=\"https://e.com/cc.svg\" width=\"32px\" height=\"32px\">",
    ] {
        assert!(pictures(&within(icon)).is_empty(), "{icon}");
    }
    for kept in [
        "<img src=\"https://e.com/a.png\" width=\"33\" height=\"33\">",
        "<img src=\"https://e.com/a.png\" width=\"1\" height=\"1\">",
    ] {
        assert_eq!(pictures(&within(kept)), ["https://e.com/a.png"], "{kept}");
    }
}
