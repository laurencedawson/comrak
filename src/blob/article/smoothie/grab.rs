//! From dom_smoothie 0.19.0's `grab.rs`: scoring the page's blocks and gathering the best of them,
//! with Readability.js's way of finding a common ancestor (dom_smoothie's own left behind).

use std::collections::HashMap;

use crate::blob::article::tree::{Id, Tree};

use super::glob::*;
use super::helpers::*;
use super::prep::prep_article;

#[derive(Clone, Copy)]
pub(crate) struct Flags {
    pub strip_unlikelys: bool,
    pub weight_classes: bool,
    pub clean_conditionally: bool,
}

/// Up to four tries at the article, each on a fresh copy of `tree` with one more of the flags let go:
/// the first long enough, or else the longest.
pub(crate) fn grab_article<'a>(tree: &Tree<'a>, body: Id, title: &str) -> Option<(Tree<'a>, Id)> {
    let mut flags = Flags {
        strip_unlikelys: true,
        weight_classes: true,
        clean_conditionally: true,
    };
    let mut best: Option<(Tree<'a>, Id, usize)> = None;
    loop {
        let mut t = tree.clone();
        if let Some(article) = attempt(&mut t, body, flags, title) {
            let len = t.char_count(article);
            if len >= CHAR_THRESHOLD {
                return Some((t, article));
            }
            if best.as_ref().is_none_or(|b| len > b.2) {
                best = Some((t, article, len));
            }
        }
        if flags.strip_unlikelys {
            flags.strip_unlikelys = false;
        } else if flags.weight_classes {
            flags.weight_classes = false;
        } else if flags.clean_conditionally {
            flags.clean_conditionally = false;
        } else {
            return best.map(|(t, a, _)| (t, a));
        }
    }
}

fn attempt(t: &mut Tree, body: Id, flags: Flags, title: &str) -> Option<Id> {
    let to_score = collect_elements_to_score(t, body, flags.strip_unlikelys, title);
    let weigh = flags.weight_classes;
    let top_candidates = score_elements(t, &to_score, weigh);

    let tc = match top_candidates.first() {
        Some(&c) if !t.is(c, "body") => {
            let mut tc = find_common_candidate(t, c, &top_candidates, weigh);
            // The only child of its parent: the parent, so the siblings below are its siblings
            while let Some(p) = t.parent(tc) {
                if t.is(p, "body") || t.element_children(p).len() != 1 {
                    break;
                }
                tc = p;
            }
            tc
        }
        _ => {
            let tc = t.new_element("div");
            t.reparent_children(body, tc);
            t.append(body, tc);
            init_node_score(t, tc, weigh);
            tc
        }
    };
    if !has_score(t, tc) {
        init_node_score(t, tc, weigh);
    }
    let article = t.new_element("div");
    assign_article_node(t, tc, article);
    prep_article(t, article, flags);
    Some(article)
}

fn is_unlikely_candidate(t: &Tree, n: Id) -> bool {
    if t.is(n, "a") {
        return false;
    }
    let s = matching_string(t, n);
    if s.is_empty() || !contains_any(&s, UNLIKELY_CANDIDATES) || contains_any(&s, MAYBE_CANDIDATES)
    {
        return false;
    }
    !has_ancestor(t, n, Some(0), |a| {
        matches!(t.name(a), Some("table" | "code"))
    })
}

/// Puts each run of phrasing content directly under `n` in a `<p>` of its own.
fn div_into_p(t: &mut Tree, n: Id) {
    let mut child = t.first_child(n);
    while let Some(c) = child {
        child = wrap_phrasing_content(t, c);
    }
}

fn wrap_phrasing_content(t: &mut Tree, n: Id) -> Option<Id> {
    if !is_phrasing_content(t, n) || is_whitespace(t, n) {
        return t.next_sibling(n);
    }
    let mut next = t.next_sibling(n);
    let p = t.new_element("p");
    t.insert_before(n, p);
    t.append(p, n);
    while let Some(c) = next {
        if !is_phrasing_content(t, c) {
            break;
        }
        next = t.next_sibling(c);
        t.append(p, c);
    }
    while let Some(last) = t.last_child(p) {
        if !is_whitespace(t, last) {
            break;
        }
        t.detach(last);
    }
    next
}

fn has_child_block_element(t: &Tree, n: Id) -> bool {
    t.descendants(n)
        .any(|d| t.name(d).is_some_and(|name| BLOCK_ELEMS.contains(&name)))
}

fn score_elements(t: &mut Tree, to_score: &[Id], weigh: bool) -> Vec<Id> {
    let mut scores: HashMap<Id, f32> = HashMap::new();
    let mut order: Vec<Id> = Vec::new();
    let mut counts: HashMap<Id, usize> = HashMap::new();
    let mut count = |t: &Tree, n: Id| *counts.entry(n).or_insert_with(|| t.char_count(n));

    for &el in to_score {
        let len = count(t, el);
        if len < 25 {
            continue;
        }
        let content_score = 2 + comma_count(t, el) + (len / 100).min(3);
        let ancestors: Vec<Id> = t.ancestors(el).take(5).collect();
        for (level, &a) in ancestors.iter().enumerate() {
            if !t.is_element(a) || t.parent(a).is_none() {
                continue;
            }
            let divider = match level {
                0 => 1.0,
                1 => 2.0,
                _ => (level * 3) as f32,
            };
            let extra = content_score as f32 / divider;
            match scores.get_mut(&a) {
                Some(s) => *s += extra,
                None => {
                    order.push(a);
                    scores.insert(a, extra + determine_node_score(t, a, weigh));
                }
            }
            if t.is(a, "body") {
                break;
            }
        }
    }

    // Good content has few links: a candidate's score falls with its link density
    let mut scored: Vec<(Id, f32)> = Vec::new();
    for a in order {
        let base = scores[&a];
        if base <= 0.0 {
            continue;
        }
        let s = if base > MIN_SCORE_TO_ADJUST {
            let mut link_length = 0f32;
            for l in t.select(a, &["a"]) {
                let href = t.attr(l, "href").unwrap_or("");
                let coeff = if href.len() > 1 && href.starts_with('#') {
                    0.3
                } else {
                    1.0
                };
                link_length += count(t, l) as f32 * coeff;
            }
            let density = if link_length == 0.0 {
                0.0
            } else {
                let len = count(t, a) as f32;
                if len == 0.0 { 0.0 } else { link_length / len }
            };
            base * (1.0 - density)
        } else {
            base
        };
        t.nodes[a].score = Some(s);
        scored.push((a, s));
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    scored
        .into_iter()
        .take(N_TOP_CANDIDATES)
        .map(|c| c.0)
        .collect()
}

/// Moves the top candidate, and those of its siblings that read as part of it, into `article`, which
/// takes their place under their parent.
fn assign_article_node(t: &mut Tree, tc: Id, article: Id) {
    let tc_score = score(t, tc);
    let threshold = (tc_score * 0.2).max(10.0);
    let parent = t.parent(tc).expect("the top candidate has a parent");
    let tc_class = t.attr(tc, "class").unwrap_or("").to_string();
    for sibling in t.element_children(parent) {
        let mut append = sibling == tc;
        if !append {
            let mut bonus = 0.0;
            if !tc_class.is_empty() && t.attr(sibling, "class").unwrap_or("") == tc_class {
                bonus += tc_score * 0.2;
            }
            let s = score(t, sibling);
            if s > 0.0 {
                append = s + bonus >= threshold;
            } else if t.is(sibling, "p") {
                let content = normalize_spaces(&t.text(sibling));
                let len = content.chars().count();
                let density = link_density(t, sibling, Some(len));
                append = (len > 80 && density < 0.25)
                    || (len < 80 && len > 0 && density == 0.0 && is_sentence(&content));
            }
        }
        if append {
            if !t
                .name(sibling)
                .is_some_and(|n| ALTER_TO_DIV_EXCEPTIONS.contains(&n))
            {
                t.rename(sibling, "div");
            }
            t.append(article, sibling);
        }
    }
    t.append(parent, article);
}

fn find_common_candidate(t: &mut Tree, mut tc: Id, top: &[Id], weigh: bool) -> Id {
    let tc_score = score(t, tc);
    let alternatives: Vec<Vec<Id>> = top
        .iter()
        .skip(1)
        .filter(|&&alt| score(t, alt) / tc_score >= 0.75)
        .map(|&alt| t.ancestors(alt).collect())
        .collect();
    if alternatives.len() > MIN_COMMON_ANCESTORS {
        let mut parent = t.parent(tc);
        while let Some(p) = parent {
            if t.is(p, "body") {
                break;
            }
            let lists = alternatives.iter().filter(|a| a.contains(&p)).count();
            if lists >= MIN_COMMON_ANCESTORS {
                tc = p;
                break;
            }
            parent = t.parent(p);
        }
    }
    adjust_top_candidate_by_parent(t, tc, weigh)
}

fn adjust_top_candidate_by_parent(t: &mut Tree, tc: Id, weigh: bool) -> Id {
    if !has_score(t, tc) {
        init_node_score(t, tc, weigh);
    }
    // A parent scoring higher than its child, a few steps up, has more of the article under it
    let mut last = score(t, tc);
    let threshold = last / 3.0;
    let mut parent = t.parent(tc);
    while let Some(p) = parent {
        if t.is(p, "body") {
            break;
        }
        if !has_score(t, p) {
            parent = t.parent(p);
            continue;
        }
        let s = score(t, p);
        if s < threshold {
            break;
        }
        if s > last {
            return p;
        }
        last = s;
        parent = t.parent(p);
    }
    tc
}

/// The blocks to score, in document order; on the way, drops what is hidden, a dialog, the title
/// again, unlikely to be the article, or empty, and turns divs of words into paragraphs.
fn collect_elements_to_score(t: &mut Tree, body: Id, strip_unlikely: bool, title: &str) -> Vec<Id> {
    let mut out = Vec::new();
    let mut remove_title = !title.is_empty();
    let mut next = next_child_or_sibling(t, body, false);
    while let Some(mut n) = next {
        let visible = is_probably_visible(t, n);
        if visible && t.is(n, "svg") {
            next = next_child_or_sibling(t, n, true);
            continue;
        }
        let title_again = remove_title
            && matches!(t.name(n), Some("h1" | "h2"))
            && text_similarity(title, &t.text(n)) > 0.75;
        if title_again {
            remove_title = false;
        }
        let drop = !visible
            || t.is(n, "dialog")
            || (t.attr(n, "aria-modal") == Some("true") && t.attr(n, "role") == Some("dialog"))
            || title_again
            || (strip_unlikely
                && (is_unlikely_candidate(t, n)
                    || t.attr(n, "role")
                        .is_some_and(|r| UNLIKELY_ROLES.contains(&r))))
            || (t
                .name(n)
                .is_some_and(|name| TAGS_WITH_CONTENT.contains(&name))
                && is_element_without_content(t, n));
        if drop {
            next = next_child_or_sibling(t, n, true);
            t.detach(n);
            continue;
        }
        if t.name(n)
            .is_some_and(|name| DEFAULT_TAGS_TO_SCORE.contains(&name))
        {
            out.push(n);
        }
        if t.is(n, "div") {
            div_into_p(t, n);
            // A div holding one paragraph and no words of its own is that paragraph
            let single_p =
                single_child_element(t, n, "p").filter(|_| link_density(t, n, None) < 0.25);
            if let Some(p) = single_p {
                t.replace_with(n, p);
                out.push(p);
                n = p;
            } else if !has_child_block_element(t, n) {
                t.rename(n, "p");
                out.push(n);
            }
        }
        next = next_child_or_sibling(t, n, false);
    }
    out
}
