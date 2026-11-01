//! Tests for a blob translated (`blob::translate`): the text it hands out, one string a line for
//! each of the blob's, and the blob it writes back from an answer, each record where its line, its
//! kept part or its link's markers landed.

use crate::benchmarks::production_options;
use crate::blob::sites::{HACKER_NEWS, LEMMY, PLAIN};
use std::ops::Range;

use crate::blob::translate::{build_translated_blob, mark_for_translation};
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
        let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
        assert_eq!(
            build_translated_blob(&blob, all(&blob), all_bytes(&blob), &text).unwrap(),
            blob,
            "{text:?}"
        );
    }
}

/// Why: a text beyond ASCII, accents and characters two UTF-16 units wide among its records, given
/// back unchanged, must be the same blob: the records' ends are moved to bytes and back.
#[test]
fn a_text_beyond_ascii_given_back_unchanged_is_the_same_blob() {
    for blob in [
        md(
            "Café 🎉 then **bold** and [a link](https://example.com/x), 𝔉𝔦𝔫.\n\n> quoted naïveté 🎉 `code` end",
            PLAIN,
        ),
        article(
            "<p>Some café words, long enough 🎉 to stand as the article itself here.</p><h2>Later 𝔉</h2><p>More <a href=\"/x\">words</a> of the article.</p>",
        ),
    ] {
        let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
        assert_eq!(
            build_translated_blob(&blob, all(&blob), all_bytes(&blob), &text).unwrap(),
            blob,
            "{text:?}"
        );
    }
}

/// Why: characters two UTF-16 units wide before a link and inside the answer move every record after
/// them; each must still stand over its own words.
#[test]
fn records_after_wide_characters_land_on_their_words() {
    let blob = md(
        "🎉 See [the docs](https://example.com/a) 🎉 and `code` here.",
        PLAIN,
    );
    assert_eq!(
        mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap(),
        "🎉 See [1]the docs[/1] 🎉 and {2} here."
    );
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "🎉🎉 Siehe [1]die Dokumente 𝔇[/1] und {2} hier.",
        )
        .unwrap(),
    );
    assert_eq!(out.text, "🎉🎉 Siehe die Dokumente 𝔇 und code hier.");
    assert_eq!(words(&out, LINK), ["die Dokumente 𝔇"]);
    assert_eq!(words(&out, CODE), ["code"]);
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
        mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap(),
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
    assert_eq!(
        mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap(),
        "More at {1} today."
    );
}

/// Why: the lines go out one for one, blank ones and a code block's included, so every block's
/// records find their lines again.
#[test]
fn the_text_has_a_line_for_every_line() {
    let blob = md(SHAPES, PLAIN);
    let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
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
        let out = read(
            &build_translated_blob(
                &blob,
                all(&blob),
                all_bytes(&blob),
                &doubled(&mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap()),
            )
            .unwrap(),
        );
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
        &build_translated_blob(
            &md(SHAPES, PLAIN),
            all(&md(SHAPES, PLAIN)),
            all_bytes(&md(SHAPES, PLAIN)),
            &doubled(
                &mark_for_translation(
                    &md(SHAPES, PLAIN),
                    all(&md(SHAPES, PLAIN)),
                    all_bytes(&md(SHAPES, PLAIN)),
                )
                .unwrap(),
            ),
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
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "{2} hier und siehe [1]die Doku[/1].",
        )
        .unwrap(),
    );
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
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "Siehe [1]die Doku und {2} hier.",
        )
        .unwrap(),
    );
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
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "Siehe [1]die Doku[/1] hier.",
        )
        .unwrap(),
    );
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
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "[ 1 ]ドキュメント［/1］と｛2｝を参照",
        )
        .unwrap(),
    );
    assert_eq!(out.text, "ドキュメントとcodeを参照");
    assert_eq!(words(&out, LINK), ["ドキュメント"]);
    assert!(!out.ascii());
}

/// Why: a line of code is code whatever the translator made of its number.
#[test]
fn a_line_of_kept_parts_alone_is_written_as_it_was() {
    let blob = md("Words\n\n```\nnpm test\n```", PLAIN);
    let out = read(
        &build_translated_blob(&blob, all(&blob), all_bytes(&blob), "Wörter\n\nnpm prüfen")
            .unwrap(),
    );
    assert_eq!(out.text, "Wörter\n\nnpm test");
    assert_eq!(words(&out, CODE_BLOCK), ["npm test"]);
}

/// Why: lines that do not match leave no way to place a block's records, so the caller is told.
#[test]
fn an_answer_of_another_count_of_lines_is_none() {
    let blob = md("one\n\ntwo", PLAIN);
    assert!(build_translated_blob(&blob, all(&blob), all_bytes(&blob), "eins zwei").is_none());
    assert!(
        build_translated_blob(&blob, all(&blob), all_bytes(&blob), "eins\n\nzwei\n").is_some(),
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
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            &doubled(&mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap()),
        )
        .unwrap(),
    );
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
    let out_bytes = build_translated_blob(
        &blob,
        all(&blob),
        all_bytes(&blob),
        &doubled(&mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap()),
    )
    .unwrap();
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
    assert!(
        !read(&build_translated_blob(&blob, all(&blob), all_bytes(&blob), "Wörter").unwrap())
            .ascii()
    );
    let accented = md("café", PLAIN);
    assert!(
        read(
            &build_translated_blob(&accented, all(&accented), all_bytes(&accented), "coffee")
                .unwrap()
        )
        .ascii()
    );
}

/// Why: a mention is the site's own page, kept as written.
#[test]
fn a_mention_is_kept() {
    let blob = md("hi @someone@lemmy.world, how are you", LEMMY);
    let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
    assert!(text.starts_with("hi {1}"), "{text:?}");
    let out = read(
        &build_translated_blob(
            &blob,
            all(&blob),
            all_bytes(&blob),
            "hallo {1}, wie geht es",
        )
        .unwrap(),
    );
    assert_eq!(words(&out, APP_LINK).len(), 1);
}

/// Why: a reader's blob carries no domain after a link, and its links are sent as words.
#[test]
fn a_reader_link_is_sent_as_words() {
    let blob = article(
        "<p>A paragraph with <a href=\"https://other.org/x\">a link</a> in it, long enough to read as the article itself.</p>",
    );
    assert!(
        mark_for_translation(&blob, all(&blob), all_bytes(&blob))
            .unwrap()
            .contains("[1]a link[/1]")
    );
}

/// Every record of `blob`: the whole of it, as a body is shown.
fn all(blob: &[u8]) -> Range<usize> {
    0..read(blob).spans.len()
}

/// All of `blob`'s text, in bytes.
fn all_bytes(blob: &[u8]) -> Range<usize> {
    0..read(blob).text.len()
}

/// Each section of `blob` as the view cuts it: its records from a heading carrying its byte to the
/// record before the next, the tile of the blank line ahead of it, the first from 0; its text's
/// bytes from that heading's byte to the next's, the newlines before it stepped back over.
fn sections(blob: &[u8]) -> Vec<(Range<usize>, Range<usize>)> {
    let b = read(blob);
    let bytes = b.text.as_bytes();
    let opens: Vec<usize> = (1..b.spans.len())
        .filter(|&i| (HEADINGS[0]..=HEADINGS[5]).contains(&b.spans[i].typ) && b.spans[i].data != 0)
        .collect();
    let mut out = Vec::new();
    let (mut first, mut start) = (0, 0);
    for &next in &opens {
        let mut end = b.spans[next].data as usize;
        while bytes[end - 1] == b'\n' {
            end -= 1;
        }
        out.push((first..next - 1, start..end));
        first = next;
        start = b.spans[next].data as usize;
    }
    out.push((first..b.spans.len(), start..bytes.len()));
    out
}

/// Why: a long body is translated a section at a time, told which records and bytes it is, as the
/// view is told to show one; each given back unchanged must be that section alone, its text the
/// section's, its records moved to start at 0, the heading opening it cutting nothing.
#[test]
fn a_section_given_back_unchanged_is_that_section_alone() {
    let blob = article(
        "<p>Some café words, long enough to stand as the article itself here.</p><h2>Later</h2>\
         <p>More words of the article, with <a href=\"https://other.org/x\">a link</a>.</p><h2>Last</h2>\
         <ul><li>one</li><li>two</li></ul>",
    );
    let whole = read(&blob);
    let sections = sections(&blob);
    assert_eq!(sections.len(), 3, "{sections:?}");
    for (records, bytes) in sections {
        let text = mark_for_translation(&blob, records.clone(), bytes.clone()).unwrap();
        let part =
            read(&build_translated_blob(&blob, records.clone(), bytes.clone(), &text).unwrap());
        assert_eq!(part.text, whole.text[bytes]);
        assert_eq!(part.spans.len(), records.len());
        assert_eq!(part.spans[0].start, 0);
        assert!(
            part.span_iter()
                .all(|s| s.data == 0 || !(HEADINGS[0]..=HEADINGS[5]).contains(&s.typ))
        );
    }
}

/// Why: records or bytes the blob does not hold are a mistake the caller is told of, not a read
/// past them.
#[test]
fn a_part_past_the_blobs_is_none() {
    let blob = md("plain words", PLAIN);
    assert!(mark_for_translation(&blob, 0..5, all_bytes(&blob)).is_none());
    assert!(mark_for_translation(&blob, all(&blob), 0..99).is_none());
}

/// `blob` built back from `answer`, all of it, read.
fn translated(blob: &[u8], answer: &str) -> Blob {
    read(&build_translated_blob(blob, all(blob), all_bytes(blob), answer).expect("a blob"))
}

/// The url of each link of `b`, in the text's order.
fn urls(b: &Blob) -> Vec<&str> {
    b.span_iter()
        .filter(|s| s.typ == LINK)
        .filter_map(|s| s.url.as_deref())
        .collect()
}

/// Why: a page's own citation, `[1]`, comes back from the translator as it went; it is the page's
/// words, not link 1's marker, so the link still stands over its own words and the citation stays.
#[test]
fn a_citation_in_the_text_is_not_a_link_marker() {
    let blob = md(
        "A claim[1] made by [the docs](https://example.com/a).",
        PLAIN,
    );
    let out = translated(
        &blob,
        &mark_for_translation(&blob, all(&blob), all_bytes(&blob))
            .unwrap()
            .replace("A claim", "Eine Behauptung")
            .replace("made by", "von")
            .replace("the docs", "der Doku"),
    );
    assert_eq!(out.text, "Eine Behauptung[1] von der Doku.");
    assert_eq!(words(&out, LINK), ["der Doku"]);
}

/// Why: braces round a number in the page's words, `{1}`, are words too, not kept part 1, which
/// keeps its own place.
#[test]
fn braces_in_the_text_are_not_a_kept_part() {
    let blob = md("Set {1} to `x` first.", PLAIN);
    let out = translated(
        &blob,
        &mark_for_translation(&blob, all(&blob), all_bytes(&blob))
            .unwrap()
            .replace("Set", "Setze")
            .replace(" to ", " auf ")
            .replace("first", "zuerst"),
    );
    assert_eq!(out.text, "Setze {1} auf x zuerst.");
    assert_eq!(words(&out, CODE), ["x"]);
}

/// Why: a translator into Arabic, Persian or Hindi may write a marker's number in its own digits.
#[test]
fn markers_in_another_scripts_digits_are_read() {
    let blob = md(
        "See [the docs](https://example.com/a) and `code` here.",
        PLAIN,
    );
    let out = translated(&blob, "انظر [١]الوثائق[/١] و{٢} هنا.");
    assert_eq!(out.text, "انظر الوثائق وcode هنا.");
    assert_eq!(words(&out, LINK), ["الوثائق"]);
    assert_eq!(words(&out, CODE), ["code"]);
}

/// Why: an answer may come back with Windows line ends; its lines are the same lines, no `\r` left
/// in the text.
#[test]
fn an_answer_with_crlf_lines_is_read_line_for_line() {
    let blob = md("First line\n\nSecond [link](https://example.com/a)", PLAIN);
    let out = translated(&blob, "Erste Zeile\r\n\r\nZweite [1]Verweis[/1]\r\n");
    assert_eq!(out.text, "Erste Zeile\n\nZweite Verweis");
    assert_eq!(words(&out, LINK), ["Verweis"]);
}

/// Why: a translator that writes a link's markers twice gives the link the words it put first
/// between them, and the rest are plain words.
#[test]
fn a_link_marked_twice_takes_its_first_words() {
    let blob = md("See [the docs](https://example.com/a) here.", PLAIN);
    let out = translated(&blob, "Siehe [1]die[/1] [1]Doku[/1] hier.");
    assert_eq!(out.text, "Siehe die Doku hier.");
    assert_eq!(words(&out, LINK), ["die"]);
}

/// Why: a kept part's number moved onto another line is no marker there; the part stays on its own
/// line, at that line's end where its own number went missing.
#[test]
fn a_kept_part_moved_to_another_line_stays_on_its_own() {
    let blob = md("First `a` line.\n\nSecond `b` line.", PLAIN);
    let out = translated(&blob, "Erste {2} Zeile.\n\nZweite {1} Zeile.");
    assert_eq!(out.text, "Erste  Zeile.a\n\nZweite  Zeile.b");
    assert_eq!(words(&out, CODE), ["a", "b"]);
}

/// Why: a line of words given back empty loses its words, not what it keeps.
#[test]
fn a_line_given_back_empty_keeps_its_kept_parts() {
    let blob = md("Some words `code` here.", PLAIN);
    let out = translated(&blob, "");
    assert_eq!(out.text, "code");
    assert_eq!(words(&out, CODE), ["code"]);
}

/// Why: two links a translator swaps each keep their own url, over their own words.
#[test]
fn two_links_swapped_keep_their_own_urls() {
    let blob = md(
        "See [one](https://example.com/1) and [two](https://example.com/2).",
        PLAIN,
    );
    let out = translated(&blob, "Siehe [2]zwei[/2] und [1]eins[/1].");
    assert_eq!(words(&out, LINK), ["zwei", "eins"]);
    assert_eq!(
        urls(&out),
        ["https://example.com/2", "https://example.com/1"]
    );
}

/// Why: a link over a hard break has no markers, its words on two lines; translated, it is dropped
/// whole, never left over half its words.
#[test]
fn a_link_over_a_hard_break_is_dropped_whole() {
    let blob = md("[first line  \nsecond](https://example.com/a) after", PLAIN);
    assert_eq!(
        mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap(),
        "first line\nsecond after"
    );
    let out = translated(&blob, "erste Zeile\nzweite danach");
    assert_eq!(out.text, "erste Zeile\nzweite danach");
    assert!(words(&out, LINK).is_empty());
}

/// Why: code inside a link's words is kept inside the link, both landing where the answer put them.
#[test]
fn code_inside_a_link_keeps_both() {
    let blob = md("See [the `npm` docs](https://example.com/a).", PLAIN);
    assert_eq!(
        mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap(),
        "See [1]the {2} docs[/1]."
    );
    let out = translated(&blob, "Siehe [1]die {2} Doku[/1].");
    assert_eq!(words(&out, LINK), ["die npm Doku"]);
    assert_eq!(words(&out, CODE), ["npm"]);
}

/// Why: whatever the answer, every record the blob keeps lies in the new text, and the records sort
/// as the writer sorts them, which spoiler pairing and the view's reads rely on.
#[test]
fn every_record_lies_in_the_new_text_in_the_writers_order() {
    for blob in samples() {
        let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
        let emptied: String = text
            .split('\n')
            .map(|l| {
                if l.chars().any(char::is_alphabetic) {
                    ""
                } else {
                    l
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        for answer in [text.clone(), doubled(&text), emptied] {
            let out = translated(&blob, &answer);
            let len = out.text.encode_utf16().count() as i32;
            let spans: Vec<_> = out.span_iter().collect();
            for s in &spans {
                assert!(
                    0 <= s.start && s.start < s.end && s.end <= len,
                    "{answer:?}"
                );
            }
            for w in spans.windows(2) {
                let key = |s: &crate::tests::blob_read::Span| (s.start, -s.end, s.typ);
                assert!(key(w[0]) <= key(w[1]), "{answer:?}");
            }
        }
    }
}

/// Why: a translation moves the text, never the url data: every picture, link and table reads the
/// same payload after as before.
#[test]
fn pictures_links_and_tables_read_their_payloads_after() {
    for blob in samples() {
        let before = read(&blob);
        let text = mark_for_translation(&blob, all(&blob), all_bytes(&blob)).unwrap();
        let after = translated(&blob, &doubled(&text));
        let payloads = |b: &Blob, typ: i32| -> Vec<(Option<String>, i32)> {
            b.span_iter()
                .filter(|s| s.typ == typ)
                .map(|s| (s.url.clone(), if typ == TABLE { s.data } else { 0 }))
                .collect()
        };
        for typ in [IMAGE, LINK, TABLE] {
            assert_eq!(payloads(&after, typ), payloads(&before, typ), "type {typ}");
        }
        let pictures = |b: &Blob| {
            b.span_iter()
                .filter_map(|s| s.picture.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(pictures(&after), pictures(&before));
        assert_eq!(after.url_data, before.url_data);
    }
}
