//! Tests for the blob renderer (`comrak::blob::render_blob`).
//!
//! Organized into sections:
//! - `format`    — blob byte layout, header, padding, UTF-16 positions, sort
//! - `inline`    — bold/italic/strike/super/sub/code/spoiler/breaks/shortcodes
//! - `links`     — explicit links, domain suffixes, autolinks
//! - `images`    — markdown images, autolink-to-image, link-wrapping-image
//! - `block`     — headings, lists, task lists, blockquotes, code blocks, tables
//! - `footnotes` — footnote refs and definitions
//! - `edge`      — unicode, pathological inputs, empty, deep nesting

use crate::blob::{BlobWriter, LIST_ITEM, QUOTE, visit};
use crate::parse_document_zerocopy;
use crate::Options;

// ── helpers ──────────────────────────────────────────────────────────────

fn test_opts() -> Options<'static> {
    let mut opts = Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.superscript = true;
    opts.extension.subscript = true;
    opts.extension.spoiler = true;
    opts.extension.tasklist = true;
    opts.extension.shortcodes = true;
    opts.extension.footnotes = true;
    opts.extension.lemmy_mention = true;
    opts.extension.lemmy_spoiler = true;
    opts.parse.strip_invisible = true;
    opts.parse.smart = true;
    opts
}

/// Render markdown through the full parse+visit pipeline, returning the
/// BlobWriter so tests can inspect internals directly.
fn render_raw(markdown: &str) -> BlobWriter {
    let opts = test_opts();
    parse_document_zerocopy(markdown.trim(), &opts, |root| {
        let mut out = BlobWriter::new(256);
        visit(root, &mut out, 0, 0, 0);
        out
    })
}

/// `render_raw` plus footnote append and pending-newline flush — what most
/// rendering assertions want.
fn render_test(markdown: &str) -> BlobWriter {
    let mut out = render_raw(markdown);
    out.append_footnotes();
    out.clear_pending();
    out
}

/// Render + serialize to the final blob byte representation.
fn blob_bytes(markdown: &str) -> Vec<u8> {
    render_raw(markdown).into_blob()
}

/// Text section of a raw blob (after the 8-byte header).
fn blob_text(blob: &[u8]) -> &str {
    let text_len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    std::str::from_utf8(&blob[8..8 + text_len]).unwrap()
}

fn blob_span_count(blob: &[u8]) -> usize {
    i32::from_le_bytes([blob[4], blob[5], blob[6], blob[7]]) as usize
}

/// Decoded view of one span in the BlobWriter (not the serialized blob).
struct SpanView {
    typ: i32,
    start: usize,
    end: usize,
    data: i32,
    url: Option<String>,
}

impl SpanView {
    fn indent(&self) -> i32 { self.data >> 16 }
    fn number(&self) -> i32 { self.data & 0xFFFF }
}

struct SpanIter<'a> {
    spans: &'a [i32],
    url_data: &'a [u8],
    idx: usize,
}

impl<'a> Iterator for SpanIter<'a> {
    type Item = SpanView;
    fn next(&mut self) -> Option<Self::Item> {
        let base = self.idx * 4;
        if base + 3 >= self.spans.len() { return None; }
        self.idx += 1;
        let typ = self.spans[base + 2];
        let raw_data = self.spans[base + 3];
        // The data field packs offset(20)|len(12) for URL-carrying spans;
        // LIST_ITEM and QUOTE reuse the slot for (indent<<16)|number and depth.
        let url_len = (raw_data & 0xFFF) as usize;
        let offset = (raw_data >> 12) as usize;
        let url = if url_len > 0 && typ != LIST_ITEM && typ != QUOTE {
            Some(String::from_utf8_lossy(&self.url_data[offset..offset + url_len]).into_owned())
        } else { None };
        Some(SpanView {
            start: self.spans[base] as usize,
            end: self.spans[base + 1] as usize,
            typ,
            data: raw_data,
            url,
        })
    }
}

trait BlobWriterExt {
    fn span_iter(&self) -> SpanIter<'_>;
}

impl BlobWriterExt for BlobWriter {
    fn span_iter(&self) -> SpanIter<'_> {
        SpanIter { spans: &self.spans, url_data: &self.url_data, idx: 0 }
    }
}

// ── format ───────────────────────────────────────────────────────────────

mod format {
    use super::*;
    use crate::blob::*;

    /// Empty writer produces a valid 8-byte header-only blob.
    #[test]
    fn empty_writer_produces_valid_blob() {
        let out = BlobWriter::new(0);
        let blob = out.into_blob();
        assert_eq!(blob.len(), 8);
        assert_eq!(blob_text(&blob), "");
        assert_eq!(blob_span_count(&blob), 0);
    }

    /// Text is padded to 4-byte alignment before span data. Covers text
    /// lengths 1–5 so every alignment offset is hit.
    #[test]
    fn four_byte_alignment_padding() {
        for md in &["a", "ab", "abc", "abcd", "abcde"] {
            let blob = blob_bytes(md);
            let text_len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
            let span_count = blob_span_count(&blob);
            let span_start = 8 + text_len + (4 - text_len % 4) % 4;
            if span_count > 0 {
                assert!(blob.len() >= span_start + span_count * 16);
            }
        }
    }

    /// URL data field packs as offset(20)|len(12); roundtrip recovers bytes.
    #[test]
    fn url_offset_len_packing_roundtrips() {
        let result = render_test("[click](https://example.com)");
        let span = result.span_iter().find(|s| s.typ == LINK).unwrap();
        assert_eq!(span.url.as_deref(), Some("https://example.com"));
        let url_len = (span.data & 0xFFF) as usize;
        let offset = (span.data >> 12) as usize;
        let unpacked = std::str::from_utf8(&result.url_data[offset..offset + url_len]).unwrap();
        assert_eq!(unpacked, "https://example.com");
    }

    /// 12-bit length field limits URLs to 4095 bytes.
    #[test]
    fn url_truncation_at_max_len() {
        let long_path = "a".repeat(4200);
        let md = format!("[click](https://example.com/{long_path})");
        let result = render_test(&md);
        let span = result.span_iter().find(|s| s.typ == LINK).unwrap();
        assert!(span.url.unwrap().len() <= 4095);
    }

    /// `clear_pending()` drops unflushed queued newlines.
    #[test]
    fn pending_newlines_dont_materialize_without_text() {
        let mut out = BlobWriter::new(64);
        out.write_text("hello");
        out.nl(2);
        out.clear_pending();
        assert_eq!(out.text(), "hello");
    }

    /// `nl(n)` keeps only the maximum pending count across successive calls.
    #[test]
    fn queue_newlines_takes_max() {
        let mut out = BlobWriter::new(64);
        out.write_text("a");
        out.nl(1);
        out.nl(3);
        out.nl(2);
        out.write_text("b");
        assert_eq!(out.text(), "a\n\n\nb");
    }

    /// Span positions use UTF-16 code units (emoji = 2 units, surrogate pair).
    #[test]
    fn span_positions_use_utf16() {
        let result = render_test("😀**bold**");
        let bold = result.span_iter().find(|s| s.typ == BOLD).unwrap();
        assert_eq!(bold.start, 2);
        assert_eq!(bold.end, 6);
    }

    /// ASCII span positions are 1:1 with byte offsets.
    #[test]
    fn utf16_positions_ascii() {
        let result = render_test("**hello**");
        let bold = result.span_iter().find(|s| s.typ == BOLD).unwrap();
        assert_eq!(bold.start, 0);
        assert_eq!(bold.end, 5);
    }

    /// CJK characters count as 1 UTF-16 unit each (3 UTF-8 bytes but BMP).
    #[test]
    fn utf16_positions_cjk() {
        let result = render_test("中文**粗体**");
        let bold = result.span_iter().find(|s| s.typ == BOLD).unwrap();
        assert_eq!(bold.start, 2);
        assert_eq!(bold.end, 4);
    }

    /// Mixed ASCII/emoji/CJK UTF-16 positions: H=1 i=1 😀=2 中=1 → bold at 5.
    #[test]
    fn utf16_positions_mixed() {
        let result = render_test("Hi😀中**b**");
        let bold = result.span_iter().find(|s| s.typ == BOLD).unwrap();
        assert_eq!(bold.start, 5);
        assert_eq!(bold.end, 6);
    }

    /// `into_blob()` sorts spans by start position so Android layout can
    /// early-exit on the sorted stream.
    #[test]
    fn span_sorting_in_blob() {
        let out = render_test("**bold *and italic* text**");
        let blob = out.into_blob();
        let text_len = i32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
        let span_count = blob_span_count(&blob);
        let span_start = 8 + text_len + (4 - text_len % 4) % 4;
        let mut prev_start = -1i32;
        for i in 0..span_count {
            let offset = span_start + i * 16;
            let start = i32::from_le_bytes([blob[offset], blob[offset + 1], blob[offset + 2], blob[offset + 3]]);
            assert!(start >= prev_start, "spans out of order: {start} after {prev_start}");
            prev_start = start;
        }
        assert!(span_count >= 2);
    }

    /// Zero-length spans (start == end) are skipped at emission time.
    #[test]
    fn zero_length_spans_skipped() {
        let mut out = BlobWriter::new(64);
        let pos = out.pos();
        out.span(BOLD, pos);
        assert_eq!(out.span_iter().count(), 0);
    }

    /// `span_url()` with empty URL still emits the span (if non-zero length)
    /// but stores no URL bytes.
    #[test]
    fn span_url_empty_url() {
        let mut out = BlobWriter::new(64);
        out.write_text("text");
        out.span_url(LINK, 0, "");
        let span = out.span_iter().find(|s| s.typ == LINK).unwrap();
        assert_eq!(span.url, None);
    }

    /// `span_data()` skips zero-length spans (same check as `span()`).
    #[test]
    fn span_data_start_ge_end_skipped() {
        let mut out = BlobWriter::new(64);
        let pos = out.pos();
        out.span_data(LIST_ITEM, pos, 42);
        assert_eq!(out.span_iter().count(), 0);
    }

    /// Multiple URLs pack sequentially in url_data with correct offsets.
    #[test]
    fn multiple_urls_correct_offsets() {
        let mut out = BlobWriter::new(64);
        out.write_text("aaa");
        out.span_url(LINK, 0, "https://first.com");
        out.write_text("bbb");
        out.span_url(LINK, 3, "https://second.com");
        let urls: Vec<_> = out.span_iter().filter_map(|s| s.url).collect();
        assert_eq!(urls, vec!["https://first.com", "https://second.com"]);
    }

    /// Zero-length spans are excluded from the serialized blob too.
    #[test]
    fn blob_spans_exclude_zero_length() {
        let mut out = BlobWriter::new(64);
        out.write_text("hello");
        out.span(BOLD, 0);
        let pos = out.pos();
        out.span(ITALIC, pos);
        let blob = out.into_blob();
        assert_eq!(blob_span_count(&blob), 1);
    }

    /// Trailing newlines are trimmed from the final blob text.
    #[test]
    fn trailing_newlines_trimmed() {
        let blob = blob_bytes("This kid:\n\n");
        assert_eq!(blob_text(&blob), "This kid:");
    }

    /// Nested blockquotes don't leave trailing newlines, and internal blanks
    /// between nesting levels are preserved.
    #[test]
    fn blockquote_blob_newlines() {
        let text = String::from_utf8(blob_text(&blob_bytes("> A\n>> B\n>>> C\n\nAfter")).as_bytes().to_vec()).unwrap();
        assert!(text.contains("A\n\nB"), "missing newline A→B: {text:?}");
        assert!(text.contains("B\n\nC"), "missing newline B→C: {text:?}");
        assert!(text.contains("C\n\n") && text.contains("After"));
        assert!(!text.ends_with('\n'));

        let blob2 = blob_bytes("> A\n>> B\n>>> C");
        assert!(!blob_text(&blob2).ends_with('\n'));
    }

    /// All serialized span start/end positions are within text bounds.
    #[test]
    fn span_positions_within_text_bounds() {
        let blob = blob_bytes("**bold** and *italic*");
        let text = blob_text(&blob);
        let text_utf16_len = text.encode_utf16().count() as i32;
        let text_len = text.len();
        let span_count = blob_span_count(&blob);
        let span_start = 8 + text_len + (4 - text_len % 4) % 4;
        for i in 0..span_count {
            let offset = span_start + i * 16;
            let start = i32::from_le_bytes([blob[offset], blob[offset+1], blob[offset+2], blob[offset+3]]);
            let end = i32::from_le_bytes([blob[offset+4], blob[offset+5], blob[offset+6], blob[offset+7]]);
            assert!(start >= 0 && start <= text_utf16_len);
            assert!(end >= 0 && end <= text_utf16_len);
            assert!(start <= end);
        }
    }
}

// ── inline ───────────────────────────────────────────────────────────────

mod inline {
    use super::*;
    use crate::blob::*;

    #[test]
    fn bold_italic() {
        let result = render_test("***bold and italic***");
        assert!(result.span_iter().any(|s| s.typ == BOLD));
        assert!(result.span_iter().any(|s| s.typ == ITALIC));
    }

    #[test]
    fn strikethrough() {
        let result = render_test("~~deleted~~");
        assert!(result.span_iter().any(|s| s.typ == STRIKETHROUGH));
        assert_eq!(result.text(), "deleted");
    }

    #[test]
    fn superscript() {
        let result = render_test("^superscript^");
        assert!(result.span_iter().any(|s| s.typ == SUPERSCRIPT));
        assert!(result.span_iter().any(|s| s.typ == SUPERSCRIPT_SIZE));
        assert_eq!(result.text(), "superscript");
    }

    #[test]
    fn subscript() {
        let result = render_test("~subscript~");
        assert!(result.span_iter().any(|s| s.typ == SUBSCRIPT));
        assert!(result.span_iter().any(|s| s.typ == SUBSCRIPT_SIZE));
        assert_eq!(result.text(), "subscript");
    }

    #[test]
    fn inline_code() {
        let result = render_test("`code`");
        assert!(result.span_iter().any(|s| s.typ == CODE));
        assert_eq!(result.text(), "code");
    }

    #[test]
    fn spoiler_rendered() {
        let result = render_test("||secret||");
        assert!(result.span_iter().any(|s| s.typ == SPOILER));
        assert_eq!(result.text(), "secret");
    }

    /// Spoiler markers inside code spans / code blocks must not produce a SPOILER span.
    #[test]
    fn spoiler_in_code() {
        let result = render_test("Use `||spoiler||` for spoilers");
        assert!(result.span_iter().any(|s| s.typ == CODE));
        assert!(!result.span_iter().any(|s| s.typ == SPOILER));

        let result = render_test("```\n||spoiler||\n```");
        assert!(result.span_iter().any(|s| s.typ == CODE_BLOCK));
        assert!(!result.span_iter().any(|s| s.typ == SPOILER));
    }

    /// Trailing double-space forces a line break.
    #[test]
    fn hard_line_break() {
        let result = render_test("line one  \nline two");
        assert_eq!(result.text(), "line one\nline two");
    }

    /// Soft break renders as space outside a blockquote, newline inside.
    #[test]
    fn soft_break() {
        let result = render_test("line one\nline two");
        assert_eq!(result.text(), "line one line two");

        let result = render_test("> line one\n> line two");
        assert!(result.text().contains("line one\nline two"));
    }

    /// Emoji shortcodes resolve to Unicode; unknown shortcodes pass through.
    #[test]
    fn shortcodes() {
        let out = render_test("Hello :smile: world");
        assert_eq!(out.text(), "Hello 😄 world");

        let out = render_test(":thumbsup: :heart:");
        assert_eq!(out.text(), "👍 ❤\u{fe0f}");

        let out = render_test("Hello :notarealcode: world");
        assert_eq!(out.text(), "Hello :notarealcode: world");
    }
}

// ── links ────────────────────────────────────────────────────────────────

mod links {
    use super::*;
    use crate::blob::*;

    #[test]
    fn domain_suffix_appended() {
        let result = render_test("[click here](https://wikipedia.org)");
        assert!(result.text().starts_with("click here (wikipedia.org)"));
    }

    #[test]
    fn domain_suffix_strips_www() {
        let result = render_test("[click here](https://www.wikipedia.org/wiki/Rust)");
        assert!(result.text().starts_with("click here (wikipedia.org)"));
    }

    /// No suffix when link text already contains the domain.
    #[test]
    fn domain_suffix_suppressed_when_text_contains_domain() {
        let result = render_test("[wikipedia.org](https://wikipedia.org)");
        assert!(!result.text().starts_with("wikipedia.org (wikipedia.org)"));
    }

    /// Bare autolink as sole content renders as LINK.
    #[test]
    fn bare_autolink() {
        let result = render_test("https://example.com/page");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Lemmy community mentions become LINK spans.
    #[test]
    fn lemmy_community_mention() {
        let result = render_test("!linux@lemmy.ml");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Lemmy user mentions don't append a domain suffix (text already contains it).
    #[test]
    fn lemmy_user_mention_no_suffix() {
        let result = render_test("@user@lemmy.ml");
        assert_eq!(result.text(), "@user@lemmy.ml");
    }

    /// Explicit link to an image URL still gets domain suffix and stays LINK.
    #[test]
    fn explicit_link_to_image_url_stays_link() {
        let result = render_test("[click](https://i.imgur.com/abc.jpg)");
        assert_eq!(result.text(), "click (i.imgur.com)");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Link whose visible text equals the URL: no suffix.
    #[test]
    fn suffix_suppressed_when_text_equals_url() {
        let result = render_test("[https://example.com/path](https://example.com/path)");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Domain match is case-insensitive.
    #[test]
    fn domain_suffix_case_insensitive() {
        let result = render_test("[Wikipedia.Org](https://wikipedia.org)");
        assert!(!result.text().starts_with("Wikipedia.Org (wikipedia.org)"));
    }

    /// Domain suffix renders wrapped in a LINK_SIZE span.
    #[test]
    fn link_size_span_wraps_domain_suffix() {
        let result = render_test("[click](https://example.com)");
        assert!(result.span_iter().any(|s| s.typ == LINK_SIZE));
    }

    /// Explicit descriptive link stays LINK.
    #[test]
    fn descriptive_link_stays_link() {
        let result = render_test("[click here](https://example.com/page)");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Bare URL preceded by other content stays LINK.
    #[test]
    fn bare_url_at_end_with_preceding_content_stays_link() {
        let result = render_test("Check this out\n\nhttps://example.com/page");
        assert!(result.span_iter().any(|s| s.typ == LINK));
        assert!(result.text().contains("https://example.com/page"));
    }

    /// Bare URL mid-document stays LINK.
    #[test]
    fn trailing_url_not_at_end_stays_link() {
        let result = render_test("Check this out: https://example.com/page\n\nMore text here.");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Non-image bare URL stays LINK (not IMAGE).
    #[test]
    fn non_image_url_stays_link() {
        let result = render_test("https://example.com/page");
        assert!(result.span_iter().any(|s| s.typ == LINK
            && s.url.as_deref() == Some("https://example.com/page")));
        assert!(!result.span_iter().any(|s| s.typ == IMAGE));
    }
}

// ── images ───────────────────────────────────────────────────────────────

mod images {
    use super::*;
    use crate::blob::*;

    /// `![alt](url)` produces IMAGE span with URL.
    #[test]
    fn markdown_image() {
        let result = render_test("![alt](https://example.com/img.png)");
        assert!(result.span_iter().any(|s| s.typ == IMAGE && s.url.as_deref() == Some("https://example.com/img.png")));
        assert_eq!(result.text(), "\n\u{FFFC}");
    }

    /// Bare autolink to an image URL becomes IMAGE, not LINK.
    #[test]
    fn bare_image_url_to_image() {
        let result = render_test("https://i.imgur.com/abc.jpg");
        assert!(result.span_iter().any(|s| s.typ == IMAGE));
        assert!(!result.span_iter().any(|s| s.typ == LINK));
    }

    /// DDG-proxied image URL (with query that looks like an image) becomes IMAGE.
    #[test]
    fn ddg_proxied_image_becomes_image() {
        let result = render_test("https://external-content.duckduckgo.com/iu/?u=http%3A%2F%2Fwiki.example.com%2Fimg%2FLink.png%2Frev%2Flatest%3Fcb%3D20090331010533&f=1");
        assert!(result.span_iter().any(|s| s.typ == IMAGE));
        assert!(!result.span_iter().any(|s| s.typ == LINK));
    }

    /// `[text](image-url)` with visible text stays LINK — only bare autolinks convert.
    #[test]
    fn explicit_link_to_image_stays_link() {
        let result = render_test("[click](https://i.imgur.com/abc.jpg)");
        assert!(result.span_iter().any(|s| s.typ == LINK));
        assert!(!result.span_iter().any(|s| s.typ == IMAGE));
    }

    /// `[![](img)](url)` becomes IMAGE using the image's URL, not the link's.
    #[test]
    fn link_wrapping_image_becomes_image() {
        let result = render_test("[![alt](https://i.imgur.com/abc.jpg)](https://example.com)");
        assert!(result.span_iter().any(|s| s.typ == IMAGE
            && s.url.as_deref() == Some("https://i.imgur.com/abc.jpg")));
        assert!(!result.span_iter().any(|s| s.typ == LINK));
    }

    /// Link wrapping an image must not append a domain suffix.
    #[test]
    fn link_wrapping_image_no_domain_suffix() {
        let result = render_test("[![](https://lemmy.ca/pictrs/image/abc.png)](https://lemmy.ca/post/123)");
        assert!(!result.text().contains("(lemmy.ca)"));
        assert!(result.span_iter().any(|s| s.typ == IMAGE));
    }

    /// Known image hosts trigger IMAGE via autolink detection.
    #[test]
    fn known_hosts_become_image() {
        for input in &[
            "https://i.redd.it/abc123",
            "https://preview.redd.it/abc123?width=640&format=png",
            "https://i.imgur.com/abc123",
            "https://example.com/img.jpg#ref",
        ] {
            let result = render_test(input);
            assert!(result.span_iter().any(|s| s.typ == IMAGE), "failed for {input}");
        }
    }

    /// Imgur album/gallery paths are excluded from image detection.
    #[test]
    fn imgur_exclusions_stay_link() {
        for input in &[
            "https://i.imgur.com/a/abc123",
            "https://i.imgur.com/gallery/abc123",
        ] {
            let result = render_test(input);
            assert!(result.span_iter().any(|s| s.typ == LINK), "failed for {input}");
            assert!(!result.span_iter().any(|s| s.typ == IMAGE), "unexpected IMAGE for {input}");
        }
    }

    /// Consecutive inline images on one line → at most one blank line between them.
    #[test]
    fn consecutive_images_single_blank_line() {
        let result = render_test("![](https://example.com/a.png) ![](https://example.com/b.png)");
        assert!(!result.text().contains("\n\n\n"));
    }

    /// Images separated by a blank line → still no triple newline.
    #[test]
    fn consecutive_images_separate_paragraphs() {
        let result = render_test("![](https://example.com/a.png)\n\n![](https://example.com/b.png)");
        assert!(!result.text().contains("\n\n\n"));
    }

    /// Image after text has a blank line before it, not three.
    #[test]
    fn image_after_text() {
        let result = render_test("hello ![](https://example.com/a.png)");
        let text = result.text();
        assert!(text.contains("\n\n"));
        assert!(!text.contains("\n\n\n"));
    }
}

// ── block ────────────────────────────────────────────────────────────────

mod block {
    use super::*;
    use crate::blob::*;

    /// All 6 heading levels produce their span type; H1 also bold.
    #[test]
    fn headings_all_levels() {
        let levels = [("# H1", HEADING_1), ("## H2", HEADING_2), ("### H3", HEADING_3),
                      ("#### H4", HEADING_4), ("##### H5", HEADING_5), ("###### H6", HEADING_6)];
        for (md, expected) in levels {
            let result = render_test(md);
            assert!(result.span_iter().any(|s| s.typ == expected), "missing {expected} for {md}");
        }
        let h1 = render_test("# H1");
        assert!(h1.span_iter().any(|s| s.typ == BOLD));
    }

    #[test]
    fn heading_with_inline_formatting() {
        let result = render_test("# **Bold Heading**");
        assert!(result.span_iter().any(|s| s.typ == HEADING_1));
        assert!(result.span_iter().any(|s| s.typ == BOLD));
        assert_eq!(result.text(), "Bold Heading");
    }

    #[test]
    fn nested_lists() {
        let result = render_test("- Item 1\n  - Nested 1\n  - Nested 2\n- Item 2");
        assert_eq!(result.text(), "Item 1\nNested 1\nNested 2\nItem 2");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items.len(), 4);
        assert_eq!((items[0].indent(), items[0].number()), (0, 0));
        assert_eq!((items[1].indent(), items[1].number()), (1, 0));
        assert_eq!((items[2].indent(), items[2].number()), (1, 0));
        assert_eq!((items[3].indent(), items[3].number()), (0, 0));

        let result = render_test("- Level 1\n  - Level 2\n    - Level 3");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items[0].indent(), 0);
        assert_eq!(items[1].indent(), 1);
        assert_eq!(items[2].indent(), 2);
    }

    /// Ordered lists preserve numbering including custom start.
    #[test]
    fn ordered_lists() {
        let result = render_test("1. First\n2. Second\n3. Third");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].number(), 1);
        assert_eq!(items[2].number(), 3);

        let result = render_test("5. Fifth\n6. Sixth");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items[0].number(), 5);
        assert_eq!(items[1].number(), 6);
    }

    /// Bullet list containing ordered sub-list.
    #[test]
    fn mixed_list_nesting() {
        let result = render_test("- Bullet\n  1. Ordered\n  2. Second");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items.len(), 3);
        assert_eq!((items[0].indent(), items[0].number()), (0, 0));
        assert_eq!((items[1].indent(), items[1].number()), (1, 1));
        assert_eq!((items[2].indent(), items[2].number()), (1, 2));
    }

    #[test]
    fn list_then_paragraph() {
        let result = render_test("- Item 1\n- Item 2\n\nParagraph after");
        assert!(result.text().starts_with("Item 1"));
        assert!(result.text().ends_with("Paragraph after"));
    }

    #[test]
    fn list_item_with_inline_formatting() {
        let result = render_test("- **bold** item");
        assert!(result.span_iter().any(|s| s.typ == BOLD));
        assert!(result.span_iter().any(|s| s.typ == LIST_ITEM));
        assert_eq!(result.text(), "bold item");
    }

    /// Task list checkbox encoded in number field: unchecked=0xFFFE, checked=0xFFFF.
    #[test]
    fn task_list_basic() {
        let result = render_test("- [ ] unchecked\n- [x] checked");
        assert_eq!(result.text(), "unchecked\nchecked");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!((items[0].indent(), items[0].number()), (0, 0xFFFE));
        assert_eq!((items[1].indent(), items[1].number()), (0, 0xFFFF));
    }

    #[test]
    fn task_list_mixed_with_regular_items() {
        let result = render_test("- regular\n- [ ] unchecked\n- [x] checked");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!(items[0].number(), 0);
        assert_eq!(items[1].number(), 0xFFFE);
        assert_eq!(items[2].number(), 0xFFFF);
    }

    #[test]
    fn task_list_nested() {
        let result = render_test("- [ ] parent\n  - [x] child");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert_eq!((items[0].indent(), items[0].number()), (0, 0xFFFE));
        assert_eq!((items[1].indent(), items[1].number()), (1, 0xFFFF));
    }

    #[test]
    fn task_list_markers_stripped() {
        assert_eq!(render_test("- [ ] todo item").text(), "todo item");
        assert_eq!(render_test("- [x] done item").text(), "done item");
    }

    #[test]
    fn multiline_blockquote() {
        let result = render_test("> Line 1\n> Line 2\n> Line 3");
        let quote = result.span_iter().find(|s| s.typ == QUOTE).unwrap();
        assert_eq!(quote.start, 0);
        assert!(quote.end >= 20);
    }

    #[test]
    fn blockquote_then_text_separated() {
        let result = render_test("> Quote\n\nRegular text");
        assert!(!result.text().contains("QuoteRegular"));
    }

    #[test]
    fn blockquote_multiple_paragraphs_separated() {
        let result = render_test("> Para 1\n>\n> Para 2");
        assert!(!result.text().contains("Para 1Para 2"));
    }

    /// Nested blockquote depth values and uniform end positions (no staircase).
    #[test]
    fn nested_blockquotes() {
        let result = render_test(
            "> one\n\
             >> two\n\
             > > > three"
        );
        let quotes: Vec<_> = result.span_iter().filter(|s| s.typ == QUOTE).collect();
        assert_eq!(quotes.len(), 3);
        let (inner, middle, outer) = (&quotes[0], &quotes[1], &quotes[2]);
        assert_eq!(inner.data, 2);
        assert_eq!(middle.data, 1);
        assert_eq!(outer.data, 0);
        assert_eq!(outer.start, 0);
        assert!(inner.start > middle.start);
        assert!(middle.start > outer.start);
        // All end at the same position — no trailing staircase.
        assert_eq!(inner.end, middle.end);
        assert_eq!(middle.end, outer.end);
    }

    /// Blockquote with a list: QUOTE sorts before LIST_ITEM at same start.
    #[test]
    fn blockquote_containing_list() {
        let result = render_test("> - item 1\n> - item 2");
        assert!(result.span_iter().any(|s| s.typ == QUOTE));
        assert!(result.span_iter().any(|s| s.typ == LIST_ITEM));

        let blob = blob_bytes("> - item 1\n> - item 2");
        let tlen = i32::from_le_bytes(blob[0..4].try_into().unwrap()) as usize;
        let base = 8 + tlen + (4 - tlen % 4) % 4;
        let first_type = i32::from_le_bytes(blob[base+8..base+12].try_into().unwrap());
        assert_eq!(first_type, QUOTE);
    }

    /// Fenced code block strips fence and language tag.
    #[test]
    fn code_block() {
        let result = render_test("```rust\nfn main() {}\n```");
        assert_eq!(result.text(), "fn main() {}");
        assert!(result.span_iter().any(|s| s.typ == CODE_BLOCK));
    }

    /// Empty code block produces no span (zero-length skipped).
    #[test]
    fn empty_code_block() {
        let result = render_test("```\n```");
        assert_eq!(result.text(), "");
        assert_eq!(result.span_iter().count(), 0);
    }

    /// Tables render as "View Table" placeholder.
    #[test]
    fn table_placeholder() {
        let result = render_test("| A | B |\n|---|---|\n| 1 | 2 |");
        assert_eq!(result.text(), "View Table");
        assert!(result.span_iter().any(|s| s.typ == TABLE));
    }

    /// `---` produces HRULE span with object-replacement char.
    #[test]
    fn thematic_break() {
        let result = render_test("---");
        assert!(result.span_iter().any(|s| s.typ == HRULE));
        assert_eq!(result.text(), "\u{FFFC}");
    }

    /// Consecutive thematic breaks produce separate spans.
    #[test]
    fn consecutive_thematic_breaks() {
        let result = render_test("---\n\n---");
        let hr_count = result.span_iter().filter(|s| s.typ == HRULE).count();
        assert_eq!(hr_count, 2);
    }
}

// ── footnotes ────────────────────────────────────────────────────────────

mod footnotes {
    use super::*;
    use crate::blob::*;

    /// References render as superscript numbers; definitions appear after an
    /// HRULE separator with the same numbering.
    #[test]
    fn refs_and_definitions() {
        let out = render_test("Hello[^1] world[^2].\n\n[^1]: First note.\n[^2]: Second note.");
        assert_eq!(out.text(), "Hello1 world2.\n\n\u{FFFC}\n\n1 First note.\n2 Second note.");
        let spans: Vec<_> = out.span_iter().collect();
        assert_eq!(spans.iter().filter(|s| s.typ == HRULE).count(), 1);
        assert_eq!(spans.iter().filter(|s| s.typ == SUPERSCRIPT).count(), 4);
    }
}

// ── edge ─────────────────────────────────────────────────────────────────

mod edge {
    use super::*;
    use crate::blob::*;

    /// Empty and whitespace-only inputs produce empty output with no spans.
    #[test]
    fn empty_inputs() {
        for input in &["", "   \n\n   \n"] {
            let result = render_test(input);
            assert_eq!(result.text(), "", "expected empty for {input:?}");
            assert_eq!(result.span_iter().count(), 0);
        }
        let blob = blob_bytes("");
        assert_eq!(blob.len(), 8);
        assert_eq!(blob_text(&blob), "");
        assert_eq!(blob_span_count(&blob), 0);
    }

    /// 5-level nested blockquote staircase depth values.
    #[test]
    fn deeply_nested_blockquotes() {
        let result = render_test("> 1\n>> 2\n>>> 3\n>>>> 4\n>>>>> 5");
        let quotes: Vec<_> = result.span_iter().filter(|s| s.typ == QUOTE).collect();
        assert_eq!(quotes.len(), 5);
        let mut depths: Vec<i32> = quotes.iter().map(|q| q.data).collect();
        depths.sort();
        assert_eq!(depths, vec![0, 1, 2, 3, 4]);
    }

    /// 4-level nested list indent values.
    #[test]
    fn deeply_nested_lists() {
        let result = render_test("- L1\n  - L2\n    - L3\n      - L4");
        let items: Vec<_> = result.span_iter().filter(|s| s.typ == LIST_ITEM).collect();
        assert!(items.len() >= 4);
        assert_eq!(items[0].indent(), 0);
        assert_eq!(items[1].indent(), 1);
        assert_eq!(items[2].indent(), 2);
        assert_eq!(items[3].indent(), 3);
    }

    #[test]
    fn unicode_emoji() {
        let result = render_test("Hello 😀🎉 world");
        assert!(result.text().contains("😀"));
        assert!(result.text().contains("🎉"));
    }

    #[test]
    fn unicode_cjk() {
        let result = render_test("中文测试");
        assert_eq!(result.text(), "中文测试");
    }

    /// Long plain paragraph produces no spans (fast path signal).
    #[test]
    fn plain_ascii_text_unchanged() {
        let input = "just some plain text";
        let result = render_test(input);
        assert_eq!(result.span_iter().count(), 0);
        assert_eq!(result.text(), input);
    }

    /// Long plain text still produces no spans.
    #[test]
    fn very_long_paragraph_no_spans() {
        let long = "word ".repeat(500);
        let result = render_test(&long);
        assert!(!result.text().is_empty());
        assert_eq!(result.span_iter().count(), 0);
    }

    /// Smart quotes transform text but produce no spans.
    #[test]
    fn smart_quotes_change_text_no_spans() {
        let result = render_test("\"hello\"");
        assert_eq!(result.span_iter().count(), 0);
        assert!(result.text().contains('\u{201C}') || result.text().contains('\u{201D}'));
    }

    /// Empty link text: LINK span is skipped (zero length), but domain suffix still appends.
    #[test]
    fn empty_link_text() {
        let result = render_test("[](https://example.com)");
        assert_eq!(result.text(), " (example.com)");
        assert_eq!(result.span_iter().filter(|s| s.typ == LINK).count(), 0);
    }

    /// Inline formatting inside link text keeps both span types.
    #[test]
    fn link_text_with_inline_formatting() {
        let result = render_test("[**bold link**](https://example.com)");
        assert!(result.span_iter().any(|s| s.typ == LINK));
        assert!(result.span_iter().any(|s| s.typ == BOLD));
    }

    /// Bare URL with query/fragment parses as LINK.
    #[test]
    fn url_with_query_and_fragment() {
        let result = render_test("https://example.com/page?q=1&r=2#section");
        assert!(result.span_iter().any(|s| s.typ == LINK));
    }

    /// Document UTF-16 unit counts for reference.
    #[test]
    fn utf16_len_reference() {
        assert_eq!("hello".encode_utf16().count(), 5);
        assert_eq!("café".encode_utf16().count(), 4);
        assert_eq!("中文".encode_utf16().count(), 2);
        assert_eq!("😀".encode_utf16().count(), 2);
        assert_eq!("Hello 中文 café 😀".encode_utf16().count(), 16);
    }

    /// Unclosed formatting markers pass through as literal text.
    #[test]
    fn pathological_unclosed_formatting() {
        let result = render_test("**unclosed bold");
        assert_eq!(result.text(), "**unclosed bold");
        assert_eq!(result.span_iter().filter(|s| s.typ == BOLD).count(), 0);

        let result = render_test("*unclosed italic");
        assert_eq!(result.text(), "*unclosed italic");
        assert_eq!(result.span_iter().filter(|s| s.typ == ITALIC).count(), 0);
    }

    /// `****` parses as thematic break, not empty bold. ```` `` ```` passes through.
    #[test]
    fn ambiguous_empty_formatting() {
        let result = render_test("****");
        assert_eq!(result.text(), "\u{FFFC}");
        assert!(result.span_iter().any(|s| s.typ == HRULE));

        let result = render_test("``");
        assert_eq!(result.text(), "``");
        assert_eq!(result.span_iter().count(), 0);
    }
}

