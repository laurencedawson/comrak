//! Tests for a blob translated (`blob::translate`): the text it hands out, one string a line for
//! each of the blob's, and the blob it writes back from an answer, each record where its line, its
//! kept part or its link's markers landed.

use crate::benchmarks::production_options;
use crate::blob::sites::{HACKER_NEWS, LEMMY, PLAIN};
use crate::blob::translate::{translated, translation_text};
use crate::blob::{
    APP_LINK, BOLD, CODE, CODE_BLOCK, HEADINGS, IMAGE, LEMMY_SPOILER_CONTENT, LEMMY_SPOILER_TITLE,
    LINK, LINK_SIZE, LIST_ITEM, QUOTE, READER_BLOCK, SUBSCRIPT_SIZE, TABLE, render_article_blob,
    render_blob, render_html_blob,
};
use crate::parse_document_zerocopy;
use crate::tests::blob_read::{Blob, read};

fn md(markdown: &str, site: u8) -> Vec<u8> {
    let opts = production_options();
    parse_document_zerocopy(markdown, &opts, |root| render_blob(site, root, markdown))
        .expect("a blob")
}

fn article(body: &str) -> Vec<u8> {
    let html = format!("<!DOCTYPE html><html><body><article>{body}</article></body></html>");
    render_article_blob(&html, "https://example.com/post").expect("an article")
}

/// Every shape a blob holds but a link's domain, which the text leaves out: blocks, a list, a
/// quote over lines, code, a code block, a table, a picture, a rule, hard breaks.
const SHAPES: &str = "# Getting started\n\n\
    Before you begin, make sure you have **everything**:\n\n\
    - Node 20 or newer\n  - check it with `node -v`\n- about ten minutes\n\n\
    > Tip: you can skip the build step\n> if you only want to read.\n\n\
    ```\nnpm install\nnpm test\n```\n\n\
    | a | b |\n|---|---|\n| 1 | 2 |\n\n\
    ![a picture](https://example.com/a.png)\n\n\
    ---\n\n\
    The first line of a poem  \nand the last.";

fn samples() -> Vec<Vec<u8>> {
    vec![
        md(SHAPES, PLAIN),
        md(
            "before\n\n::: spoiler A title\nthe hidden words\n:::\n\nafter",
            LEMMY,
        ),
        md(
            "hi @someone@lemmy.world, see https://example.com/x for it",
            LEMMY,
        ),
        render_html_blob(
            HACKER_NEWS,
            "<p>Words <i>here</i></p><p>Second <code>x</code></p>",
            None,
        )
        .expect("a blob"),
        article(
            "<h1>Heading</h1><p>A paragraph with <a href=\"/rel\">a link</a> in it, long enough to \
             read as the article's own words.</p><blockquote><p>Quoted words</p></blockquote>\
             <ul><li>first</li><li>second</li></ul><h2>Later</h2><p>More of the article's words, \
             and enough of them to keep it.</p>",
        ),
    ]
}

/// What the doubling answer does to a line: every word written twice, the markers as they were.
fn doubled(text: &str) -> String {
    let mut out = String::new();
    let mut word = String::new();
    for c in text.chars().chain(std::iter::once(' ')) {
        if c.is_alphabetic() {
            word.push(c);
        } else {
            out.push_str(&word);
            out.push_str(&word);
            word.clear();
            out.push(c);
        }
    }
    out.pop();
    out
}

fn types(b: &Blob, kept: impl Fn(i32) -> bool) -> Vec<i32> {
    let mut t: Vec<i32> = b.span_iter().map(|s| s.typ).filter(|&t| kept(t)).collect();
    t.sort_unstable();
    t
}

fn words(b: &Blob, typ: i32) -> Vec<String> {
    let utf16: Vec<u16> = b.text.encode_utf16().collect();
    b.span_iter()
        .filter(|s| s.typ == typ)
        .map(|s| String::from_utf16_lossy(&utf16[s.start as usize..s.end as usize]))
        .collect()
}

/// Why: a translation that changes nothing must give the blob back as it was, every record, byte
/// and bit, or the rebuild is wrong before any translator is involved.
#[test]
fn the_text_given_back_unchanged_is_the_same_blob() {
    for blob in samples() {
        let text = translation_text(&blob).unwrap();
        assert_eq!(translated(&blob, &text).unwrap(), blob, "{text:?}");
    }
}

/// Why: what the translator sees is the words alone, a link's between its markers, everything to
/// keep as a number, and none of the writer's own domain after a link.
#[test]
fn the_text_marks_links_and_kept_parts_and_leaves_the_domain_out() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    assert_eq!(
        read(&blob).text,
        "See the docs (example.com) and code here."
    );
    assert_eq!(
        translation_text(&blob).unwrap(),
        "See [1]the docs[/1] and {2} here."
    );
}

/// Why: a link whose words are an address is kept whole, since a translator moves words into it.
#[test]
fn a_link_whose_words_are_its_address_is_kept() {
    let blob = md(
        "More at [example.com](https://example.com/more) today.",
        PLAIN,
    );
    assert_eq!(translation_text(&blob).unwrap(), "More at {1} today.");
}

/// Why: the lines go out one for one, blank ones and a code block's included, so every block's
/// records find their lines again.
#[test]
fn the_text_has_a_line_for_every_line() {
    let blob = md(SHAPES, PLAIN);
    let text = translation_text(&blob).unwrap();
    assert_eq!(
        text.split('\n').count(),
        read(&blob).text.split('\n').count()
    );
    assert!(
        text.contains("\n\n{2}\n{3}\n\n"),
        "a code block a kept part a line: {text:?}"
    );
}

/// Why: a translation is longer or shorter everywhere; every block must still stand over its own
/// words, the domain gone and emphasis inside a sentence with it.
#[test]
fn longer_words_keep_every_block_over_its_words() {
    for blob in samples() {
        let out = read(&translated(&blob, &doubled(&translation_text(&blob).unwrap())).unwrap());
        let before = read(&blob);
        let blocks = |t: i32| !(BOLD..=SUBSCRIPT_SIZE).contains(&t);
        assert_eq!(
            types(&out, blocks),
            types(&before, blocks),
            "{:?}",
            out.text
        );
    }
    let out = read(
        &translated(
            &md(SHAPES, PLAIN),
            &doubled(&translation_text(&md(SHAPES, PLAIN)).unwrap()),
        )
        .unwrap(),
    );
    assert_eq!(words(&out, HEADINGS[0]), ["GettingGetting startedstarted"]);
    assert_eq!(words(&out, CODE), ["node -v"]);
    assert_eq!(words(&out, CODE_BLOCK), ["npm install\nnpm test"]);
    assert_eq!(words(&out, TABLE), ["View Table"]);
    assert_eq!(words(&out, IMAGE).len(), 1);
    assert!(words(&out, QUOTE)[0].starts_with("TipTip: youyou"));
    assert!(words(&out, LIST_ITEM).contains(&"NodeNode 20 oror newernewer".to_string()));
    assert!(
        words(&out, BOLD).is_empty(),
        "emphasis inside a sentence is dropped"
    );
}

/// Why: a translator puts a link and code where its language wants them; each keeps its own words.
#[test]
fn a_reordered_answer_lands_each_link_and_kept_part_on_its_own() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    let out = read(&translated(&blob, "{2} hier und siehe [1]die Doku[/1].").unwrap());
    assert_eq!(out.text, "code hier und siehe die Doku.");
    assert_eq!(words(&out, LINK), ["die Doku"]);
    assert_eq!(words(&out, LINK_SIZE), ["die Doku"]);
    assert_eq!(words(&out, CODE), ["code"]);
    assert_eq!(
        out.span_iter()
            .find(|s| s.typ == LINK)
            .unwrap()
            .url
            .as_deref(),
        Some("https://example.com/a")
    );
}

/// Why: a marker a translator broke costs that link alone; its words stay and nothing is left over.
#[test]
fn a_broken_link_marker_drops_the_link_and_keeps_its_words() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    let out = read(&translated(&blob, "Siehe [1]die Doku und {2} hier.").unwrap());
    assert_eq!(out.text, "Siehe die Doku und code hier.");
    assert!(words(&out, LINK).is_empty() && words(&out, LINK_SIZE).is_empty());
    assert_eq!(words(&out, CODE), ["code"]);
}

/// Why: what must be kept is shown even when the translator dropped its number.
#[test]
fn a_lost_kept_part_goes_at_the_end_of_its_line() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    let out = read(&translated(&blob, "Siehe [1]die Doku[/1] hier.").unwrap());
    assert_eq!(out.text, "Siehe die Doku hier.code");
    assert_eq!(words(&out, CODE), ["code"]);
}

/// Why: tolerance for what translators do to a marker: spaces inside it, full-width brackets.
#[test]
fn markers_are_read_with_spaces_and_full_width_brackets() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    let out = read(&translated(&blob, "[ 1 ]ドキュメント［/1］と｛2｝を参照").unwrap());
    assert_eq!(out.text, "ドキュメントとcodeを参照");
    assert_eq!(words(&out, LINK), ["ドキュメント"]);
    assert!(!out.ascii());
}

/// Why: a line of code is code whatever the translator made of its number.
#[test]
fn a_line_of_kept_parts_alone_is_written_as_it_was() {
    let blob = md("Words\n\n```\nnpm test\n```", PLAIN);
    let out = read(&translated(&blob, "Wörter\n\nnpm prüfen").unwrap());
    assert_eq!(out.text, "Wörter\n\nnpm test");
    assert_eq!(words(&out, CODE_BLOCK), ["npm test"]);
}

/// Why: lines that do not match leave no way to place a block's records, so the caller is told.
#[test]
fn an_answer_of_another_count_of_lines_is_none() {
    let blob = md("one\n\ntwo", PLAIN);
    assert!(translated(&blob, "eins zwei").is_none());
    assert!(
        translated(&blob, "eins\n\nzwei\n").is_some(),
        "a trailing newline is no line"
    );
}

/// Why: a spoiler's content pairs with the record just before it, its title, which must survive.
#[test]
fn a_spoiler_title_still_sorts_just_before_its_content() {
    let blob = md(
        "before\n\n::: spoiler A title\nthe hidden words\n:::\n\nafter",
        LEMMY,
    );
    let out = read(&translated(&blob, &doubled(&translation_text(&blob).unwrap())).unwrap());
    let i = out
        .spans
        .iter()
        .position(|s| s.typ == LEMMY_SPOILER_CONTENT)
        .unwrap();
    assert_eq!(out.spans[i - 1].typ, LEMMY_SPOILER_TITLE);
    assert_eq!(words(&out, LEMMY_SPOILER_TITLE), ["AA titletitle"]);
}

/// Why: a reader body is cut into sections at a free heading's byte; a translation moves the bytes,
/// so each must be the heading's byte in the new text, its tiles still over every line.
#[test]
fn a_reader_body_keeps_its_tiles_and_its_headings_bytes() {
    let blob = article(
        "<p>Some café words, long enough to stand as the article itself here.</p><h2>Later</h2><p>More words of the article.</p>",
    );
    let out_bytes = translated(&blob, &doubled(&translation_text(&blob).unwrap())).unwrap();
    let out = read(&out_bytes);
    let before = read(&blob);
    assert_eq!(
        out.span_iter().filter(|s| s.typ == READER_BLOCK).count(),
        before.span_iter().filter(|s| s.typ == READER_BLOCK).count()
    );
    let utf16: Vec<u16> = out.text.encode_utf16().collect();
    for h in out
        .span_iter()
        .filter(|s| (HEADINGS[0]..=HEADINGS[5]).contains(&s.typ) && s.data != 0)
    {
        let byte = String::from_utf16_lossy(&utf16[..h.start as usize]).len() as i32;
        assert_eq!(h.data, byte, "{:?}", out.text);
    }
    assert!(
        out.span_iter()
            .any(|s| s.data != 0 && (HEADINGS[0]..=HEADINGS[5]).contains(&s.typ))
    );
}

/// Why: the view reads the bytes in place only while every one is ASCII, so the bit must follow the
/// new text, both ways.
#[test]
fn the_ascii_bit_follows_the_new_text() {
    let blob = md("plain words", PLAIN);
    assert!(read(&blob).ascii());
    assert!(!read(&translated(&blob, "Wörter").unwrap()).ascii());
    let accented = md("café", PLAIN);
    assert!(read(&translated(&accented, "coffee").unwrap()).ascii());
}

/// Why: a mention is the site's own page, kept as written.
#[test]
fn a_mention_is_kept() {
    let blob = md("hi @someone@lemmy.world, how are you", LEMMY);
    let text = translation_text(&blob).unwrap();
    assert!(text.starts_with("hi {1}"), "{text:?}");
    let out = read(&translated(&blob, "hallo {1}, wie geht es").unwrap());
    assert_eq!(words(&out, APP_LINK).len(), 1);
}

/// Why: a reader's blob carries no domain after a link, and its links are sent as words.
#[test]
fn a_reader_link_is_sent_as_words() {
    let blob = article(
        "<p>A paragraph with <a href=\"https://other.org/x\">a link</a> in it, long enough to read as the article itself.</p>",
    );
    assert!(translation_text(&blob).unwrap().contains("[1]a link[/1]"));
}
