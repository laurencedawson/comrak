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
//! Experimental: shaped by one translator on one device.
//!
//! The text is read and written as the UTF-8 it is: only the records count in UTF-16 units, so
//! their ends are moved to bytes once as the blob is read and back once as it is written, neither
//! for ASCII text.

use super::format::{
    ASCII_BIT, HEADER_SIZE, LINK_LENGTH_BITS, PICTURE_ALT_LENGTH, PICTURE_URL, PICTURE_URL_LENGTH,
    RECORD, TABLE_CELLS, TABLE_DATA_BIAS, TABLE_ROWS_SHIFT,
};
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
    let blob = Blob::read(blob, records, bytes)?;
    let plan = Plan::of(&blob);
    let mut out = String::with_capacity(plan.text.len() + plan.text.len() / 8);
    for i in 0..plan.lines.len() {
        if i > 0 {
            out.push('\n');
        }
        plan.mark_into(i, &mut out);
    }
    Some(out)
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
    // An empty last line is a line; only blank lines past the text's, a trailing newline, are not
    let mut answers: Vec<&str> = answer.split('\n').collect();
    while answers.len() > plan.lines.len() && answers.last().is_some_and(|l| l.trim().is_empty()) {
        answers.pop();
    }
    if answers.len() != plan.lines.len() {
        return None;
    }

    let mut text = String::with_capacity(answer.len() + plan.text.len() / 4);
    // Where an old position lands: every line's two ends, each kept part's start, a line written
    // as it was its start; a link's ends by its number
    let mut edges = Vec::with_capacity(plan.lines.len() * 2);
    let mut moved: Vec<(usize, usize, usize)> = Vec::new(); // old from..to now at
    let mut links: Vec<Option<(usize, usize)>> = vec![None; plan.marks];
    let mut sent = String::new();
    for (i, line) in plan.lines.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        edges.push((line.start, text.len()));
        // A line of kept parts alone, or one that came back as it went, is written as it was, every
        // record on it kept, emphasis too
        sent.clear();
        plan.mark_into(i, &mut sent);
        if !line.translates || answers[i].trim() == sent.trim() {
            moved.push((line.start, line.end, text.len()));
            text.push_str(&plan.text[line.start..line.end]);
        } else {
            write_answer(&plan, line, answers[i], &mut text, &mut moved, &mut links);
        }
        edges.push((line.end, text.len()));
    }
    moved.sort_unstable();

    // Edges are in the text's order, and the moved ranges, sorted, never overlap: each lookup is a
    // binary search for the one that can hold the position
    let land = |p: usize, end: bool| -> Option<usize> {
        let e = edges.partition_point(|&(old, _)| old < p);
        if let Some(&(_, now)) = edges.get(e).filter(|&&(old, _)| old == p) {
            return Some(now);
        }
        let m = moved.partition_point(|&(from, _, _)| if end { from < p } else { from <= p });
        let &(from, to, at) = moved.get(m.checked_sub(1)?)?;
        (if end { p <= to } else { p < to }).then_some(at + p - from)
    };
    let mut records = Vec::with_capacity(read.records.len());
    // A record a kept part or a line written as it was holds moves with it, wherever the answer put
    // it; a link's records take its markers; any other takes its ends' places, a line's ends first
    let held = |r: &Record| {
        let m = moved.partition_point(|&(from, _, _)| from <= r.start);
        let &(from, to, at) = moved.get(m.checked_sub(1)?)?;
        (r.end <= to).then_some((at + r.start - from, at + r.end - from))
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

/// One line of the text and what is sent of it, in the old text's bytes.
struct Line {
    start: usize,
    end: usize,
    /// Its pieces, a range of the plan's.
    pieces: Range<usize>,
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
struct Plan<'t> {
    text: &'t str,
    lines: Vec<Line>,
    /// Every line's pieces, one after another.
    pieces: Vec<Piece>,
    /// How many numbers were given: links and kept parts share them.
    marks: usize,
    /// Each sent link's old range and number, for the records over exactly its words.
    links: Vec<(usize, usize, usize)>,
    /// Each kept part's old range by number.
    kept: Vec<Option<(usize, usize)>>,
}

impl<'t> Plan<'t> {
    fn of(blob: &'t Blob) -> Plan<'t> {
        let text = blob.text;
        let len = text.len();
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
                    if is_address(&text[r.start..r.end], url) {
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
        let mut pieces = Vec::new();
        let mut links = Vec::new();
        let mut kept = vec![None];
        let mut marks = 1;
        let mut start = 0;
        let mut k = 0;
        // The links to send are in the text's order, so the next is found by a cursor, not a search
        let mut next_sent = 0;
        let mut closes: Vec<(usize, usize)> = Vec::new(); // end, number
        let ends = memchr::memchr_iter(b'\n', text.as_bytes()).chain(std::iter::once(len));
        for at in ends {
            let first = pieces.len();
            let mut i = start;
            let mut from = start;
            let mut letters = false;
            // Words, but for any of them a translator would read as a marker, a citation's [1], which
            // goes as a kept part of its own and comes back as the page wrote it
            let flush = |pieces: &mut Vec<Piece>,
                         kept: &mut Vec<Option<(usize, usize)>>,
                         marks: &mut usize,
                         from: usize,
                         to: usize,
                         letters: &mut bool| {
                if from < to {
                    *letters |= !*letters && text[from..to].chars().any(char::is_alphabetic);
                    let mut at = from;
                    while let Some((m, len, _)) = next_marker(&text[at..to]) {
                        if m > 0 {
                            pieces.push(Piece::Words(at, at + m));
                        }
                        kept.push(Some((at + m, at + m + len)));
                        pieces.push(Piece::Kept(*marks));
                        *marks += 1;
                        at += m + len;
                    }
                    if at < to {
                        pieces.push(Piece::Words(at, to));
                    }
                }
            };
            while i < at {
                if let Some(c) = closes.iter().position(|&(end, _)| end == i) {
                    flush(&mut pieces, &mut kept, &mut marks, from, i, &mut letters);
                    pieces.push(Piece::Close(closes.remove(c).1));
                    from = i;
                    continue;
                }
                while sent.get(next_sent).is_some_and(|&(s, _)| s < i) {
                    next_sent += 1;
                }
                if let Some(&(s, e)) = sent.get(next_sent).filter(|&&(s, e)| s == i && e <= at) {
                    next_sent += 1;
                    flush(&mut pieces, &mut kept, &mut marks, from, i, &mut letters);
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
                    flush(&mut pieces, &mut kept, &mut marks, from, i, &mut letters);
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
                // Nothing opens or closes here: on to the next place something does, or the line's end
                i = closes
                    .iter()
                    .map(|&(end, _)| end)
                    .chain(sent.get(next_sent).map(|&(s, _)| s))
                    .chain(outer.get(k).map(|&(s, _, _)| s))
                    .filter(|&p| p > i)
                    .fold(at, usize::min);
            }
            flush(&mut pieces, &mut kept, &mut marks, from, at, &mut letters);
            for (_, n) in closes.drain(..).rev() {
                pieces.push(Piece::Close(n));
            }
            let translates = letters || pieces[first..].iter().any(|p| matches!(p, Piece::Open(_)));
            lines.push(Line {
                start,
                end: at,
                pieces: first..pieces.len(),
                translates,
            });
            start = at + 1;
        }
        Plan {
            text,
            lines,
            pieces,
            marks,
            links,
            kept,
        }
    }

    /// Line `i` as it is sent, written onto `out`: its words, its links between their markers, its
    /// kept parts numbered.
    fn mark_into(&self, i: usize, out: &mut String) {
        use std::fmt::Write;
        for piece in &self.pieces[self.lines[i].pieces.clone()] {
            let _ = match *piece {
                Piece::Words(from, to) => {
                    out.push_str(&self.text[from..to]);
                    Ok(())
                }
                Piece::Open(n) => write!(out, "[{n}]"),
                Piece::Close(n) => write!(out, "[/{n}]"),
                Piece::Kept(n) => write!(out, "{{{n}}}"),
            };
        }
    }

    /// The number of the sent link whose words `r` lies over exactly, a link's own record or a
    /// span over all its words and no more.
    fn link_of(&self, r: &Record) -> Option<usize> {
        let i = self.links.partition_point(|&(s, _, _)| s < r.start);
        self.links
            .get(i)
            .filter(|&&(s, e, _)| s == r.start && e == r.end)
            .map(|&(.., n)| n)
    }
}

/// Writes `answer`, the translation of `line`, noting where each kept part and link marker lands;
/// a kept part the answer lost goes at its end, a marker it never sent is left out.
fn write_answer(
    plan: &Plan,
    line: &Line,
    answer: &str,
    text: &mut String,
    moved: &mut Vec<(usize, usize, usize)>,
    links: &mut [Option<(usize, usize)>],
) {
    // The line's own links and kept parts, by number: where each link opened, whether each kept part
    // was placed. A marker of any other line is left out
    let mut mine: Vec<(usize, Option<usize>, bool)> = plan.pieces[line.pieces.clone()]
        .iter()
        .filter_map(|p| match *p {
            Piece::Open(n) | Piece::Kept(n) => Some((n, None, false)),
            _ => None,
        })
        .collect();
    let mut rest = answer.trim();
    while !rest.is_empty() {
        let Some((at, len, marker)) = next_marker(rest) else {
            text.push_str(rest);
            break;
        };
        text.push_str(&rest[..at]);
        rest = &rest[at + len..];
        let (Marker::Open(n) | Marker::Close(n) | Marker::Kept(n)) = marker;
        let Some(m) = mine.iter_mut().find(|m| m.0 == n) else {
            continue;
        };
        match marker {
            Marker::Open(_) if m.1.is_none() => m.1 = Some(text.len()),
            Marker::Close(_) => {
                if let Some(s) = m.1.filter(|_| links[n].is_none()) {
                    links[n] = Some((s, text.len()));
                }
            }
            Marker::Kept(_) if !m.2 => {
                if let Some((from, to)) = plan.kept[n] {
                    m.2 = true;
                    moved.push((from, to, text.len()));
                    text.push_str(&plan.text[from..to]);
                }
            }
            _ => {}
        }
    }
    for &(n, _, placed) in &mine {
        if let (Some((from, to)), false) = (plan.kept[n], placed) {
            moved.push((from, to, text.len()));
            text.push_str(&plan.text[from..to]);
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
    // A marker opens on '[' or '{', or a full-width one, whose UTF-8 starts 0xEF
    memchr::memchr3_iter(b'[', b'{', 0xEF, s.as_bytes())
        .find_map(|i| marker_at(&s[i..]).map(|(len, marker)| (i, len, marker)))
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
        let Some(d) = digit(c) else { break };
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

/// A decimal digit's value, in any script a translator writes a number in: each script's digits
/// run on from its zero.
fn digit(c: char) -> Option<u32> {
    const ZEROS: [char; 19] = [
        '0', '\u{660}', '\u{6F0}', '\u{7C0}', '\u{966}', '\u{9E6}', '\u{A66}', '\u{AE6}',
        '\u{B66}', '\u{BE6}', '\u{C66}', '\u{CE6}', '\u{D66}', '\u{E50}', '\u{ED0}', '\u{F20}',
        '\u{1040}', '\u{17E0}', '\u{FF10}',
    ];
    ZEROS
        .iter()
        .find_map(|&z| (c as u32).checked_sub(z as u32).filter(|&d| d < 10))
}

/// Whether a link's `words` are an address, its own url or its host: kept as written, since a
/// translator moves words into it.
fn is_address(words: &str, url: &str) -> bool {
    !words.is_empty()
        && !words.contains(char::is_whitespace)
        && (words.contains('.') || words.contains("://"))
        && url
            .as_bytes()
            .windows(words.len())
            .any(|w| w.eq_ignore_ascii_case(words.as_bytes()))
}

/// The end of the writer's ` (domain)` after a link's words at `at`, where the text has it.
fn suffix(text: &str, at: usize, url: &str) -> Option<usize> {
    let domain = extract_domain(url)?;
    let rest = text[at..].strip_prefix(" (")?.strip_prefix(&*domain)?;
    rest.starts_with(')').then(|| text.len() - rest.len() + 1)
}

struct Record {
    start: usize,
    end: usize,
    typ: i32,
    data: i32,
}

/// A blob read for translating: its text as it lies in the blob, its records over its bytes, and
/// the rest kept to write again.
struct Blob<'a> {
    flags: u32,
    site: u8,
    text: &'a str,
    records: Vec<Record>,
    urls: &'a [u8],
    /// Whether every record over all the text was read, so the url data is all its records'.
    whole: bool,
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
        let text =
            std::str::from_utf8(blob.get(HEADER_SIZE + bytes.start..HEADER_SIZE + bytes.end)?)
                .ok()?;
        let from = if bytes.start == 0 {
            0
        } else {
            read.first()?.start
        };
        for r in &mut read {
            r.start = r.start.checked_sub(from)?;
            r.end = r.end.checked_sub(from)?;
        }
        if !text.is_ascii() {
            remap(
                text,
                read.iter_mut().flat_map(|r| [&mut r.start, &mut r.end]),
                false,
            )?;
        }
        Some(Blob {
            flags,
            site: ((flags >> format::SITE_SHIFT) & ((1 << format::SITE_BITS) - 1)) as u8,
            text,
            records: read,
            urls: blob.get(records_at + count * RECORD..)?,
            whole: records == (0..count) && bytes == (0..text_len),
        })
    }

    /// The url of the link whose record holds `data`, its offset over its length.
    fn url(&self, data: i32) -> &str {
        self.urls
            .get(link_range(data))
            .and_then(|b| std::str::from_utf8(b).ok())
            .unwrap_or("")
    }

    /// A blob of `text` and `records` over its bytes, sorted as the writer sorts them, their ends
    /// in UTF-16 units, the ASCII bit read off the new text, a free heading's section byte its new
    /// byte, the site as it was; the url data as it was for the whole blob, and for a part its own
    /// records' alone, each payload once and each record pointing at where it now lies, so a page
    /// built a section at a time holds its url data once rather than once a section.
    fn write(&self, text: &str, mut records: Vec<[i32; 4]>) -> Vec<u8> {
        let urls = if self.whole {
            None
        } else {
            Some(own_urls(self.urls, &mut records))
        };
        let urls = urls.as_deref().unwrap_or(self.urls);
        for r in &mut records {
            if (HEADINGS[0]..=HEADINGS[5]).contains(&r[2]) && r[3] != 0 {
                r[3] = r[0];
            }
        }
        let ascii = text.is_ascii();
        if !ascii {
            let mut ends: Vec<usize> = records
                .iter()
                .flat_map(|r| [r[0] as usize, r[1] as usize])
                .collect();
            remap(text, ends.iter_mut(), true);
            for (r, e) in records.iter_mut().zip(ends.chunks_exact(2)) {
                (r[0], r[1]) = (e[0] as i32, e[1] as i32);
            }
        }
        records.sort_by_key(sort_key);

        let flags = (self.flags & !(1 << ASCII_BIT)) | ((ascii as u32) << ASCII_BIT);
        let mut out =
            Vec::with_capacity(HEADER_SIZE + text.len() + 3 + records.len() * RECORD + urls.len());
        out.extend_from_slice(&header(text.len(), records.len(), flags));
        out.extend_from_slice(text.as_bytes());
        out.resize(records_at(text.len()), 0);
        for r in &records {
            for v in r {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(urls);
        out
    }
}

/// The url data `records` point into, out of `urls`, each payload once in the order first pointed
/// at: a link's url, a picture's payload, a table's cells. Each record's data is rewritten to where
/// its payload now lies; a payload the data does not reach whole is left out, its record pointing
/// at nothing.
fn own_urls(urls: &[u8], records: &mut [[i32; 4]]) -> Vec<u8> {
    let mut own = Vec::new();
    let mut moved: Vec<(usize, usize)> = Vec::new(); // a payload's old offset, its new one
    for r in records.iter_mut() {
        let Some(range) = payload(urls, r[2], r[3]) else {
            continue;
        };
        let at = match moved.iter().find(|&&(from, _)| from == range.start) {
            Some(&(_, to)) => to,
            None => {
                let to = own.len();
                own.extend_from_slice(&urls[range.clone()]);
                moved.push((range.start, to));
                to
            }
        };
        r[3] = match r[2] {
            LINK | APP_LINK => ((at as i32) << LINK_LENGTH_BITS) | range.len() as i32,
            TABLE => (at + TABLE_DATA_BIAS) as i32,
            _ => at as i32,
        };
    }
    own
}

/// Where the payload of a record of `typ` holding `data` lies in `urls`: a link's url, a picture's
/// head and its url and alt, a table's rows and cols and its cells; none for a record with no
/// payload, or one past the url data's end.
fn payload(urls: &[u8], typ: i32, data: i32) -> Option<Range<usize>> {
    let u16_at = |at: usize| -> Option<usize> {
        Some(u16::from_le_bytes(urls.get(at..at + 2)?.try_into().ok()?) as usize)
    };
    let range = match typ {
        LINK | APP_LINK => link_range(data),
        IMAGE => {
            let at = data as usize;
            at..at
                + PICTURE_URL
                + u16_at(at + PICTURE_URL_LENGTH)?
                + u16_at(at + PICTURE_ALT_LENGTH)?
        }
        TABLE if data as usize >= TABLE_DATA_BIAS => {
            let at = data as usize - TABLE_DATA_BIAS;
            let packed = u32::from_le_bytes(urls.get(at..at + 4)?.try_into().ok()?);
            let cells = (packed >> TABLE_ROWS_SHIFT) as usize
                * (packed & ((1 << TABLE_ROWS_SHIFT) - 1)) as usize;
            let mut end = at + TABLE_CELLS;
            for _ in 0..cells {
                end += 2 + u16_at(end)?;
            }
            at..end
        }
        _ => return None,
    };
    (range.end <= urls.len()).then_some(range)
}

/// Moves each of `ends`, offsets into `text`, from UTF-16 units to bytes, or back where `to_units`:
/// one walk of the text's characters for all of them, sorted first. `None` for an end past the
/// text's.
fn remap<'e>(text: &str, ends: impl Iterator<Item = &'e mut usize>, to_units: bool) -> Option<()> {
    let mut ends: Vec<&mut usize> = ends.collect();
    ends.sort_unstable_by_key(|e| **e);
    let (mut unit, mut byte) = (0, 0);
    let mut chars = text.chars();
    for e in ends {
        let want = *e;
        while if to_units { byte } else { unit } < want {
            let c = chars.next()?;
            unit += c.len_utf16();
            byte += c.len_utf8();
        }
        *e = if to_units { unit } else { byte };
    }
    Some(())
}
