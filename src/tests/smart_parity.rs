//! Characterization suite for the `parse.smart` retirement.
//!
//! Pins the FINAL blob output (text, flags, urls) of every smart-affected
//! shape under the production option set, via the real `render_blob` entry
//! point — including its None short-circuit and ASCII fast-path re-render.
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
    opts.parse.strip_invisible = true;
    opts.parse.strip_leading_breaks = true;
    opts
}

/// The production pipeline end to end: None means the caller renders the
/// raw input as plain text (the fast path the JNI shim exposes as null).
fn parity(markdown: &str) -> Option<Vec<u8>> {
    let trimmed = markdown.trim();
    parse_document_zerocopy(trimmed, &opts(), |root| render_blob(root, trimmed))
}

fn render(markdown: &str) -> Vec<u8> {
    parity(markdown).expect("expected Some(blob)")
}

fn text(blob: &[u8]) -> &str {
    let len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    std::str::from_utf8(&blob[8..8 + len]).unwrap()
}

fn is_ascii_flag(blob: &[u8]) -> bool {
    blob[7] & 0x01 != 0
}

fn span_records(blob: &[u8]) -> Vec<[i32; 4]> {
    let text_len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    let count = (i32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) & 0x00FF_FFFF) as usize;
    let base = 8 + ((text_len + 3) & !3);
    (0..count)
        .map(|i| {
            let p = base + i * 16;
            let f = |o: usize| i32::from_le_bytes([blob[p + o], blob[p + o + 1], blob[p + o + 2], blob[p + o + 3]]);
            [f(0), f(4), f(8), f(12)]
        })
        .collect()
}

fn url_data_offset(blob: &[u8]) -> usize {
    let text_len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    let count = (i32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) & 0x00FF_FFFF) as usize;
    8 + ((text_len + 3) & !3) + count * 16
}

/// The url slice a LINK/IMAGE span's packed data field addresses.
fn span_url(blob: &[u8], data: i32) -> &str {
    let base = url_data_offset(blob) + (data >> 12) as usize;
    std::str::from_utf8(&blob[base..base + (data & 0xFFF) as usize]).unwrap()
}

fn first_url_of_type(blob: &[u8], typ: i32) -> String {
    let rec = span_records(blob).into_iter().find(|r| r[2] == typ).expect("span type not found");
    span_url(blob, rec[3]).to_string()
}

fn has_span_type(blob: &[u8], typ: i32) -> bool {
    span_records(blob).iter().any(|r| r[2] == typ)
}

/// Row-major cell markdown of the first TABLE span's payload.
fn table_cells(blob: &[u8]) -> Vec<String> {
    let rec = span_records(blob).into_iter().find(|r| r[2] == TABLE).expect("no TABLE span");
    let mut p = url_data_offset(blob) + (rec[3] - 1) as usize;
    let packed = u32::from_le_bytes([blob[p], blob[p + 1], blob[p + 2], blob[p + 3]]);
    p += 4;
    let (rows, cols) = ((packed >> 16) as usize, (packed & 0xFFFF) as usize);
    (0..rows * cols)
        .map(|_| {
            let len = u16::from_le_bytes([blob[p], blob[p + 1]]) as usize;
            p += 2;
            let s = String::from_utf8(blob[p..p + len].to_vec()).unwrap();
            p += len;
            s
        })
        .collect()
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
    fn apostrophe_only_input_returns_none() {
        assert!(parity("it's a test").is_none());
    }

    #[test]
    fn quote_only_input_returns_none() {
        assert!(parity("don't \"quote\" me").is_none());
    }

    #[test]
    fn ellipsis_only_returns_none() {
        assert!(parity("wait...").is_none());
        assert!(parity("wait....").is_none());
        assert!(parity("wait . . . what").is_none());
    }

    #[test]
    fn under_threshold_runs_return_none() {
        assert!(parity("ok??? fine").is_none());
        assert!(parity("wow!!! nice").is_none());
        assert!(parity("no, really").is_none());
    }

    #[test]
    fn contractions_render_verbatim_and_stay_ascii() {
        // Multiline forces a blob (soft break becomes a space); the quotes
        // themselves must come through as typed, on the byte[] fast path.
        let b = render("it's fine\nsecond line");
        assert_eq!(text(&b), "it's fine second line");
        assert!(is_ascii_flag(&b));
        assert!(span_records(&b).is_empty());
    }

    #[test]
    fn symbols_become_typographic_and_clear_ascii() {
        let b = render("(c) (C) (r) (tm) (TM) 5 +- 2");
        assert_eq!(text(&b), "\u{a9} \u{a9} \u{ae} \u{2122} \u{2122} 5 \u{b1} 2");
        assert!(!is_ascii_flag(&b));
    }

    #[test]
    fn symbol_mixed_case_pairs_not_matched() {
        assert!(parity("(Tm) and (tM) and (Rr)").is_none());
    }

    #[test]
    fn symbols_next_to_multibyte_text() {
        // Non-ASCII input takes the scanning writer from the start; the
        // substitution must keep UTF-16 offsets exact around multibyte
        // neighbours (was typographic.rs's smart_typographic_with_multibyte).
        let b = render("好(c)好 **b**");
        assert_eq!(text(&b), "好\u{a9}好 b");
        assert!(!is_ascii_flag(&b));
        let bold = span_records(&b).into_iter().find(|r| r[2] == BOLD).unwrap();
        assert_eq!((bold[0], bold[1]), (4, 5));
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
        assert_eq!(text(&b), "hi1\n\n\u{1}\n\n1 it's fine");
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
    /// the author typed. None (no blob) and Some with text == input are both
    /// valid deliveries of that.
    fn displays_verbatim(markdown: &str) {
        match parity(markdown) {
            None => {}
            Some(b) => assert_eq!(text(&b), markdown.trim()),
        }
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
        for md in ["a-b", "a--b", "a---b", "a----b", "a-----b", "x ------ y", "a -- b"] {
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
        let cells = table_cells(&b);
        assert_eq!(cells[0], "don't");
        assert_eq!(cells[1], "a--b");
        assert_eq!(cells[2], "\"q\"");
        assert_eq!(cells[3], "c...");
    }
}
