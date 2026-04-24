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
