//! A blob read back, for the tests: the header, the text, the spans past the padding with their
//! addresses, and a table's cells, so no test parses the bytes itself.

use crate::blob::{APP_LINK, IMAGE, LINK};

/// A span as the blob holds it. A link's or picture's address is read out as `url` and its packed
/// offset and length consumed, so its `data` is 0; any other span's data is as written.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Span {
    pub start: i32,
    pub end: i32,
    pub typ: i32,
    pub data: i32,
    pub url: Option<String>,
}

impl Span {
    /// A list item's indent, from its data.
    pub fn indent(&self) -> i32 {
        self.data >> 16
    }

    /// A list item's number, from its data.
    pub fn number(&self) -> i32 {
        self.data & 0xFFFF
    }
}

/// The blob read back.
pub(crate) struct Blob {
    pub text: String,
    /// The header's flag byte: bit 0 ASCII text, bit 1 a spoiler's body.
    pub flags: u8,
    /// In the blob's order: start ascending, end descending, type ascending.
    pub spans: Vec<Span>,
    url_data: Vec<u8>,
}

impl Blob {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn ascii(&self) -> bool {
        self.flags & 1 != 0
    }

    pub fn span_iter(&self) -> impl Iterator<Item = &Span> {
        self.spans.iter()
    }

    /// A table's rows, columns and cells, row-major, from its span's data: a `(rows<<16|cols)`
    /// header at `data - 1`, then each cell's length and its markdown.
    pub fn table(&self, data: i32) -> (usize, usize, Vec<String>) {
        let mut p = (data - 1) as usize;
        let packed = u32::from_le_bytes(self.url_data[p..p + 4].try_into().unwrap());
        p += 4;
        let (rows, cols) = ((packed >> 16) as usize, (packed & 0xFFFF) as usize);
        let cells = (0..rows * cols)
            .map(|_| {
                let len = u16::from_le_bytes([self.url_data[p], self.url_data[p + 1]]) as usize;
                p += 2;
                let cell = String::from_utf8(self.url_data[p..p + len].to_vec()).unwrap();
                p += len;
                cell
            })
            .collect();
        (rows, cols, cells)
    }
}

/// Reads a blob back: the header, the text, the spans past the padding and their addresses.
pub(crate) fn read(blob: &[u8]) -> Blob {
    let int = |at: usize| i32::from_le_bytes(blob[at..at + 4].try_into().unwrap());
    let text_len = int(0) as usize;
    let count = (int(4) & 0xFF_FFFF) as usize;
    let spans_at = 8 + text_len + (4 - text_len % 4) % 4;
    let url_data = blob[spans_at + count * 16..].to_vec();
    let spans = (0..count)
        .map(|i| {
            let at = spans_at + i * 16;
            let (start, end, typ, data) = (int(at), int(at + 4), int(at + 8), int(at + 12));
            let url = (typ == LINK || typ == APP_LINK || typ == IMAGE).then(|| {
                let (offset, len) = ((data >> 12) as usize, (data & 0xFFF) as usize);
                String::from_utf8(url_data[offset..offset + len].to_vec()).unwrap()
            });
            Span { start, end, typ, data: if url.is_some() { 0 } else { data }, url }
        })
        .collect();
    Blob {
        text: std::str::from_utf8(&blob[8..8 + text_len]).unwrap().to_string(),
        flags: blob[7],
        spans,
        url_data,
    }
}
