//! From dom_smoothie 0.19.0's `helpers.rs`, `matching.rs` and `score.rs`, on [`Tree`] in place of
//! dom_query: a node's score is a field, not an attribute.

use crate::blob::article::tree::{Id, Tree};

use super::glob::*;

pub(crate) fn text_similarity(text_a: &str, text_b: &str) -> f64 {
    if text_a.is_empty() || text_b.is_empty() {
        return 0.0;
    }
    let a = text_a.to_lowercase();
    let b = text_b.to_lowercase();
    if a.contains(&b) {
        return 1.0;
    }
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(String::from)
            .collect()
    };
    let tokens_a = words(&a);
    let tokens_b = words(&b);
    let b_total: usize = tokens_b.iter().map(|s| s.chars().count()).sum();
    if b_total == 0 {
        return 0.0;
    }
    let b_unique: usize = tokens_b
        .iter()
        .filter(|s| !tokens_a.contains(s))
        .map(|s| s.chars().count())
        .sum();
    1.0 - b_unique as f64 / b_total as f64
}

pub(crate) fn is_phrasing_content(t: &Tree, n: Id) -> bool {
    if t.text_of(n).is_some() {
        return true;
    }
    let Some(name) = t.name(n) else { return false };
    if PHRASING_ELEMS.contains(&name) {
        return true;
    }
    if matches!(name, "a" | "del" | "ins") {
        let mut children = t.children(n).peekable();
        return children.peek().is_some() && children.all(|c| is_phrasing_content(t, c));
    }
    false
}

pub(crate) fn is_whitespace(t: &Tree, n: Id) -> bool {
    t.text_of(n).is_some_and(|s| s.trim().is_empty()) || t.is(n, "br")
}

/// `max_depth` of `Some(0)` looks three up, as dom_smoothie's does.
pub(crate) fn has_ancestor(
    t: &Tree,
    n: Id,
    max_depth: Option<usize>,
    f: impl Fn(Id) -> bool,
) -> bool {
    let depth = match max_depth {
        Some(0) => 3,
        Some(d) => d,
        None => usize::MAX,
    };
    t.ancestors(n).take(depth).any(f)
}

pub(crate) fn normalize_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn link_density(t: &Tree, n: Id, char_count: Option<usize>) -> f32 {
    let mut link_length = 0f32;
    for a in t.select(n, &["a"]) {
        let href = t.attr(a, "href").unwrap_or("");
        let coeff = if href.len() > 1 && href.starts_with('#') {
            0.3
        } else {
            1.0
        };
        link_length += t.char_count(a) as f32 * coeff;
    }
    if link_length == 0.0 {
        return 0.0;
    }
    let text_length = char_count.unwrap_or_else(|| t.char_count(n)) as f32;
    if text_length == 0.0 {
        return 0.0;
    }
    link_length / text_length
}

/// The one element child of `n`, if it is a `tag` and `n` has no words of its own.
pub(crate) fn single_child_element(t: &Tree, n: Id, tag: &str) -> Option<Id> {
    let children = t.element_children(n);
    if children.len() != 1 || !t.is(children[0], tag) {
        return None;
    }
    if t.children(n).any(|c| t.is_nonempty_text(c)) {
        return None;
    }
    Some(children[0])
}

pub(crate) fn is_element_without_content(t: &Tree, n: Id) -> bool {
    if t.descendants(n).any(|d| t.is_nonempty_text(d)) {
        return false;
    }
    let elements = t.children(n).filter(|&c| t.is_element(c)).count();
    if elements == 0 {
        return true;
    }
    elements == t.select(n, &["br", "hr"]).len()
}

pub(crate) fn is_probably_visible(t: &Tree, n: Id) -> bool {
    if t.has_attr(n, "hidden") || is_invisible_style(t, n) {
        return false;
    }
    t.attr(n, "aria-hidden") != Some("true")
        || t.attr(n, "class")
            .is_some_and(|c| c.contains("fallback-image"))
}

fn is_invisible_style(t: &Tree, n: Id) -> bool {
    let Some(style) = t.attr(n, "style") else {
        return false;
    };
    let style = style.to_ascii_lowercase();
    style_has_kv(&style, "display", "none") || style_has_kv(&style, "visibility", "hidden")
}

fn style_has_kv(style: &str, key: &str, val: &str) -> bool {
    let Some(pos) = style.find(key) else {
        return false;
    };
    let mut rest = &style[pos..];
    let Some(pos) = rest.find(':') else {
        return false;
    };
    rest = &rest[pos + 1..];
    if let Some(pos) = rest.find(';') {
        rest = &rest[..pos];
    }
    rest = rest.trim_start();
    if let Some(pos) = rest.find(char::is_whitespace) {
        rest = &rest[..pos];
    }
    rest.trim_end() == val
}

/// A node's class and id, lowercased, for the word lists to search.
pub(crate) fn matching_string(t: &Tree, n: Id) -> String {
    let mut buf = String::new();
    for (k, v) in t.attrs(n) {
        if k == "class" || k == "id" {
            buf.push_str(v);
            buf.push(' ');
        }
    }
    buf.make_ascii_lowercase();
    buf
}

/// The next element in document order, skipping `n`'s children when `ignore_child` is set.
pub(crate) fn next_child_or_sibling(t: &Tree, n: Id, ignore_child: bool) -> Option<Id> {
    if !ignore_child && let Some(c) = t.first_element_child(n) {
        return Some(c);
    }
    if let Some(s) = t.next_element_sibling(n) {
        return Some(s);
    }
    t.ancestors(n).find_map(|p| t.next_element_sibling(p))
}

/// dom_smoothie's `BytePatternCheck`: a pattern with a byte the haystack lacks is passed over
/// before any search for it.
pub(crate) fn contains_any(haystack: &str, pats: &[&str]) -> bool {
    let mut seen = [false; 256];
    for &b in haystack.as_bytes() {
        seen[b as usize] = true;
    }
    pats.iter()
        .any(|p| p.bytes().all(|b| seen[b as usize]) && haystack.contains(p))
}

pub(crate) fn contains_one_of_words(haystack: &str, words: &[&str]) -> bool {
    haystack.split_whitespace().any(|w| words.contains(&w))
}

pub(crate) fn is_video_url(haystack: &str) -> bool {
    VIDEO_DOMAINS.iter().any(|&p| {
        haystack.find(p).is_some_and(|pos| {
            pos > 1 && &haystack[pos - PROTOCOL_PFX.len()..pos] == PROTOCOL_PFX
                || pos > 5 && haystack.get(pos - WWW_PFX.len()..pos) == Some(WWW_PFX)
        })
    })
}

pub(crate) fn is_sentence(text: &str) -> bool {
    text.ends_with('.') || text.contains(". ")
}

pub(crate) fn is_img_attr_to_srcset(s: &str) -> bool {
    for ext in IMG_EXT {
        let mut start = 0;
        while let Some(pos) = s[start..].find(ext) {
            let idx = start + pos + ext.len();
            if idx + 1 < s.len() {
                let bytes = s.as_bytes();
                if bytes[idx].is_ascii_whitespace() && bytes[idx + 1].is_ascii_digit() {
                    return true;
                }
            }
            start = idx;
        }
    }
    false
}

pub(crate) fn is_img_attr_to_src(s: &str) -> bool {
    s.trim()
        .split('.')
        .skip(1)
        .any(|part| IMG_EXT.iter().any(|ext| part.starts_with(&ext[1..])))
}

pub(crate) fn is_loading_word(text: &str) -> bool {
    LOADING_WORDS.contains(&text.trim_end_matches(['…', '.']))
}

pub(crate) fn contains_share_elements(value: &str) -> bool {
    value
        .split([' ', '_'])
        .any(|part| SHARE_WORDS.iter().any(|w| part.eq_ignore_ascii_case(w)))
}

pub(crate) fn has_image_address(t: &Tree, n: Id) -> bool {
    t.attrs(n).iter().any(|(k, v)| {
        (k == "src" || k == "srcset") && {
            let v = v.trim();
            !v.is_empty() && !is_data_url(v)
        }
    })
}

fn is_data_url(src: &str) -> bool {
    src.get(..DATA_URL_PFX.len())
        .is_some_and(|p| p.eq_ignore_ascii_case(DATA_URL_PFX))
}

pub(crate) fn split_base64_url(src: &str) -> Option<(&str, &str)> {
    let rest = src.strip_prefix(DATA_URL_PFX)?;
    let pos = rest.find(BASE64_MARKER)?;
    let data = &rest[pos + BASE64_MARKER.len()..];
    (!data.is_empty()).then(|| (&rest[..pos], data))
}

pub(crate) fn determine_node_score(t: &Tree, n: Id, weigh_classes: bool) -> f32 {
    let score = match t.name(n) {
        Some("div") => 5.0,
        Some("pre" | "td" | "blockquote") => 3.0,
        Some("address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form") => -3.0,
        Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th") => -5.0,
        Some(_) => 0.0,
        None => return 0.0,
    };
    score + class_weight(t, n, weigh_classes)
}

pub(crate) fn class_weight(t: &Tree, n: Id, weigh_classes: bool) -> f32 {
    if !weigh_classes {
        return 0.0;
    }
    let mut weight = 0.0;
    for key in ["class", "id"] {
        if let Some(v) = t.attr(n, key) {
            weight += attr_weight(&v.to_ascii_lowercase());
        }
    }
    weight
}

fn attr_weight(attr: &str) -> f32 {
    let mut weight = 0.0;
    if contains_any(attr, CLASSES_NEGATIVE) || contains_one_of_words(attr, CLASSES_NEGATIVE_WORDS) {
        weight -= 25.0;
    }
    if contains_any(attr, CLASSES_POSITIVE) {
        weight += 25.0;
    }
    weight
}

pub(crate) fn init_node_score(t: &mut Tree, n: Id, weigh_classes: bool) -> f32 {
    let score = determine_node_score(t, n, weigh_classes);
    t.nodes[n].score = Some(score);
    score
}

pub(crate) fn score(t: &Tree, n: Id) -> f32 {
    t.nodes[n].score.unwrap_or(0.0)
}

pub(crate) fn has_score(t: &Tree, n: Id) -> bool {
    t.nodes[n].score.is_some()
}

pub(crate) fn comma_count(t: &Tree, n: Id) -> usize {
    t.descendants(n)
        .filter_map(|d| t.text_of(d))
        .map(|s| s.chars().filter(|c| COMMAS.contains(c)).count())
        .sum()
}
