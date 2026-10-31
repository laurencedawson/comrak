//! Characterization suite for the `parse.smart` retirement.
//!
//! Pins the FINAL blob output (text, flags, urls) of every smart-affected
//! shape under the production option set, via the real `render_blob` entry
//! point — including its ASCII fast-path re-render.
//!
//! `invariant`: output that must be byte-identical before and after the
//! retirement. If one of these fails during the migration, the change is
//! wrong, full stop.
//!
//! `transitional`: today's output where a decided or pending step will
//! consciously flip the expectation. Each test names the decision that owns
//! it. When a step lands, its tests are updated in the same commit — never
//! silently.

use crate::blob::render_blob;
use crate::blob::*;
use crate::tests::blob_read::read;
use crate::{Options, parse_document_zerocopy};

/// Mirrors `production_options()` in the JNI shim
/// (library/src/main/rust/src/lib.rs). Keep in sync.
fn opts() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.superscript = true;
    opts.extension.subscript = true;
    #[cfg(feature = "shortcodes")]
    {
        opts.extension.shortcodes = true;
    }
    opts.extension.footnotes = true;
    opts.extension.lemmy_mention = true;
    opts.extension.lemmy_spoiler = true;
    opts.parse.sanitize_input = true;
    opts.parse.smart = false; // deliberately off: rendering policy lives in the blob writer (src/text.rs)
    opts
}

/// The production pipeline end to end: every input answers a blob.
fn parity(markdown: &str) -> Option<Vec<u8>> {
    let trimmed = markdown.trim();
    parse_document_zerocopy(trimmed, &opts(), |root| {
        render_blob(root, trimmed, crate::blob::sites::PLAIN)
    })
}

fn render(markdown: &str) -> Vec<u8> {
    parity(markdown).expect("expected Some(blob)")
}

fn text(blob: &[u8]) -> String {
    read(blob).text
}

/// The input comes back as typed: its words alone, no record, the text ASCII.
fn assert_verbatim(markdown: &str) {
    let b = read(&render(markdown));
    assert_eq!(b.text, markdown, "{markdown:?}");
    assert!(b.spans.is_empty(), "{markdown:?}: {:?}", b.spans);
    assert!(b.ascii(), "{markdown:?}");
}

fn has_span_type(blob: &[u8], typ: i32) -> bool {
    read(blob).spans.iter().any(|s| s.typ == typ)
}

fn first_url_of_type(blob: &[u8], typ: i32) -> String {
    let spans = read(blob).spans;
    spans
        .into_iter()
        .find(|s| s.typ == typ)
        .and_then(|s| s.url)
        .expect("span type not found")
}

mod invariant {
    use super::*;

    #[test]
    fn normalization_passes_commute_and_are_idempotent() {
        // prefer_ascii (Unicode→ASCII) and typographic_symbols (ASCII→glyph)
        // act on disjoint character sets: neither consumes the other's output,
        // so order is irrelevant and reapplying the pipeline is a no-op. If a
        // future edit makes these sets overlap, the passes start fighting the
        // way smart and prefer_ascii once did — this test is the tripwire.
        use crate::text::{prefer_ascii, typographic_symbols};
        let inputs = [
            "it's (c) \u{201c}fine\u{201d} +- 5 \u{2014} (tm) wait\u{2026} x >> 2",
            "(c)(C)(r)(R)(tm)(TM)+-",
            "\u{2018}a\u{2019} \u{2013} b",
        ];
        for input in inputs {
            let a = typographic_symbols(&prefer_ascii(input)).into_owned();
            let b = prefer_ascii(&typographic_symbols(input)).into_owned();
            assert_eq!(a, b, "passes must commute for {input:?}");
            let again = typographic_symbols(&prefer_ascii(&a)).into_owned();
            assert_eq!(a, again, "pipeline must be idempotent for {input:?}");
        }
    }

    #[test]
    fn apostrophe_only_input_renders_verbatim() {
        assert_verbatim("it's a test");
    }

    #[test]
    fn quote_only_input_renders_verbatim() {
        assert_verbatim("don't \"quote\" me");
    }

    #[test]
    fn ellipsis_only_renders_verbatim() {
        assert_verbatim("wait...");
        assert_verbatim("wait....");
        assert_verbatim("wait . . . what");
    }

    #[test]
    fn under_threshold_runs_render_verbatim() {
        assert_verbatim("ok??? fine");
        assert_verbatim("wow!!! nice");
        assert_verbatim("no, really");
    }

    #[test]
    fn contractions_render_verbatim_and_stay_ascii() {
        // Multiline forces a blob (soft break becomes a space); the quotes
        // themselves must come through as typed, on the byte[] fast path.
        let b = render("it's fine\nsecond line");
        assert_eq!(text(&b), "it's fine second line");
        assert!(read(&b).ascii());
        assert!(read(&b).spans.is_empty());
    }

    #[test]
    fn symbols_become_typographic_and_clear_ascii() {
        let b = render("(c) (C) (r) (tm) (TM) 5 +- 2");
        assert_eq!(
            text(&b),
            "\u{a9} \u{a9} \u{ae} \u{2122} \u{2122} 5 \u{b1} 2"
        );
        assert!(!read(&b).ascii());
    }

    #[test]
    fn symbol_mixed_case_pairs_not_matched() {
        assert_verbatim("(Tm) and (tM) and (Rr)");
    }

    #[test]
    fn symbols_next_to_multibyte_text() {
        // Non-ASCII input takes the scanning writer from the start; the
        // substitution must keep UTF-16 offsets exact around multibyte
        // neighbours (was typographic.rs's smart_typographic_with_multibyte).
        let b = render("好(c)好 **b**");
        assert_eq!(text(&b), "好\u{a9}好 b");
        assert!(!read(&b).ascii());
        let bold = read(&b).spans.into_iter().find(|s| s.typ == BOLD).unwrap();
        assert_eq!((bold.start, bold.end), (4, 5));
    }

    #[test]
    fn code_span_exempt_from_all_transforms() {
        let b = render("code `(c) -- \"x\"` end");
        assert_eq!(text(&b), "code (c) -- \"x\" end");
        assert!(has_span_type(&b, CODE));
    }

    #[test]
    fn code_block_exempt_from_all_transforms() {
        let b = render("```\n(c) -- \"quotes\" ----\n```");
        assert_eq!(text(&b), "(c) -- \"quotes\" ----");
        assert!(has_span_type(&b, CODE_BLOCK));
    }

    #[test]
    fn href_untouched_by_dash_transform() {
        let b = render("[a -- \"b\"](https://x.com/a--b)");
        assert_eq!(first_url_of_type(&b, LINK), "https://x.com/a--b");
    }

    #[test]
    fn autolink_display_text_untouched() {
        let b = render("https://x.com/a--b");
        assert_eq!(text(&b), "https://x.com/a--b");
        assert_eq!(first_url_of_type(&b, LINK), "https://x.com/a--b");
    }

    #[test]
    fn spoiler_title_is_raw() {
        let b = render("::: spoiler don't \"stop\"\nbody text\n:::");
        assert!(text(&b).starts_with("don't \"stop\""));
        assert!(has_span_type(&b, LEMMY_SPOILER_TITLE));
    }

    #[test]
    fn footnote_body_quotes_verbatim() {
        let b = render("hi[^1]\n\n[^1]: it's fine");
        assert_eq!(text(&b), "hi1\n\n\u{2}\n\n1 it's fine");
    }

    #[test]
    fn heading_and_blockquote_quotes_verbatim() {
        let b = render("# don't \"quote\"");
        assert_eq!(text(&b), "don't \"quote\"");
        let b = render("> it's \"quoted\"");
        assert_eq!(text(&b), "it's \"quoted\"");
        assert!(has_span_type(&b, QUOTE));
    }

    #[test]
    fn hyphen_block_syntax_is_not_the_dash_transform() {
        // Thematic breaks are block-level CommonMark, parsed before inlines
        // and independent of parse.smart: a line of only hyphens (3+) becomes
        // an HRULE span no matter what the inline dash decision is. The smart
        // transform only ever sees hyphen runs inside a paragraph's text,
        // which can never satisfy the thematic-break line rule.
        let b = render("before\n\n---\n\nafter");
        assert!(has_span_type(&b, HRULE));
        let b = render("before\n\n------\n\nafter");
        assert!(has_span_type(&b, HRULE));
        // Setext heading: `---` under a paragraph line is a level-2 heading,
        // also block-level, also untouched by any dash decision.
        let b = render("title\n---");
        assert_eq!(text(&b), "title");
        assert!(has_span_type(&b, HEADING_2));
        assert!(!has_span_type(&b, HRULE));
    }

    #[test]
    fn video_syntax_alt_text_quotes_verbatim() {
        let b = render("![alt's text](https://x.com/v.mp4)");
        assert_eq!(text(&b), "alt's text (x.com)");
        assert_eq!(first_url_of_type(&b, LINK), "https://x.com/v.mp4");
    }
}

/// Behavior consciously changed by the retirement (2026-08-08 decisions).
/// Each test pins the new contract and names what shipped before it.
mod retired_transforms {
    use super::*;

    /// The deleted transforms' shared contract: the reader sees exactly what
    /// the author typed.
    fn displays_verbatim(markdown: &str) {
        assert_eq!(text(&render(markdown)), markdown.trim());
    }

    // Guillemets deleted: `<<`/`>>` were unpaired unconditional substitutions
    // that corrupted bit shifts (`x >> 2` shipped as `x » 2`).
    #[test]
    fn guillemets_render_verbatim() {
        displays_verbatim("<<hello>> word");
        displays_verbatim("x >> 2 and y << 3");
        let b = render("<<**bold**>>");
        assert_eq!(text(&b), "<<bold>>");
        assert!(has_span_type(&b, BOLD));
    }

    // Dash transform deleted: hyphen runs previously squashed through the
    // em/en grouping (`--`→`-`, `----`→`--`). Now verbatim, matching stock
    // comrak and the CommonMark spec. Block-level `---` (hrule, setext) is
    // pinned separately in `invariant`.
    #[test]
    fn inline_hyphen_runs_render_verbatim() {
        for md in [
            "a-b",
            "a--b",
            "a---b",
            "a----b",
            "a-----b",
            "x ------ y",
            "a -- b",
        ] {
            displays_verbatim(md);
        }
    }

    // Punctuation-run capping deleted: `????`→`???` (and `,,`→`,`) rewrote
    // what the author typed; per the 2026-08-08 decision, we don't.
    #[test]
    fn punctuation_runs_render_verbatim() {
        for md in ["what????", "no!!!!!!", "hm,,,,", "ok??? fine"] {
            displays_verbatim(md);
        }
    }

    // Quotes deleted: table cells previously carried curly/en-dash/ellipsis
    // forms (cells bypass prefer_ascii); they now carry the typed ASCII.
    // Symbols still convert in cells' downstream re-render, not in the payload.
    #[test]
    fn table_cells_carry_typed_ascii() {
        let b = render("| don't | a--b |\n|---|---|\n| \"q\" | c... |");
        let b = read(&b);
        let table = b
            .span_iter()
            .find(|s| s.typ == TABLE)
            .expect("no TABLE span");
        let (_, _, cells) = b.table(table.data);
        assert_eq!(cells[0], "don't");
        assert_eq!(cells[1], "a--b");
        assert_eq!(cells[2], "\"q\"");
        assert_eq!(cells[3], "c...");
    }
}
