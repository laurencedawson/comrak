//! From dom_smoothie 0.19.0 (MIT, see `smoothie/LICENSE`), Mykola Humanov's port of Mozilla's
//! Readability.js: finding a page's article, run on [`Tree`] in place of dom_query.
//! Readability.js itself is Apache-2.0, Arc90's and Mozilla's: its license and notice are
//! `smoothie/LICENSE-readability` and `smoothie/NOTICE`.
//!
//! - `glob.rs`: the numbers and word lists
//! - `helpers.rs`: from `helpers.rs`, `matching.rs` and `score.rs`
//! - `grab.rs`: scoring the page's blocks and gathering the best
//! - `prep.rs`: from `prep_article.rs`, clearing the article of what is not
//!
//! Left behind: JSON-LD and the page's other metadata but its title and picture, text
//! modes, the readability check, dom_smoothie's own candidate mode, and what the walk does itself:
//! resolving addresses, leaving `javascript:` links words, and reading nested and empty divs as
//! nothing.

mod glob;
mod grab;
mod helpers;
mod prep;

use super::tree::{Id, ROOT, Tree};

use glob::*;
use helpers::*;

pub struct Article<'a> {
    /// The page's title: its metadata's, else its `<title>` with the site's name cut off.
    pub title: String,
    /// The picture the page's metadata leads with, its address as written.
    // Read as dom_smoothie reads it; the walk shows the article's own pictures, the preview card being
    // a logo as often as not
    #[allow(dead_code)]
    pub image: Option<String>,
    /// The article's title as the page's JSON-LD heads it; none where it gives an article none.
    pub headline: Option<String>,
    /// The site's own name, as the page's metadata gives it.
    pub site: Option<String>,
    /// Who wrote it, as the page's JSON-LD names its authors, comma separated; none where it names none.
    pub byline: Option<String>,
    pub tree: Tree<'a>,
    pub root: Id,
}

/// The article on the page `html`, or none where nothing on it reads as one.
pub fn extract(html: &str) -> Option<Article<'_>> {
    let mut tree = Tree::parse(html);
    let title = metadata_title(&tree).unwrap_or_else(|| article_title(&tree));
    let body = tree.select(ROOT, &["body"]).into_iter().next()?;
    // The byline element is the page's chrome, so it goes, whether or not the metadata names the
    // author; the author comes from the page's JSON-LD alone, which names one cleanly where the
    // element reads "By", a date or an empty list's "No items found." (25 Hacker News pages, 2026-10)
    drop_byline(&mut tree, body);
    let ld = ld_json(html);
    let authors = ld_authors(&ld);
    let byline = (!authors.is_empty()).then(|| authors.join(", "));
    let image = meta(&tree, META_IMAGE_KEYS);
    let headline = ld_headline(&ld).map(|h| plain(&h));
    let site = meta(&tree, &["og:site_name"]);
    prepare(&mut tree);
    let (mut tree, root) = grab::grab_article(&tree, body, &title)?;
    if tree.char_count(root) == 0 {
        return None;
    }
    unwrap_stray_cells(&mut tree, root);
    drop_dates(&mut tree, root);
    Some(Article {
        title,
        image,
        headline,
        site,
        byline,
        tree,
        root,
    })
}

fn metadata_title(t: &Tree) -> Option<String> {
    meta(t, META_TITLE_KEYS)
}

/// The first of `keys` a `<meta>` names, by `property` or `name`, with words in its `content`.
fn meta(t: &Tree, keys: &[&str]) -> Option<String> {
    let metas = t.select(ROOT, &["meta"]);
    keys.iter().find_map(|key| {
        metas.iter().find_map(|&m| {
            let named = t
                .attr(m, "property")
                .is_some_and(|p| p.split_whitespace().any(|p| p == *key))
                || t.attr(m, "name")
                    .is_some_and(|n| n.eq_ignore_ascii_case(key));
            let content = t.attr(m, "content")?.trim();
            (named && !content.is_empty()).then(|| content.to_string())
        })
    })
}

/// Takes the first element under `body` that reads as a byline out of the page.
fn drop_byline(t: &mut Tree, body: Id) {
    // A byline stands between the title and the first paragraph of the body, like a dateline:
    // past that an element named for an author is content, a box about one in a profile piece
    let first = body_start(t, body);
    let mut next = helpers::next_child_or_sibling(t, body, false);
    while let Some(n) = next {
        if Some(n) == first {
            return;
        }
        let marked = t.attr(n, "rel") == Some("author")
            || t.attr(n, "itemprop").is_some_and(|p| p.contains("author"));
        let byline = (marked || contains_any(&matching_string(t, n), BYLINE_PATTERNS))
            && (1..100).contains(&t.text(n).trim().chars().count());
        if byline {
            t.detach(n);
            return;
        }
        next = helpers::next_child_or_sibling(t, n, false);
    }
}

/// The authors the page's JSON-LD names, in order and each once: the `name` of each `author`,
/// an object, an array of them, or a string, under any object of the document. Parsed with
/// tinyjson, 19 KB of native library against a reader of our own.
pub(crate) fn ld_authors(ld: &[tinyjson::JsonValue]) -> Vec<String> {
    use tinyjson::JsonValue;
    fn walk(v: &JsonValue, out: &mut Vec<String>) {
        match v {
            JsonValue::Object(o) => {
                if let Some(a) = o.get("author") {
                    author(a, out);
                }
                o.values().for_each(|v| walk(v, out));
            }
            JsonValue::Array(a) => a.iter().for_each(|v| walk(v, out)),
            _ => {}
        }
    }
    fn author(v: &JsonValue, out: &mut Vec<String>) {
        match v {
            JsonValue::String(s) => name(s, out),
            JsonValue::Object(o) => {
                if let Some(JsonValue::String(s)) = o.get("name") {
                    name(s, out);
                }
            }
            JsonValue::Array(a) => a.iter().for_each(|v| author(v, out)),
            _ => {}
        }
    }
    fn name(s: &str, out: &mut Vec<String>) {
        if !s.trim().is_empty() && !out.iter().any(|n| n == s) {
            out.push(s.to_string());
        }
    }
    let mut out = Vec::new();
    ld.iter().for_each(|v| walk(v, &mut out));
    out
}

/// The page's `ld+json` scripts that parse, in order, parsed once for every reader of them.
fn ld_json(html: &str) -> Vec<tinyjson::JsonValue> {
    let bytes = html.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(at) = memchr::memmem::find(&bytes[from..], b"ld+json").map(|i| from + i) {
        let Some(open) = memchr::memchr(b'>', &bytes[at..]).map(|i| at + i + 1) else {
            break;
        };
        let close =
            memchr::memmem::find(&bytes[open..], b"</script").map_or(bytes.len(), |i| open + i);
        if let Ok(v) = html[open..close].parse() {
            out.push(v);
        }
        from = close;
    }
    out
}

/// The first `headline` the page's JSON-LD gives an article, a `NewsArticle`, `BlogPosting` and
/// the like, under any object of the document; a `WebPage`'s `name` is the page's title, site and
/// all, so it is not read.
pub(crate) fn ld_headline(ld: &[tinyjson::JsonValue]) -> Option<String> {
    use tinyjson::JsonValue;
    fn article(t: Option<&JsonValue>) -> bool {
        let is = |t: &JsonValue| matches!(t, JsonValue::String(s) if s.ends_with("Article") || s == "BlogPosting" || s == "Report");
        match t {
            Some(JsonValue::Array(a)) => a.iter().any(is),
            Some(t) => is(t),
            None => false,
        }
    }
    fn find(v: &JsonValue) -> Option<&String> {
        match v {
            JsonValue::Object(o) => article(o.get("@type"))
                .then(|| o.get("headline"))
                .flatten()
                .and_then(|h| h.get::<String>())
                .filter(|h| !h.trim().is_empty())
                .or_else(|| o.values().find_map(find)),
            JsonValue::Array(a) => a.iter().find_map(find),
            _ => None,
        }
    }
    ld.iter().find_map(find).cloned()
}

/// `s` as the page would show it: tags gone, entities decoded, spaces collapsed. JSON-LD's strings
/// are HTML as WordPress writes them (`We&#8217;re`), which no parse of the page decodes.
fn plain(s: &str) -> String {
    normalize_spaces(&Tree::parse(s).text(ROOT))
}

/// The page's `<title>`, its site's name cut off where that can be told apart.
fn article_title(t: &Tree) -> String {
    let raw = t
        .select(ROOT, &["title"])
        .first()
        .map(|&n| t.text(n))
        .unwrap_or_default();
    let orig = raw.trim();
    let mut cur = orig.to_string();
    let mut h1 = None;
    let mut hierarchy = false;
    if orig.chars().any(|c| TITLE_SEPARATORS.contains(&c)) {
        hierarchy = orig.chars().any(|c| TITLE_HIERARCHY_SEP.contains(&c));
        if let Some(part) = truncate_title_last(orig) {
            cur = part.to_string();
        }
        if cur.split_whitespace().count() < 3
            && let Some(part) = truncate_title_first(orig)
        {
            cur = part.to_string();
        }
    } else if cur.contains(": ") {
        let matched = t
            .select(ROOT, &["h1", "h2"])
            .iter()
            .any(|&h| t.text(h).trim() == cur);
        if !matched && let Some(idx) = orig.rfind(':') {
            cur = orig[idx + 1..].trim().to_string();
            if cur.split_whitespace().count() < 3 {
                if let Some(idx) = orig.find(':') {
                    cur = orig[idx + 1..].trim().to_string();
                }
            } else if orig
                .find(':')
                .map_or(0, |idx| orig[..=idx].split_whitespace().count())
                > 5
            {
                cur = orig.to_string();
            }
        }
    } else if !(15..=150).contains(&orig.chars().count()) {
        h1 = t.select(ROOT, &["h1"]).first().map(|&n| t.text(n));
    }
    if let Some(h1) = h1 {
        cur = h1;
    }
    let cur = normalize_spaces(&cur);
    let words = cur.split_whitespace().count();
    let orig_words = orig
        .split(TITLE_SEPARATORS)
        .flat_map(str::split_whitespace)
        .count();
    if words <= 4 && (!hierarchy || words + 1 != orig_words) {
        return orig.to_string();
    }
    cur
}

fn truncate_title_last(title: &str) -> Option<&str> {
    let (at, sep) = title
        .char_indices()
        .rev()
        .find(|(_, c)| TITLE_SEPARATORS.contains(c))?;
    let next = title.get(at + sep.len_utf8()..)?.chars().next()?;
    (next == ' ').then(|| title[..at].trim())
}

fn truncate_title_first(title: &str) -> Option<&str> {
    let (at, sep) = title
        .char_indices()
        .find(|(_, c)| TITLE_SEPARATORS.contains(c))?;
    let next = title.get(at + sep.len_utf8()..)?.chars().next()?;
    (next == ' ').then(|| title[at + sep.len_utf8()..].trim())
}

fn prepare(t: &mut Tree) {
    remove_empty_imgs(t);
    unwrap_noscript_images(t);
    for n in t.select(ROOT, &["noscript"]) {
        t.detach(n);
    }
    replace_brs(t);
    for f in t.select(ROOT, &["font"]) {
        t.rename(f, "span");
        t.clear_attrs(f);
    }
}

fn remove_empty_imgs(t: &mut Tree) {
    for img in t.select(ROOT, &["img"]) {
        let has_src = t.attrs(img).iter().any(|(k, v)| {
            matches!(&**k, "src" | "data-src" | "data-srcset" | "srcset")
                || IMG_EXT.iter().any(|e| v.contains(e))
        });
        if !has_src {
            t.detach(img);
        }
    }
}

/// A `<noscript>` holding only a picture, after a picture: the noscript's is the real one.
fn unwrap_noscript_images(t: &mut Tree) {
    for ns in t.select(ROOT, &["noscript"]) {
        let children = t.element_children(ns);
        let [new_img] = children[..] else { continue };
        if !t.is(new_img, "img") || t.children(ns).any(|c| t.is_nonempty_text(c)) {
            continue;
        }
        let Some(prev) = t.prev_element_sibling(ns) else {
            continue;
        };
        let prev_img = if t.is(prev, "img") {
            prev
        } else {
            match t.element_children(prev)[..] {
                [only] if t.is(only, "img") => only,
                _ => continue,
            }
        };
        let attrs: Vec<_> = t.attrs(prev_img).to_vec();
        for (k, v) in attrs {
            if v.is_empty() {
                continue;
            }
            if matches!(&*k, "src" | "srcset") || IMG_EXT.iter().any(|e| v.contains(e)) {
                if t.attr(new_img, &k) == Some(&*v) {
                    continue;
                }
                let key = if t.has_attr(new_img, &k) {
                    format!("data-old-{k}")
                } else {
                    k.to_string()
                };
                t.set_attr(new_img, &key, v);
            }
        }
        t.replace_with(prev_img, new_img);
    }
}

fn next_significant(t: &Tree, mut n: Option<Id>) -> Option<Id> {
    while let Some(c) = n {
        if !t.is_element(c) && t.text(c).trim().is_empty() {
            n = t.next_sibling(c);
        } else {
            break;
        }
    }
    n
}

/// Two or more `<br>`s in a row end a paragraph: the words after them, to the next such run, become
/// a `<p>`.
fn replace_brs(t: &mut Tree) {
    for br in t.select(ROOT, &["br"]) {
        if t.parent(br).is_none() {
            continue;
        }
        let mut next = t.next_sibling(br);
        let mut replaced = false;
        while let Some(n) = next_significant(t, next) {
            if !t.is(n, "br") {
                break;
            }
            replaced = true;
            next = t.next_sibling(n);
            t.detach(n);
        }
        if !replaced {
            continue;
        }
        let p = t.new_element("p");
        t.replace_with(br, p);
        let mut next = t.next_sibling(p);
        while let Some(n) = next {
            if t.is(n, "br")
                && next_significant(t, t.next_sibling(n)).is_some_and(|m| t.is(m, "br"))
            {
                break;
            }
            if !is_phrasing_content(t, n) {
                break;
            }
            next = t.next_sibling(n);
            t.append(p, n);
        }
        while let Some(last) = t.last_child(p) {
            if !is_whitespace(t, last) {
                break;
            }
            t.detach(last);
        }
        if let Some(parent) = t.parent(p)
            && t.is(parent, "p")
        {
            t.rename(parent, "div");
        }
    }
}

/// A cell outside a row gives its first child to its parent and goes, as Readability's
/// `simplify_nested_elements` has it; its nested divs and empty ones it leaves, as the walk reads
/// them the same either way.
fn unwrap_stray_cells(t: &mut Tree, root: Id) {
    for td in t.select(root, &["td"]) {
        let Some(parent) = t.parent(td) else { continue };
        if t.is(parent, "tr") {
            continue;
        }
        if let Some(first) = t.first_child(td) {
            t.append(parent, first);
        }
        t.detach(td);
    }
}

/// Takes the article's date lines out, the header's to show if anyone's: each with the parent it
/// leaves a word or two in, an eyebrow's category.
/// Where the body's words begin under `root`: its first paragraph holding a sentence or twenty
/// words. The title, byline and dateline stand ahead of it.
fn body_start(t: &Tree, root: Id) -> Option<Id> {
    t.descendants(root)
        .find(|&n| t.name(n) == Some("p") && labels(&t.text(n)).is_none_or(|w| w >= 20))
}

fn drop_dates(t: &mut Tree, root: Id) {
    // A dateline stands between the title and the first paragraph of the body: past that a date is
    // content, a recipe's time, a result in a sentence, a release's row
    let body = body_start(t, root);
    let dates: Vec<Id> = t
        .descendants(root)
        .take_while(|&n| Some(n) != body)
        .filter(|&n| is_date(t, n))
        .collect();
    for n in dates {
        // What goes with its date, an eyebrow's category or a reading time and tags: a parent
        // holding a few words besides and no sentence
        let mut out = n;
        while let Some(parent) = t.parent(out)
            && parent != root
            && t.select(parent, &["img", "picture", "video"]).is_empty()
            && labels(&t.text(parent).replacen(t.text(out).as_str(), "", 1)).is_some_and(|w| w <= 4)
        {
            out = parent;
        }
        t.detach(out);
    }
}

/// A short element with no picture that its class or id names a date, or that holds a `<time>`
/// with a word or two about it ("Updated:"); not a `<time>` itself, which may sit in a sentence.
/// Elements that sit inside a sentence: a date in one is the sentence's, never a dateline.
const INLINE: &[&str] = &[
    "span", "a", "em", "strong", "b", "i", "small", "abbr", "sup", "sub", "code",
];

/// Whether `n` is a block that reads as a dateline: named for a date by its class or id, or
/// holding a date, a `<time>` or an element so named, with a label or two about it and no sentence.
fn is_date(t: &Tree, n: Id) -> bool {
    let Some(name) = t.name(n) else { return false };
    if name == "time" || INLINE.contains(&name) {
        return false;
    }
    let text = t.text(n);
    let len = text.trim().chars().count();
    if len == 0 || len >= 80 || !t.select(n, &["img", "picture", "video"]).is_empty() {
        return false;
    }
    if named_for_a_date(t, n) {
        return true;
    }
    let dated: Vec<Id> = t
        .descendants(n)
        .filter(|&d| t.name(d) == Some("time") || named_for_a_date(t, d))
        .collect();
    if dated.is_empty() {
        return false;
    }
    let mut rest = text;
    for &d in &dated {
        rest = rest.replacen(t.text(d).as_str(), "", 1);
    }
    labels(&rest).is_some_and(|w| w <= 4)
}

/// Whether a class or id of `n` has a date's word in it.
fn named_for_a_date(t: &Tree, n: Id) -> bool {
    ["class", "id"].iter().any(|k| {
        t.attr(n, k).is_some_and(|v| {
            v.split([' ', '_', '-'])
                .any(|w| DATE_WORDS.iter().any(|d| w.eq_ignore_ascii_case(d)))
        })
    })
}

/// How many words `text` holds where they read as labels, not a sentence: none ending in `.`, `!`
/// or `?`. A separator, a `·` or `|` with no letter or digit, is no word.
fn labels(text: &str) -> Option<usize> {
    let words = text
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric));
    let mut count = 0;
    for w in words {
        if w.ends_with(['.', '!', '?']) {
            return None;
        }
        count += 1;
    }
    Some(count)
}

#[cfg(test)]
mod survey {
    use super::*;

    /// What the page's JSON-LD names as its authors, on real pages: `HN_PAGES=<dir> cargo test
    /// --lib survey_bylines -- --ignored --nocapture`, one line a page.
    #[test]
    #[ignore]
    fn survey_bylines() {
        let Ok(dir) = std::env::var("HN_PAGES") else {
            return;
        };
        let mut files: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().collect();
        files.sort_by_key(|f| f.file_name());
        for f in files {
            let html = std::fs::read_to_string(f.path()).unwrap_or_default();
            println!(
                "{} | {:?}",
                f.file_name().to_string_lossy(),
                ld_authors(&ld_json(&html))
            );
        }
    }
}
