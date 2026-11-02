//! Tests for a reader body's headings, which the writer works out for every walk: the page's title,
//! the first block written where it is a heading no span crosses, and where a long body may be cut,
//! the byte each other free heading starts on.
//!
//! - `title`    : the first block a heading is the title, its line's tile a title's; anything else
//!   first, a heading in a quote, or another site, has none
//! - `sections` : each free heading after a gap carries its byte, in UTF-8 whatever the text's
//!   characters, as the view cuts sections; a crossed one carries none
//! - `article`  : its title the same title, its byline the authors line, its sections as before
//! - `rules`    : a rule just before a cut is the list's to draw, so the reader writes none there

use crate::benchmarks::production_options;
use crate::blob::sites::{CHAN, HACKER_NEWS, LEMMY, LOBSTERS, PLAIN, READER};
use crate::blob::translate::{build_translated_blob, mark_for_translation};
use crate::blob::{
    HEADING_BYLINE, HEADING_TITLE, HEADINGS, HRULE, READER_BLOCK, reader_block,
    render_article_blob, render_blob,
};
use crate::parse_document_zerocopy;
use crate::tests::blob_read::{Blob, Span, read};

fn md(markdown: &str, site: u8) -> Blob {
    let opts = production_options();
    read(
        &parse_document_zerocopy(markdown, &opts, |root| render_blob(site, root, markdown))
            .expect("a blob"),
    )
}

fn page(markdown: &str) -> Blob {
    md(markdown, READER)
}

/// Prose long enough that Readability takes the block holding it for the article.
const PROSE: &str = "This paragraph carries the article, with commas, clauses, and enough words \
     that the scoring counts it as content rather than chrome, which is what a reader wants.";

fn article(head: &str, body: &str) -> Blob {
    let html = format!(
        "<!DOCTYPE html><html><head>{head}</head><body><article>{body}</article></body></html>"
    );
    read(&render_article_blob(&html, "https://example.com/post").expect("an article"))
}

fn of(b: &Blob, typ: i32) -> Vec<&Span> {
    b.span_iter().filter(|s| s.typ == typ).collect()
}

fn words(b: &Blob, s: &Span) -> String {
    let utf16: Vec<u16> = b.text.encode_utf16().collect();
    String::from_utf16_lossy(&utf16[s.start as usize..s.end as usize])
}

fn headings(b: &Blob) -> Vec<&Span> {
    b.span_iter()
        .filter(|s| HEADINGS.contains(&s.typ))
        .collect()
}

/// The words of every heading the view cuts a section at: data that is a byte, the heading's own
/// words starting there in the text's UTF-8, and a blank line's tile the record just before it, as
/// `Blob.firstRecord` and `Blob.sectionByteStart` read them.
fn cuts(b: &Blob) -> Vec<String> {
    let mut out = Vec::new();
    for (i, s) in b.spans.iter().enumerate() {
        if !HEADINGS.contains(&s.typ) || s.data == 0 {
            continue;
        }
        let shown = words(b, s);
        assert!(
            b.text.as_bytes()[s.data as usize..].starts_with(shown.as_bytes()),
            "{shown:?} at byte {} of {:?}",
            s.data,
            b.text
        );
        let gap = &b.spans[i - 1];
        assert!(
            gap.typ == READER_BLOCK && gap.data == reader_block::GAP && gap.end == s.start,
            "{shown:?} cut after {gap:?}"
        );
        out.push(shown);
    }
    out
}

// ===== The title =====

/// Why: a page the app ships opens on its `#` title, which the view centres at the largest size and
/// which opens no section of its own, being the first's.
#[test]
fn a_page_opening_on_a_heading_has_it_as_its_title() {
    let b = page("# Privacy Policy\n\nWords.\n\n# What stays\n\nMore.");
    let title = of(&b, HEADING_TITLE);
    assert_eq!(title.len(), 1);
    assert_eq!(words(&b, title[0]), "Privacy Policy");
    assert_eq!((title[0].start, title[0].data), (0, 0));
    assert_eq!(cuts(&b), ["What stays"]);
}

/// Why: the title's line is sized as a title's, not as the level it was written at.
#[test]
fn the_titles_line_is_a_titles() {
    let b = page("# Privacy Policy\n\nWords.");
    let tile = of(&b, READER_BLOCK)[0];
    assert_eq!((tile.start, tile.data), (0, reader_block::TITLE));
}

/// Why: a title is whatever heading a page opens on, so a page written from `##` is titled too.
#[test]
fn any_level_opening_the_page_is_its_title() {
    let b = page("### A small title\n\nWords.");
    assert_eq!(words(&b, of(&b, HEADING_TITLE)[0]), "A small title");
    assert!(headings(&b).is_empty());
}

/// Why: only the first block can be a title; a page that opens on words has none, and its first
/// heading opens a section.
#[test]
fn a_page_opening_on_words_has_no_title() {
    let b = page("Some words first.\n\n# A heading\n\nMore.");
    assert!(of(&b, HEADING_TITLE).is_empty());
    assert_eq!(cuts(&b), ["A heading"]);
}

/// Why: a heading inside a quote at the page's start is the quote's, crossed by it, so no title.
#[test]
fn a_heading_in_a_quote_opening_the_page_is_no_title() {
    let b = page("> # Quoted\n> words\n\nAfter.");
    assert!(of(&b, HEADING_TITLE).is_empty());
    assert_eq!(headings(&b).len(), 1);
    assert_eq!(headings(&b)[0].data, 0);
}

/// Why: a page's title may hold styles of its own; the rule is where the heading starts, not which
/// span was pushed first.
#[test]
fn a_styled_title_is_still_the_title() {
    let b = page("# The **bold** title\n\nWords.");
    assert_eq!(words(&b, of(&b, HEADING_TITLE)[0]), "The bold title");
}

/// Why: a comment is never a page, so one opening on a heading keeps its level and carries no byte
/// on any other site.
#[test]
fn another_sites_heading_is_no_title_and_no_cut() {
    for site in [PLAIN, HACKER_NEWS, CHAN, LEMMY, LOBSTERS] {
        let b = md("# A heading\n\nWords.\n\n# Another\n\nMore.", site);
        assert!(of(&b, HEADING_TITLE).is_empty(), "site {site}");
        assert_eq!(of(&b, HEADINGS[0]).len(), 2, "site {site}");
        assert!(headings(&b).iter().all(|s| s.data == 0), "site {site}");
        assert!(of(&b, HEADING_BYLINE).is_empty(), "site {site}");
    }
}

// ===== Sections =====

/// Why: a page's top level headings are where it is cut into sections, as an article's are, each
/// at its own heading whatever its level.
#[test]
fn a_pages_free_headings_are_its_cuts() {
    let b = page(
        "# Title\n\nWords.\n\n---\n\n# One\n\nWords.\n\n## A part\n\nWords.\n\n# Two\n\nWords.",
    );
    assert_eq!(cuts(&b), ["One", "A part", "Two"]);
}

/// Why: a section's byte is in the text's UTF-8 while a span's start is in UTF-16 units; accents,
/// CJK and a surrogate pair ahead of a heading must leave its byte on its first word.
#[test]
fn a_cut_is_the_headings_byte_past_any_characters() {
    let b = page("# Café\n\nnaïve 日本語 🙂 words.\n\n# Über\n\nmore 🙂🙂.\n\n# Last\n\nend.");
    assert!(!b.ascii());
    assert_eq!(cuts(&b), ["Über", "Last"]);
}

/// Why: a heading a quote or a list holds is no place to cut, the container crossing it.
#[test]
fn a_heading_a_container_holds_is_no_cut() {
    let b = page("Words.\n\n> # In a quote\n> more\n\n- # In a list\n\n# Free\n\nWords.");
    assert_eq!(cuts(&b), ["Free"]);
    for s in headings(&b) {
        if words(&b, s) != "Free" {
            assert_eq!(s.data, 0, "{:?}", words(&b, s));
        }
    }
}

// ===== The article =====

/// Why: an article's title is the page's title as a markdown page's is, the writer's one rule, and
/// its authors are its byline under it, neither cutting a section.
#[test]
fn an_articles_title_and_byline() {
    let b = article(
        r#"<title>A title long enough to keep</title><script type="application/ld+json">{"@type":"NewsArticle","author":{"@type":"Person","name":"Jane Doe"}}</script>"#,
        &format!("<p>{PROSE}</p><h2>Later</h2><p>{PROSE}</p>"),
    );
    assert_eq!(
        words(&b, of(&b, HEADING_TITLE)[0]),
        "A title long enough to keep"
    );
    assert_eq!(words(&b, of(&b, HEADING_BYLINE)[0]), "Jane Doe");
    assert_eq!(of(&b, HEADING_BYLINE)[0].data, 0);
    assert_eq!(cuts(&b), ["Later"]);
    // The title and byline are one run of lines, its tile a title's
    let tile = of(&b, READER_BLOCK)[0];
    assert_eq!((tile.start, tile.data), (0, reader_block::TITLE));
}

/// Why: an article's sections cut past its non-ASCII words as a page's do, the byte worked out by
/// the writer and no longer by the walk.
#[test]
fn an_articles_cuts_past_its_characters() {
    let b = article(
        "<title>Café culture, long enough to keep</title>",
        &format!(
            "<p>{PROSE} naïve 🙂</p><h2>Über</h2><p>{PROSE}</p><h3>Smaller 日本</h3><p>{PROSE}</p>"
        ),
    );
    assert_eq!(cuts(&b), ["Über", "Smaller 日本"]);
}

/// Why: an article with no title of its own opens on whatever comes first; a heading there is its
/// title by the same rule.
#[test]
fn an_untitled_article_opening_on_a_heading_has_it_as_its_title() {
    let b = article(
        "",
        &format!("<h2>Opening</h2><p>{PROSE}</p><h2>Next</h2><p>{PROSE}</p>"),
    );
    assert_eq!(words(&b, of(&b, HEADING_TITLE)[0]), "Opening");
    assert_eq!(cuts(&b), ["Next"]);
}

// ===== A translation =====

/// Why: a page translated and given back unchanged must be the same blob, its title still the
/// title and every cut on the same heading, the rebuild reworking a cut's byte as the writer does.
#[test]
fn a_page_given_back_unchanged_is_the_same_blob() {
    let markdown =
        "# Café policy\n\nnaïve 🙂 words.\n\n---\n\n# Über\n\nmore.\n\n## A part\n\nend.";
    let opts = production_options();
    let blob = parse_document_zerocopy(markdown, &opts, |root| render_blob(READER, root, markdown))
        .unwrap();
    let b = read(&blob);
    let (records, bytes) = (0..b.spans.len(), 0..b.text.len());
    let text = mark_for_translation(&blob, records.clone(), bytes.clone()).unwrap();
    assert_eq!(
        build_translated_blob(&blob, records, bytes, &text).unwrap(),
        blob,
        "{text:?}"
    );
}

// ===== Rules =====

/// Why: the reader's list draws a rule between its sections, so a page's own rule just before a cut
/// would stand a second one there; the reader writes none, and the cut is as it was.
#[test]
fn a_rule_just_before_a_cut_is_left_to_the_list() {
    let b = page("Words first.\n\n---\n\n## Next\n\nMore words.");
    assert!(of(&b, HRULE).is_empty(), "{:?}", b.text);
    assert!(!b.text.contains('\u{2}'), "{:?}", b.text);
    assert_eq!(b.text, "Words first.\n\nNext\n\nMore words.");
    assert_eq!(cuts(&b), ["Next"]);
}

/// Why: a rule with words after it, or the page's last block, stands inside a section, where only
/// the body draws it.
#[test]
fn a_rule_inside_a_section_stays() {
    let b = page("Words first.\n\n---\n\nMore words.\n\n## Next\n\nEnd.\n\n---");
    assert_eq!(of(&b, HRULE).len(), 2, "{:?}", b.text);
}

/// Why: a rule in a quote before a heading in it is the quote's, no section's edge.
#[test]
fn a_rule_in_a_quote_before_a_heading_stays() {
    let b = page("Words first.\n\n> ---\n> ## Quoted\n\nMore words.");
    assert_eq!(of(&b, HRULE).len(), 1, "{:?}", b.text);
}

/// Why: a comment is never cut, so its rule before a heading is the only one drawn.
#[test]
fn a_comments_rule_before_a_heading_stays() {
    let b = md("Words first.\n\n---\n\n## Next\n\nMore words.", LEMMY);
    assert_eq!(of(&b, HRULE).len(), 1, "{:?}", b.text);
}

/// Why: an article's `<hr>` before an `<h2>` is the same rule before the same cut.
#[test]
fn an_articles_rule_just_before_a_cut_is_left_to_the_list() {
    let b = article(
        "",
        &format!("<p>{PROSE}</p><hr>\n<h2>Next</h2><p>{PROSE}</p>"),
    );
    assert!(of(&b, HRULE).is_empty(), "{:?}", b.text);
    assert_eq!(cuts(&b), ["Next"]);
}

/// Why: words between an article's rule and its heading keep the rule, inside the section above.
#[test]
fn an_articles_rule_before_words_stays() {
    let b = article(
        "",
        &format!("<p>{PROSE}</p><hr>Some words.<h2>Next</h2><p>{PROSE}</p>"),
    );
    assert_eq!(of(&b, HRULE).len(), 1, "{:?}", b.text);
}
