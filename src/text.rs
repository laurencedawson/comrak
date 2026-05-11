//! Small text-normalization helpers used by the blob renderer.

use std::borrow::Cow;

/// Collapse runs of two or more spaces into a single space. Zero-copy fast
/// path when the input has no double-space (the common case).
#[inline]
pub fn collapse_whitespace(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().windows(2).any(|w| w[0] == b' ' && w[1] == b' ') {
        return Cow::Borrowed(s);
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
    Cow::Owned(out)
}

/// Substitute common typographic chars with ASCII equivalents:
/// - U+2018/U+2019 (curly single quotes) → `'`
/// - U+201C/U+201D (curly double quotes) → `"`
/// - U+2013 (en dash) / U+2014 (em dash) → `-`
/// - U+2026 (horizontal ellipsis) → `...`
///
/// All six share the `E2 80 X` UTF-8 prefix; one window scan covers them.
/// Zero-copy fast path when none are present (the common case).
#[inline]
pub fn prefer_ascii(s: &str) -> Cow<'_, str> {
    if !s.as_bytes().windows(3).any(|w|
        w[0] == 0xE2 && w[1] == 0x80 && matches!(w[2], 0x93 | 0x94 | 0x98 | 0x99 | 0x9C | 0x9D | 0xA6))
    {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201C}' | '\u{201D}' => out.push('"'),
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            _ => out.push(ch),
        }
    }
    Cow::Owned(out)
}
