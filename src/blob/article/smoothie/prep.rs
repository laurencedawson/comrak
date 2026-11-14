//! From dom_smoothie 0.19.0's `prep_article.rs`: clearing the gathered article of what is not the
//! article. Presentational attributes stay, as nothing reads them.

use std::borrow::Cow;

use crate::blob::article::tree::{Id, Tree};

use super::glob::*;
use super::grab::Flags;
use super::helpers::*;

const DATA_TABLE: &str = "data-readability-table";

pub(crate) fn prep_article(t: &mut Tree, article: Id, flags: Flags) {
    // The order matters, as dom_smoothie notes
    remove_share_elements(t, article);
    mark_data_tables(t, article);
    fix_lazy_images(t, article);
    clean_conditionally(t, article, &["form", "fieldset"], flags);
    clean(t, article);
    for h in t.select(article, &["h1", "h2"]) {
        if class_weight(t, h, flags.weight_classes) < 0.0 {
            t.detach(h);
        }
    }
    clean_conditionally(t, article, &["table", "ul", "div"], flags);
    for h in t.select(article, &["h1"]) {
        t.rename(h, "h2");
    }
    for p in t.select(article, &["p"]) {
        if t.select(p, &["img", "object", "embed", "iframe"])
            .is_empty()
            && t.char_count(p) == 0
        {
            t.detach(p);
        }
    }
    for br in t.select(article, &["br"]) {
        if t.next_element_sibling(br).is_some_and(|n| t.is(n, "p")) {
            t.detach(br);
        }
    }
    fix_single_cell_tables(t, article);
}

fn clean(t: &mut Tree, root: Id) {
    for n in t.select(root, CLEAN) {
        // Video embeds stay, as people want to see them
        let keep = t.name(n).is_some_and(|name| EMBED_ELEMENTS.contains(&name))
            && t.attrs(n).iter().any(|(_, v)| is_video_url(v));
        if !keep {
            t.detach(n);
        }
    }
}

/// Whether `matches` under `n` hold more (or, by `greater`, less) than `threshold` of its words.
fn text_density_cmp(
    t: &Tree,
    n: Id,
    matches: &[&str],
    char_count: usize,
    threshold: f32,
    greater: bool,
) -> bool {
    if char_count == 0 {
        return if greater {
            0.0 > threshold
        } else {
            0.0 < threshold
        };
    }
    let total = char_count as f32;
    let mut children = 0usize;
    for d in t.select(n, matches) {
        children += t.char_count(d);
        if children as f32 / total > threshold {
            return greater;
        }
    }
    let density = children as f32 / total;
    if greater {
        density > threshold
    } else {
        density < threshold
    }
}

fn is_data_table(t: &Tree, n: Id) -> bool {
    t.is(n, "table") && t.has_attr(n, DATA_TABLE)
}

fn should_clean_conditionally(t: &Tree, n: Id, flags: Flags) -> bool {
    if is_data_table(t, n)
        || t.descendants(n).any(|d| is_data_table(t, d))
        || has_ancestor(t, n, None, |a| is_data_table(t, a))
    {
        return false;
    }
    let weight = class_weight(t, n, flags.weight_classes);
    if weight < 0.0 {
        return !t.descendants(n).any(|d| t.is(d, "pre"));
    }
    let text = t.text(n);
    if text.matches(',').count() >= 10 {
        return false;
    }

    let mut embeds = 0;
    for e in t.select(n, &["object", "embed", "iframe"]) {
        if t.attrs(e).iter().any(|(_, v)| is_video_url(v)) {
            return false;
        }
        embeds += 1;
    }
    let low = text.trim().to_lowercase();
    if AD_WORDS.contains(&low.as_str()) || is_loading_word(&low) {
        return true;
    }

    let char_count = t.char_count(n);
    let is_list = matches!(t.name(n), Some("ul" | "ol"))
        || text_density_cmp(t, n, &["ul", "ol"], char_count, 0.9, true);
    let img = t.select(n, &["img"]).len();
    let should_remove = || {
        let in_figure = has_ancestor(t, n, None, |a| t.is(a, "figure"));
        let p = t.select(n, &["p"]).len() as f32;
        let img_f = img as f32;
        if !in_figure && img_f > 1.0 && p / img_f < 0.5 {
            return true;
        }
        let li = t.select(n, &["li"]).len() as f32 - 100.0;
        if !is_list && li > p {
            return true;
        }
        let input = t.select(n, &["input"]).len() as f32;
        if input > (p / 3.0).floor() {
            return true;
        }
        let density = link_density(t, n, Some(char_count));
        if !is_list
            && !in_figure
            && char_count < 25
            && (img == 0 || img > 2)
            && density > 0.0
            && text_density_cmp(t, n, H1_H6, char_count, 0.9, false)
        {
            return true;
        }
        if !is_list && weight < 25.0 && density > 0.2 {
            return true;
        }
        if weight >= 25.0 && density > 0.5 {
            return true;
        }
        if (embeds == 1 && char_count < 75) || embeds > 1 {
            return true;
        }
        img == 0 && !text_density_cmp(t, n, TEXTISH, 1, 0.0, true)
    };
    let remove = should_remove();
    if is_list && remove {
        // A list of only pictures, one to an item, is a gallery
        if t.children(n).any(|c| t.element_children(c).len() > 1) {
            return remove;
        }
        return img != t.select(n, &["li"]).len();
    }
    remove
}

fn clean_conditionally(t: &mut Tree, root: Id, tags: &[&str], flags: Flags) {
    if !flags.clean_conditionally {
        return;
    }
    // Innermost first, so a child is judged before its parent
    for n in t.select(root, tags).into_iter().rev() {
        if should_clean_conditionally(t, n, flags) {
            t.detach(n);
        }
    }
}

fn mark_data_tables(t: &mut Tree, root: Id) {
    for table in t.select(root, &["table"]) {
        let data = if t.attr(table, "role") == Some("presentation")
            || t.attr(table, "datatable") == Some("0")
        {
            false
        } else if t.has_attr(table, "summary") || !t.select(table, TABLE_MEMBERS).is_empty() {
            true
        } else if !t.select(table, &["table"]).is_empty() {
            false
        } else {
            let (rows, cols) = rows_and_cols(t, table);
            if rows == 1 || cols == 1 {
                false
            } else {
                rows >= 10 || cols > 4 || rows.saturating_mul(cols) > 10
            }
        };
        if data {
            t.set_attr(table, DATA_TABLE, Cow::Borrowed("true"));
        } else {
            t.remove_attr(table, DATA_TABLE);
        }
    }
}

fn rows_and_cols(t: &Tree, table: Id) -> (usize, usize) {
    let mut rows = 0;
    let mut cols = 0;
    for tr in t.select(table, &["tr"]) {
        rows += 1;
        let mut in_row = 0usize;
        for td in t.select(tr, &["td"]) {
            let raw = t.attr(td, "colspan").unwrap_or("1");
            let span = match raw.parse::<usize>() {
                Ok(v) => v.clamp(1, 1000),
                Err(_) if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) => 1000,
                Err(_) => 1,
            };
            in_row = in_row.saturating_add(span);
        }
        cols = cols.max(in_row);
    }
    (rows, cols)
}

/// A `lazy` class always marks a placeholder; `loading="lazy"` only while there is no address.
fn is_lazy_image(t: &Tree, n: Id) -> bool {
    t.attr(n, "class").is_some_and(|c| c.contains("lazy"))
        || (t.is(n, "img") && t.attr(n, "loading") == Some("lazy") && !has_image_address(t, n))
}

fn fix_lazy_images(t: &mut Tree, root: Id) {
    for n in t.select(root, &["img", "picture", "figure"]) {
        // A tiny base64 picture in `src` beside a real address elsewhere is a placeholder
        if let Some((kind, data)) = t.attr(n, "src").and_then(split_base64_url) {
            if kind == "image/svg+xml" {
                continue;
            }
            let placeholder = data.len() < 133
                && t.attrs(n)
                    .iter()
                    .any(|(k, v)| k != "src" && IMG_EXT.iter().any(|e| v.contains(e)));
            if placeholder {
                t.remove_attr(n, "src");
            }
        }

        if (t.has_attr(n, "src") || t.has_attr(n, "srcset")) && !is_lazy_image(t, n) {
            continue;
        }
        let attrs: Vec<(String, String)> = t
            .attrs(n)
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        for (k, v) in attrs {
            if matches!(k.as_str(), "src" | "srcset" | "alt") {
                continue;
            }
            let low = v.to_ascii_lowercase();
            let copy_to = if is_img_attr_to_srcset(&low) {
                "srcset"
            } else if is_img_attr_to_src(&low) {
                "src"
            } else {
                continue;
            };
            if matches!(t.name(n), Some("img" | "picture")) {
                t.set_attr(n, copy_to, Cow::Owned(v));
            } else if t.select(n, &["img", "picture"]).is_empty() {
                // A figure with no picture in it gets one
                let img = t.new_element("img");
                t.set_attr(img, copy_to, Cow::Owned(v));
                t.append(n, img);
            }
        }
    }
}

fn fix_single_cell_tables(t: &mut Tree, root: Id) {
    for table in t.select(root, &["table"]) {
        let tbody = single_child_element(t, table, "tbody").unwrap_or(table);
        if let Some(row) = single_child_element(t, tbody, "tr")
            && let Some(cell) = single_child_element(t, row, "td")
        {
            let phrasing = t
                .children(cell)
                .collect::<Vec<_>>()
                .into_iter()
                .all(|c| is_phrasing_content(t, c));
            t.rename(cell, if phrasing { "p" } else { "div" });
            t.replace_with(table, cell);
        }
    }
}

fn remove_share_elements(t: &mut Tree, root: Id) {
    let candidates: Vec<Id> = t
        .descendants(root)
        .filter(|&d| t.has_attr(d, "class") || t.has_attr(d, "id"))
        .collect();
    for n in candidates {
        let share = t.attr(n, "class").is_some_and(contains_share_elements)
            || t.attr(n, "id").is_some_and(contains_share_elements);
        if share && t.char_count(n) < CHAR_THRESHOLD {
            t.detach(n);
        }
    }
}
