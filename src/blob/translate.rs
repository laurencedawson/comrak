//! A blob translated: its text handed out as one string, a line for each of its lines, and the
//! translated string written back over the same blob.
//!
//! [`mark_for_translation`] marks what a translator must hand back as it was: a link's words between
//! `[n]` and `[/n]`, and as `{n}` each part of a line that is no words to translate, code, a
//! picture, a rule, a table, a mention, a link whose words are an address. The domain the writer
//! puts after a link's words, a reader's aid, is left out and not put back. [`build_translated_blob`] takes
//! the answer line for line: the text is written again, each record finding its place from where
//! its line, its kept part or its link's markers landed; a line that came back as it went is
//! written as it was, every record on it kept. A record whose ends fall on none of them,
//! emphasis inside a sentence, is dropped, as is a link whose markers came back broken; a kept part
//! the answer lost goes at the end of its line. The url data is copied as it was, so every picture,
//! link and table keeps its payload. Nothing maps one offset onto another.
//!
//! Experimental: shaped by one translator on one device, unmeasured and unrefactored.
//!
//! TODO: benchmark both directions on a long reader page and a comment thread, the line scan and
//! the position lookups being linear searches, then refactor with the numbers: the plan's piece
//! walk, the edge and kept-part lookups, the UTF-16 copies.

use super::format::{ASCII_BIT, HEADER_SIZE, RECORD};
use std::ops::Range;

use super::*;
use crate::parser::url::extract_domain;

/// The blob's `records` over its text's `bytes`, as the view shows a body or one of its sections,
/// as one string to translate, its lines kept one for one, its links and kept parts marked. The
/// caller says where both lie, as the view's own reads do; `None` for a part the blob does not hold.
pub fn mark_for_translation(
    blob: &[u8],
    records: Range<usize>,
    bytes: Range<usize>,
) -> Option<String> {
    let plan = Plan::of(&Blob::read(blob, records, bytes)?);
    Some(
        (0..plan.lines.len())
            .map(|i| plan.marked(i))
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

/// A blob of the blob's `records` over its text's `bytes`, that text replaced by `answer`, a
/// translation of its [`mark_for_translation`]; the whole blob where they are all of it. `None` where
/// the answer has not as many lines, which leaves nothing to place a block's records by, so the
/// caller sends the lines one at a time and joins them.
pub fn build_translated_blob(
    blob: &[u8],
    records: Range<usize>,
    bytes: Range<usize>,
    answer: &str,
) -> Option<Vec<u8>> {
    let read = Blob::read(blob, records, bytes)?;
    let plan = Plan::of(&read);
    let answers: Vec<&str> = answer.trim_end().split('\n').collect();
    if answers.len() != plan.lines.len() {
        return None;
    }

    let mut text: Vec<u16> = Vec::with_capacity(plan.text.len() + plan.text.len() / 4);
    // Where an old position lands: every line's two ends, each kept part's start, a line written
    // as it was its start; a link's ends by its number
    let mut edges = Vec::with_capacity(plan.lines.len() * 2);
    let mut moved: Vec<(usize, usize, usize)> = Vec::new(); // old from..to now at
    let mut links: Vec<Option<(usize, usize)>> = vec![None; plan.marks];
    for (i, line) in plan.lines.iter().enumerate() {
        if i > 0 {
            text.push(b'\n' as u16);
        }
        edges.push((line.start, text.len()));
        // A line of kept parts alone, or one that came back as it went, is written as it was, every
        // record on it kept, emphasis too
        if !line.translates || answers[i].trim() == plan.marked(i).trim() {
            moved.push((line.start, line.end, text.len()));
            text.extend_from_slice(&plan.text[line.start..line.end]);
        } else {
            write_answer(&plan, line, answers[i], &mut text, &mut moved, &mut links);
        }
        edges.push((line.end, text.len()));
    }
    moved.sort_unstable();

    let land = |p: usize, end: bool| -> Option<usize> {
        if let Some(&(_, now)) = edges.iter().find(|&&(old, _)| old == p) {
            return Some(now);
        }
        moved
            .iter()
            .find(|&&(from, to, _)| {
                if end {
                    from < p && p <= to
                } else {
                    from <= p && p < to
                }
            })
            .map(|&(from, _, at)| at + p - from)
    };
    let mut records = Vec::with_capacity(read.records.len());
    // A record a kept part or a line written as it was holds moves with it, wherever the answer put
    // it; a link's records take its markers; any other takes its ends' places, a line's ends first
    let held = |r: &Record| {
        moved
            .iter()
            .find(|&&(from, to, _)| from <= r.start && r.end <= to)
            .map(|&(from, _, at)| (at + r.start - from, at + r.end - from))
    };
    for r in &read.records {
        let placed = held(r).or_else(|| match plan.link_of(r) {
            Some(n) => links[n],
            None => land(r.start, false).zip(land(r.end, true)),
        });
        if let Some((s, e)) = placed.filter(|&(s, e)| s < e) {
            records.push([s as i32, e as i32, r.typ, r.data]);
        }
    }
    Some(read.write(&text, records))
}

/// One line of the text and what is sent of it, in the old text's UTF-16 units.
struct Line {
    start: usize,
    end: usize,
    pieces: Vec<Piece>,
    /// Whether it holds words to translate; a line of kept parts alone is written as it was,
    /// whatever came back for it.
    translates: bool,
}

#[derive(Clone, Copy)]
enum Piece {
    Words(usize, usize),
    Open(usize),
    Close(usize),
    /// A kept part, by its number.
    Kept(usize),
}

/// The blob cut into its lines, its links and kept parts numbered across the whole text.
struct Plan {
    text: Vec<u16>,
    lines: Vec<Line>,
    /// How many numbers were given: links and kept parts share them.
    marks: usize,
    /// Each sent link's old range and number, for the records over exactly its words.
    links: Vec<(usize, usize, usize)>,
    /// Each kept part's old range by number.
    kept: Vec<Option<(usize, usize)>>,
}

impl Plan {
    fn of(blob: &Blob) -> Plan {
        let text = &blob.text;
        let len = text.len();
        let words = |r: &Record| String::from_utf16_lossy(&text[r.start..r.end]);
        // Kept parts and dropped suffixes, outermost first; then the links whose words are sent
        let mut keep: Vec<(usize, usize, bool)> = Vec::new(); // from, to, dropped
        let mut sent: Vec<(usize, usize)> = Vec::new();
        for r in &blob.records {
            match r.typ {
                CODE | CODE_BLOCK | IMAGE | TABLE | HRULE | APP_LINK => {
                    keep.push((r.start, r.end, false))
                }
                LINK => {
                    let url = blob.url(r.data);
                    if is_address(&words(r), url) {
                        keep.push((r.start, r.end, false));
                    } else {
                        sent.push((r.start, r.end));
                        if let Some(end) =
                            suffix(text, r.end, url).filter(|_| site::link_domain(blob.site))
                        {
                            keep.push((r.end, end, true));
                        }
                    }
                }
                _ => {}
            }
        }
        keep.sort_by_key(|&(from, to, _)| (from, std::cmp::Reverse(to)));
        let mut outer: Vec<(usize, usize, bool)> = Vec::new();
        for k in keep {
            if outer.last().is_none_or(|&(_, to, _)| k.0 >= to) {
                outer.push(k);
            }
        }

        let mut lines = Vec::new();
        let mut links = Vec::new();
        let mut kept = vec![None];
        let mut marks = 1;
        let mut start = 0;
        let mut k = 0;
        for at in (0..=len).filter(|&i| i == len || text[i] == b'\n' as u16) {
            let mut pieces = Vec::new();
            let mut closes: Vec<(usize, usize)> = Vec::new(); // end, number
            let mut i = start;
            let mut from = start;
            let mut letters = false;
            let flush = |pieces: &mut Vec<Piece>, from: usize, to: usize, letters: &mut bool| {
                if from < to {
                    *letters |= String::from_utf16_lossy(&text[from..to])
                        .chars()
                        .any(char::is_alphabetic);
                    pieces.push(Piece::Words(from, to));
                }
            };
            while i < at {
                if let Some(c) = closes.iter().position(|&(end, _)| end == i) {
                    flush(&mut pieces, from, i, &mut letters);
                    pieces.push(Piece::Close(closes.remove(c).1));
                    from = i;
                    continue;
                }
                if let Some(&(s, e)) = sent.iter().find(|&&(s, e)| s == i && e <= at) {
                    flush(&mut pieces, from, i, &mut letters);
                    links.push((s, e, marks));
                    kept.push(None);
                    pieces.push(Piece::Open(marks));
                    closes.push((e, marks));
                    marks += 1;
                    from = i;
                }
                while k < outer.len() && outer[k].1 <= i && outer[k].0 < i {
                    k += 1;
                }
                if let Some(&(s, e, dropped)) = outer.get(k).filter(|&&(s, _, _)| s == i) {
                    // A kept part runs to its own end or its line's, a code block's lines each their own
                    let to = e.min(at);
                    flush(&mut pieces, from, i, &mut letters);
                    if !dropped {
                        kept.push(Some((s, to)));
                        pieces.push(Piece::Kept(marks));
                        marks += 1;
                    }
                    if to == e {
                        k += 1;
                    } else {
                        outer[k].0 = at + 1;
                    }
                    i = to;
                    from = to;
                    continue;
                }
                i += 1;
            }
            flush(&mut pieces, from, at, &mut letters);
            for (_, n) in closes.drain(..).rev() {
                pieces.push(Piece::Close(n));
            }
            let translates = letters || pieces.iter().any(|p| matches!(p, Piece::Open(_)));
            lines.push(Line {
                start,
                end: at,
                pieces,
                translates,
            });
            start = at + 1;
        }
        Plan {
            text: text.clone(),
            lines,
            marks,
            links,
            kept,
        }
    }

    /// Line `i` as it is sent: its words, its links between their markers, its kept parts numbered.
    fn marked(&self, i: usize) -> String {
        let mut out = String::new();
        for piece in &self.lines[i].pieces {
            match *piece {
                Piece::Words(from, to) => {
                    out.push_str(&String::from_utf16_lossy(&self.text[from..to]))
                }
                Piece::Open(n) => out.push_str(&format!("[{n}]")),
                Piece::Close(n) => out.push_str(&format!("[/{n}]")),
                Piece::Kept(n) => out.push_str(&format!("{{{n}}}")),
            }
        }
        out
    }

    /// The number of the sent link whose words `r` lies over exactly, a link's own record or a
    /// span over all its words and no more.
    fn link_of(&self, r: &Record) -> Option<usize> {
        self.links
            .iter()
            .find(|&&(s, e, _)| s == r.start && e == r.end)
            .map(|&(.., n)| n)
    }
}

/// Writes `answer`, the translation of `line`, noting where each kept part and link marker lands;
/// a kept part the answer lost goes at its end, a marker it never sent is left out.
fn write_answer(
    plan: &Plan,
    line: &Line,
    answer: &str,
    text: &mut Vec<u16>,
    moved: &mut Vec<(usize, usize, usize)>,
    links: &mut [Option<(usize, usize)>],
) {
    let mut opened = vec![None; plan.marks];
    let mut placed = vec![false; plan.marks];
    let mine: Vec<usize> = line
        .pieces
        .iter()
        .filter_map(|p| match *p {
            Piece::Open(n) | Piece::Kept(n) => Some(n),
            _ => None,
        })
        .collect();
    let mut rest = answer.trim();
    while !rest.is_empty() {
        let Some((at, len, marker)) = next_marker(rest) else {
            text.extend(rest.encode_utf16());
            break;
        };
        text.extend(rest[..at].encode_utf16());
        rest = &rest[at + len..];
        match marker {
            Marker::Open(n) if mine.contains(&n) && opened[n].is_none() => {
                opened[n] = Some(text.len())
            }
            Marker::Close(n) if mine.contains(&n) => {
                if let Some(s) = opened[n].filter(|_| links[n].is_none()) {
                    links[n] = Some((s, text.len()));
                }
            }
            Marker::Kept(n) if mine.contains(&n) && !placed[n] => {
                if let Some((from, to)) = plan.kept[n] {
                    placed[n] = true;
                    moved.push((from, to, text.len()));
                    text.extend_from_slice(&plan.text[from..to]);
                }
            }
            _ => {}
        }
    }
    for &n in &mine {
        if let (Some((from, to)), false) = (plan.kept[n], placed[n]) {
            moved.push((from, to, text.len()));
            text.extend_from_slice(&plan.text[from..to]);
        }
    }
}

enum Marker {
    Open(usize),
    Close(usize),
    Kept(usize),
}

/// The first marker in `s`: where it starts, how long it is, and what it is. A translator may put
/// spaces inside one, `[ 1 ]`, or a full-width bracket round it.
fn next_marker(s: &str) -> Option<(usize, usize, Marker)> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if let Some((len, marker)) = marker_at(&s[i..]) {
            return Some((i, len, marker));
        }
        i += s[i..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

fn marker_at(s: &str) -> Option<(usize, Marker)> {
    let mut chars = s.char_indices().peekable();
    let (_, open) = chars.next()?;
    let close = match open {
        '[' | '［' => [']', '］'],
        '{' | '｛' => ['}', '｝'],
        _ => return None,
    };
    let skip = |chars: &mut std::iter::Peekable<std::str::CharIndices>| {
        while chars.peek().is_some_and(|&(_, c)| c == ' ') {
            chars.next();
        }
    };
    skip(&mut chars);
    let slash = open != '{' && open != '｛' && chars.peek().is_some_and(|&(_, c)| c == '/');
    if slash {
        chars.next();
        skip(&mut chars);
    }
    let mut n = 0usize;
    let mut digits = 0;
    while let Some(&(_, c)) = chars.peek() {
        let Some(d) = c.to_digit(10) else { break };
        n = n * 10 + d as usize;
        digits += 1;
        chars.next();
    }
    skip(&mut chars);
    let (at, end) = chars.next()?;
    if digits == 0 || !close.contains(&end) {
        return None;
    }
    let len = at + end.len_utf8();
    Some(match (open, slash) {
        ('{' | '｛', _) => (len, Marker::Kept(n)),
        (_, true) => (len, Marker::Close(n)),
        _ => (len, Marker::Open(n)),
    })
}

/// Whether a link's `words` are an address, its own url or its host: kept as written, since a
/// translator moves words into it.
fn is_address(words: &str, url: &str) -> bool {
    !words.contains(char::is_whitespace)
        && (words.contains('.') || words.contains("://"))
        && url
            .to_ascii_lowercase()
            .contains(&words.to_ascii_lowercase())
}

/// The end of the writer's ` (domain)` after a link's words at `at`, where the text has it.
fn suffix(text: &[u16], at: usize, url: &str) -> Option<usize> {
    let domain = extract_domain(url)?;
    let want: Vec<u16> = format!(" ({domain})").encode_utf16().collect();
    let end = at + want.len();
    (end <= text.len() && text[at..end] == want[..]).then_some(end)
}

struct Record {
    start: usize,
    end: usize,
    typ: i32,
    data: i32,
}

/// A blob read for translating: its text in UTF-16 units, as its records count, its records, and
/// the rest kept to write again.
struct Blob<'a> {
    flags: u32,
    site: u8,
    text: Vec<u16>,
    records: Vec<Record>,
    urls: &'a [u8],
}

impl<'a> Blob<'a> {
    /// The blob's `records` over its text's `bytes`, moved to start at 0: the first record opens the
    /// part, a section on its heading, so its start is where the text starts.
    fn read(blob: &'a [u8], records: Range<usize>, bytes: Range<usize>) -> Option<Blob<'a>> {
        let int = |at: usize| -> Option<u32> {
            Some(u32::from_le_bytes(blob.get(at..at + 4)?.try_into().ok()?))
        };
        let (text_len, count, flags) = read_header(blob)?;
        let records_at = records_at(text_len);
        if records.start > records.end || records.end > count {
            return None;
        }
        let mut read = Vec::with_capacity(records.len());
        for i in records.clone() {
            let at = records_at + i * RECORD;
            read.push(Record {
                start: int(at)? as usize,
                end: int(at + 4)? as usize,
                typ: int(at + 8)? as i32,
                data: int(at + 12)? as i32,
            });
        }
        if bytes.start > bytes.end || bytes.end > text_len {
            return None;
        }
        let text: Vec<u16> =
            std::str::from_utf8(blob.get(HEADER_SIZE + bytes.start..HEADER_SIZE + bytes.end)?)
                .ok()?
                .encode_utf16()
                .collect();
        let from = if bytes.start == 0 {
            0
        } else {
            read.first()?.start
        };
        for r in &mut read {
            r.start = r.start.checked_sub(from)?;
            r.end = r.end.checked_sub(from)?;
        }
        Some(Blob {
            flags,
            site: ((flags >> format::SITE_SHIFT) & ((1 << format::SITE_BITS) - 1)) as u8,
            text,
            records: read,
            urls: blob.get(records_at + count * RECORD..)?,
        })
    }

    /// The url of the link whose record holds `data`, its offset over its length.
    fn url(&self, data: i32) -> &str {
        self.urls
            .get(link_range(data))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }

    /// A blob of `text` and `records`, sorted as the writer sorts them, the ASCII bit read off the
    /// new text, a free heading's section byte its new byte, the site and url data as they were.
    fn write(&self, text: &[u16], mut records: Vec<[i32; 4]>) -> Vec<u8> {
        let text = String::from_utf16_lossy(text);
        let mut byte_at = Vec::with_capacity(text.len() + 1); // byte offset by UTF-16 unit
        let mut bytes = 0;
        for c in text.chars() {
            for _ in 0..c.len_utf16() {
                byte_at.push(bytes);
            }
            bytes += c.len_utf8();
        }
        byte_at.push(bytes);
        for r in &mut records {
            if (HEADINGS[0]..=HEADINGS[5]).contains(&r[2]) && r[3] != 0 {
                r[3] = byte_at[r[0] as usize] as i32;
            }
        }
        records.sort_by_key(sort_key);

        let flags = (self.flags & !(1 << ASCII_BIT)) | ((text.is_ascii() as u32) << ASCII_BIT);
        let mut out = Vec::with_capacity(
            HEADER_SIZE + text.len() + 3 + records.len() * RECORD + self.urls.len(),
        );
        out.extend_from_slice(&header(text.len(), records.len(), flags));
        out.extend_from_slice(text.as_bytes());
        out.resize(records_at(text.len()), 0);
        for r in &records {
            for v in r {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(self.urls);
        out
    }
}
