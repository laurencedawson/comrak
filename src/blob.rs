//! Binary blob renderer for Android's FastSpannable.
//!
//! Produces a compact blob: `[text_len:4][span_count:4][text][pad][spans][url_data]`
//! All positions are UTF-16 code units mapping directly to Java String indices.
//!
//! Unlike HTML rendering, this walks the AST once and writes text + span metadata
//! into a single contiguous buffer with zero intermediate allocations.

use crate::arena_tree::Node;
use crate::nodes::{Ast, NodeValue::*, ListType};
use std::cell::RefCell;

type AstNode<'a> = Node<'a, RefCell<Ast>>;

// Span type constants (must match SpanTypes.java)
const QUOTE: i32 = 0;
const LIST_ITEM: i32 = 1;
const HEADING_1: i32 = 2;
const CODE: i32 = 8;
const CODE_BLOCK: i32 = 9;
const IMAGE: i32 = 10;
const TABLE: i32 = 11;
const HRULE: i32 = 12;
const BOLD: i32 = 14;
const ITALIC: i32 = 15;
const STRIKETHROUGH: i32 = 16;
const SUPERSCRIPT: i32 = 17;
const SUBSCRIPT: i32 = 18;
const SUPERSCRIPT_SIZE: i32 = 19;
const SUBSCRIPT_SIZE: i32 = 20;
const SPOILER: i32 = 21;
const LINK: i32 = 22;
const LINK_SIZE: i32 = 23;

const HEADINGS: [i32; 6] = [HEADING_1, HEADING_1 + 1, HEADING_1 + 2, HEADING_1 + 3, HEADING_1 + 4, HEADING_1 + 5];
const HEADER_SIZE: usize = 8;
const MAX_URL_LEN: usize = 4095;

/// Count UTF-16 code units from a UTF-8 string.
#[inline]
fn utf16_len(s: &str) -> usize {
    let mut len = 0;
    for &b in s.as_bytes() {
        if (b & 0b1100_0000) != 0b1000_0000 { len += 1; }
        if (b & 0b1111_1000) == 0b1111_0000 { len += 1; }
    }
    len
}

/// Collapse runs of whitespace into a single space.
#[inline]
fn collapse_whitespace(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.as_bytes().windows(2).any(|w| w[0] == b' ' && w[1] == b' ') {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space { out.push(' '); }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    std::borrow::Cow::Owned(out)
}

/// Extract the domain from a URL for display suffix deduplication.
/// Returns a borrowed slice when the host is already lowercase ASCII — most
/// links in practice — avoiding an allocation per link render.
fn extract_domain(url: &str) -> Option<std::borrow::Cow<'_, str>> {
    let start = url.find("://")? + 3;
    let end = url[start..].find('/').or_else(|| url[start..].find('?')).map_or(url.len(), |i| start + i);
    let mut host = &url[start..end];
    if host.starts_with("www.") { host = &host[4..]; }
    if host.is_empty() { return None; }
    if host.bytes().all(|b| !b.is_ascii_uppercase()) {
        Some(std::borrow::Cow::Borrowed(host))
    } else {
        Some(std::borrow::Cow::Owned(host.to_ascii_lowercase()))
    }
}

/// Render a comrak AST into a binary blob for Android's FastSpannable.
///
/// Returns `None` if the document has no spans and text is unchanged from input
/// (caller should use the raw input string directly).
/// Render a comrak AST into a binary blob for Android's FastSpannable.
///
/// For best performance, use `parse_document_raw()` to skip text node postprocessing,
/// since the blob visitor handles adjacent Text nodes natively.
pub fn render_blob<'a>(root: &'a AstNode<'a>, input: &str, show_previews: bool) -> Option<Vec<u8>> {
    let mut b = BlobWriter::new(input.len());
    visit(root, &mut b, 0, 0, 0);
    b.append_footnotes();
    if show_previews { b.append_previews(); }

    if b.spans.is_empty() && b.text() == input {
        return None;
    }
    Some(b.into_blob())
}

struct BlobWriter {
    blob: Vec<u8>,
    spans: Vec<i32>,
    url_data: Vec<u8>,
    footnotes: Vec<String>,
    previews: Vec<String>,
    len: usize,
    p: usize,
}

impl BlobWriter {
    fn new(cap: usize) -> Self {
        let mut blob = Vec::with_capacity(HEADER_SIZE + cap);
        blob.extend_from_slice(&[0u8; HEADER_SIZE]);
        Self { blob, spans: vec![], url_data: vec![], footnotes: vec![], previews: vec![], len: 0, p: 0 }
    }

    #[inline]
    fn pos(&self) -> usize { self.len + self.p }

    #[inline]
    fn write_text(&mut self, s: &str) {
        if self.p > 0 {
            self.blob.extend(std::iter::repeat_n(b'\n', self.p));
            self.len += self.p;
            self.p = 0;
        }
        self.blob.extend_from_slice(s.as_bytes());
        self.len += if s.is_ascii() { s.len() } else { utf16_len(s) };
    }

    fn nl(&mut self, n: usize) { if n > self.p { self.p = n; } }

    fn span(&mut self, t: i32, start: usize) { self.span_data(t, start, 0); }

    fn span_data(&mut self, t: i32, start: usize, data: i32) {
        if start < self.len {
            self.spans.extend_from_slice(&[start as i32, self.len as i32, t, data]);
        }
    }

    fn span_url(&mut self, t: i32, start: usize, url: &str) {
        if start >= self.len { return; }
        let offset = self.url_data.len();
        let url_len = url.len().min(MAX_URL_LEN);
        self.url_data.extend_from_slice(&url.as_bytes()[..url_len]);
        let data = ((offset as i32) << 12) | (url_len as i32);
        self.spans.extend_from_slice(&[start as i32, self.len as i32, t, data]);
    }

    fn blob_len(&self) -> usize { self.blob.len() }

    fn emit_image(&mut self, url: &str) {
        if self.len > 0 {
            let mut i = self.blob.len();
            while i > HEADER_SIZE && matches!(self.blob[i - 1], b' ' | b'\t') { i -= 1; }
            let after_ws = i;
            while i > HEADER_SIZE && self.blob[i - 1] == b'\n' { i -= 1; }
            let nl = after_ws - i;
            self.p += 2usize.saturating_sub(nl + self.p);
        } else {
            self.write_text("\n");
        }
        let start = self.pos();
        self.write_text("\u{FFFC}");
        self.span_url(IMAGE, start, url);
        self.nl(2);
    }

    fn append_domain_suffix(&mut self, text_start: usize, url: &str) {
        if let Some(domain) = extract_domain(url) {
            let link_text = &self.blob[text_start..];
            let needle = domain.as_bytes();
            let has_domain = link_text.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle));
            if !has_domain {
                self.write_text(" (");
                self.write_text(&domain);
                self.write_text(")");
            }
        }
    }

    fn append_footnotes(&mut self) {
        let notes = std::mem::take(&mut self.footnotes);
        if notes.is_empty() { return; }
        self.nl(2);
        let start = self.pos();
        self.write_text("\u{FFFC}");
        self.span(HRULE, start);
        self.nl(2);
        for (i, note) in notes.iter().enumerate() {
            let start = self.pos();
            self.write_text(&(i + 1).to_string());
            self.span(SUPERSCRIPT, start);
            self.span(SUPERSCRIPT_SIZE, start);
            self.write_text(" ");
            self.write_text(note);
            self.nl(1);
        }
    }

    fn append_previews(&mut self) {
        let mut urls = std::mem::take(&mut self.previews);
        if urls.is_empty() { return; }
        let mut seen = Vec::new();
        urls.retain(|u| if seen.contains(u) { false } else { seen.push(u.clone()); true });
        if urls.len() > 3 { return; }
        for url in &urls {
            let start = self.pos();
            self.write_text("\u{FFFC}");
            self.span_url(LINK, start, url);
            self.nl(2);
        }
    }

    fn text(&self) -> &str {
        std::str::from_utf8(&self.blob[HEADER_SIZE..]).unwrap_or("")
    }

    fn into_blob(mut self) -> Vec<u8> {
        let txt_len = self.blob.len() - HEADER_SIZE;
        let span_count = self.spans.len() / 4;

        let spans_view = unsafe {
            let ptr = self.spans.as_mut_ptr() as *mut [i32; 4];
            std::slice::from_raw_parts_mut(ptr, span_count)
        };
        // Pack the sort key (start asc, end desc, type asc) into a single u64
        // so the comparator is a plain u64 compare instead of a 3-field tuple
        // walk. Layout: bits 63..32 = start, bits 31..8 = inverted 24-bit end
        // (for descending order), bits 7..0 = span type. Safe because start/end
        // are non-negative text offsets (well under 2^24 for all realistic
        // documents) and span types fit in u8.
        spans_view.sort_unstable_by_key(|s| {
            let start = s[0] as u32 as u64;
            let end_inv = (0xFF_FFFF - (s[1] as u32 & 0xFF_FFFF)) as u64;
            let ty = (s[2] as u32 & 0xFF) as u64;
            (start << 32) | (end_inv << 8) | ty
        });

        self.blob[0..4].copy_from_slice(&(txt_len as i32).to_le_bytes());
        self.blob[4..8].copy_from_slice(&(span_count as i32).to_le_bytes());

        let padding = (4 - (txt_len % 4)) % 4;
        self.blob.extend_from_slice(&[0u8; 3][..padding]);

        let span_bytes = unsafe {
            std::slice::from_raw_parts(self.spans.as_ptr() as *const u8, self.spans.len() * 4)
        };
        self.blob.extend_from_slice(span_bytes);
        self.blob.extend_from_slice(&self.url_data);
        self.blob
    }
}

fn visit<'a>(node: &'a AstNode<'a>, out: &mut BlobWriter, list_depth: usize, quote_depth: usize, ordinal: i32) {
    let val = &node.data.borrow().value;
    let start = out.pos();

    match val {
        List(l) => {
            let mut num = match l.list_type { ListType::Ordered => l.start as i32, ListType::Bullet => 0 };
            for c in node.children() {
                visit(c, out, list_depth + 1, quote_depth, num);
                if num > 0 { num += 1; }
            }
            if list_depth == 0 { out.nl(2); }
        }

        Item(_) | TaskItem(_) => {
            let indent = list_depth.saturating_sub(1);
            let number = match val {
                TaskItem(ti) => if ti.symbol.is_some() { 0xFFFF } else { 0xFFFE },
                _ => ordinal.max(0),
            };
            for c in node.children() {
                if !matches!(c.data.borrow().value, List(_)) {
                    visit(c, out, list_depth, quote_depth, 0);
                }
            }
            out.span_data(LIST_ITEM, start, ((indent as i32) << 16) | number);
            for c in node.children() {
                if matches!(c.data.borrow().value, List(_)) {
                    out.nl(1);
                    visit(c, out, list_depth, quote_depth, 0);
                }
            }
            out.nl(1);
        }

        BlockQuote => {
            for (i, c) in node.children().enumerate() {
                if i > 0 { out.nl(2); }
                visit(c, out, list_depth, quote_depth + 1, 0);
            }
            out.span_data(QUOTE, start, quote_depth as i32);
            if quote_depth == 0 { out.nl(2); }
        }

        Paragraph => {
            visit_children(node, out, list_depth, quote_depth);
            if !node.parent().is_some_and(|p|
                matches!(p.data.borrow().value, BlockQuote | Item(_) | TaskItem(_))
            ) {
                out.nl(2);
            }
        }

        Heading(h) => {
            visit_children(node, out, list_depth, quote_depth);
            out.span(HEADINGS[(h.level as usize).saturating_sub(1).min(5)], start);
            out.span(BOLD, start);
            out.nl(2);
        }

        CodeBlock(c) => {
            out.write_text(c.literal.trim_end());
            out.span(CODE_BLOCK, start);
            out.nl(2);
        }

        Table(_) => {
            out.write_text("View Table");
            out.span(TABLE, start);
            out.nl(2);
        }

        ThematicBreak => {
            out.write_text("\u{FFFC}");
            out.span(HRULE, start);
            out.nl(2);
        }

        FootnoteReference(nfr) => {
            out.write_text(&nfr.ix.to_string());
            out.span(SUPERSCRIPT, start);
            out.span(SUPERSCRIPT_SIZE, start);
        }

        FootnoteDefinition(_) => {
            let mut tmp = BlobWriter::new(64);
            for c in node.children() { visit(c, &mut tmp, 0, 0, 0); }
            out.footnotes.push(tmp.text().trim().to_string());
        }

        Text(t) => out.write_text(&collapse_whitespace(t)),
        ShortCode(sc) => out.write_text(&sc.emoji),
        Code(c) => {
            out.write_text(&c.literal);
            out.span(CODE, start);
        }

        Image(l) => out.emit_image(&l.url),

        LineBreak => out.nl(1),
        SoftBreak => if quote_depth > 0 { out.nl(1) } else { out.write_text(" ") },

        Strong | Emph | Strikethrough | Superscript | Subscript | SpoileredText => {
            visit_children(node, out, list_depth, quote_depth);
            let (t, size) = match val {
                Strong => (BOLD, 0),
                Emph => (ITALIC, 0),
                Strikethrough => (STRIKETHROUGH, 0),
                Superscript => (SUPERSCRIPT, SUPERSCRIPT_SIZE),
                Subscript => (SUBSCRIPT, SUBSCRIPT_SIZE),
                SpoileredText => (SPOILER, 0),
                _ => unreachable!(),
            };
            out.span(t, start);
            if size != 0 { out.span(size, start); }
        }

        Link(l) => {
            let url = &l.url;
            let only_child = node.first_child().filter(|c| c.next_sibling().is_none());
            if only_child.is_some_and(|c| matches!(&c.data.borrow().value, Image(_))) {
                visit_children(node, out, list_depth, quote_depth);
            } else {
                let text_start = out.blob_len();
                visit_children(node, out, list_depth, quote_depth);
                out.span_url(LINK, start, url);
                out.span(LINK_SIZE, start);
                out.append_domain_suffix(text_start, url);
            }
        }

        LemmySpoiler(ls) => {
            if !ls.title.is_empty() {
                out.write_text(&ls.title);
                out.span(BOLD, start);
                out.nl(1);
            }
            let content_start = out.pos();
            visit_children(node, out, list_depth, quote_depth);
            out.span(SPOILER, content_start);
            out.nl(2);
        }

        _ => visit_children(node, out, list_depth, quote_depth),
    }
}

fn visit_children<'a>(node: &'a AstNode<'a>, out: &mut BlobWriter, ld: usize, qd: usize) {
    for c in node.children() { visit(c, out, ld, qd, 0); }
}
