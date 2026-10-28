//! A page's HTML as a tree the reader can cut: an arena of elements and words, read by mdream's scan
//! with the closes HTML implies (a `<p>` ends at the next block, an `<li>` at the next item) and no
//! more of the standard's repairs.
//!
//! Every element's attributes sit in one pool, each element holding its run of it, so building
//! the tree, and copying it for each of Readability's tries, allocates per tree and not per element.

use std::borrow::Cow;

use crate::blob::html::mdream::entities::{
    decode_html_attribute_entities, decode_html_entities_into,
};
use crate::blob::html::mdream::scan::{Attrs, process_comment_or_doctype, scan_tag};

pub type Id = usize;

pub const ROOT: Id = 0;

/// No node: links are `u32`s, with this for none.
const NONE: u32 = u32::MAX;

type Attr<'a> = (Cow<'a, str>, Cow<'a, str>);

#[derive(Clone)]
pub enum Data<'a> {
    Root,
    /// Its name, and where its attributes start in the pool and how many there are.
    Element(Cow<'a, str>, u32, u32),
    Text(Cow<'a, str>, Count),
}

/// A text node's share of [`Tree::char_count`], worked out once as it is read: how many characters
/// it comes to after one with no whitespace before it, each run of whitespace one, and whether it
/// starts and ends on whitespace.
#[derive(Clone, Copy)]
pub struct Count {
    chars: u32,
    leads: bool,
    trails: bool,
}

impl Count {
    fn of(t: &str) -> Count {
        let mut count = Count {
            chars: 0,
            leads: false,
            trails: false,
        };
        let mut step = |ws: bool| {
            if count.chars == 0 {
                count.leads = ws;
            }
            if !(count.trails && ws) {
                count.chars += 1;
            }
            count.trails = ws;
        };
        // ASCII's whitespace is `char::is_whitespace`'s below 0x80, so its bytes need no decoding
        if t.is_ascii() {
            t.bytes()
                .for_each(|b| step(matches!(b, b'\t'..=b'\r' | b' ')));
        } else {
            t.chars().for_each(|c| step(c.is_whitespace()));
        }
        count
    }
}

#[derive(Clone)]
pub struct Node<'a> {
    pub data: Data<'a>,
    parent: u32,
    first: u32,
    last: u32,
    prev: u32,
    next: u32,
    /// The reader's score, once it has given one.
    pub score: Option<f32>,
}

#[derive(Clone)]
pub struct Tree<'a> {
    pub nodes: Vec<Node<'a>>,
    attrs: Vec<Attr<'a>>,
}

/// Tags with no end and no children.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

/// Tags whose words are not HTML: their contents are skipped to their end tag.
const RAW: &[&str] = &["script", "style", "iframe", "noembed", "noframes", "xmp"];

/// Tags whose words are text, entities and all, to their end tag.
const RCDATA: &[&str] = &["title", "textarea"];

/// Tags that end an open `<p>`.
#[rustfmt::skip]
const CLOSES_P: &[&str] = &[
    "address", "article", "aside", "blockquote", "details", "dialog", "div", "dl", "fieldset",
    "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header",
    "hgroup", "hr", "main", "menu", "nav", "ol", "p", "pre", "section", "table", "ul", "li", "dd",
    "dt",
];

/// Tags a search for an open tag to close does not look past.
const SCOPE: &[&str] = &[
    "table", "td", "th", "caption", "html", "body", "object", "marquee", "button", "template",
];

fn link(id: u32) -> Option<Id> {
    (id != NONE).then_some(id as Id)
}

impl<'a> Tree<'a> {
    pub fn parse(html: &'a str) -> Tree<'a> {
        // About a node to each `<`: a tag, or the words before one
        let mut nodes = Vec::with_capacity(memchr::memchr_iter(b'<', html.as_bytes()).count() + 2);
        nodes.push(Node::new(Data::Root));
        let mut tree = Tree {
            nodes,
            attrs: Vec::new(),
        };
        let mut open: Vec<Id> = vec![ROOT];
        let mut body: Option<Id> = None;
        let bytes = html.as_bytes();
        let mut decoded = String::new();
        let mut i = 0;
        while i < bytes.len() {
            let lt = memchr::memchr(b'<', &bytes[i..]).map_or(bytes.len(), |n| i + n);
            if lt > i {
                tree.push_text(*open.last().unwrap(), &html[i..lt], &mut decoded);
            }
            if lt >= bytes.len() {
                break;
            }
            let next = bytes.get(lt + 1).copied().unwrap_or(0);
            if next == b'!' || next == b'?' {
                let comment = process_comment_or_doctype(html, lt);
                i = if comment.complete {
                    comment.new_position
                } else {
                    bytes.len()
                };
                continue;
            }
            let closing = next == b'/';
            let name_start = lt + 1 + closing as usize;
            let name_end = bytes[name_start..]
                .iter()
                .position(|&c| !(c.is_ascii_alphanumeric() || c == b'-' || c == b':'))
                .map_or(bytes.len(), |n| name_start + n);
            if name_end == name_start || !bytes[name_start].is_ascii_alphabetic() {
                // A `<` that opens no tag is a word
                tree.push_text(*open.last().unwrap(), "<", &mut decoded);
                i = lt + 1;
                continue;
            }
            let name = lower(&html[name_start..name_end]);
            if closing {
                i = memchr::memchr(b'>', &bytes[name_end..])
                    .map_or(bytes.len(), |n| name_end + n + 1);
                tree.end(&mut open, &name);
                continue;
            }
            let from = tree.attrs.len();
            let scan = scan_tag(html, name_end, Attrs::All, |k, v| {
                tree.attrs
                    .push((lower(k), decode_html_attribute_entities(v)));
            });
            if !scan.complete {
                tree.attrs.truncate(from);
                break;
            }
            i = scan.new_position;
            let self_closing = bytes[i - 2] == b'/';
            let skipped = match &*name {
                "html" | "head" => true,
                "body" => body.is_some(),
                "script" | "style" => {
                    i = find_end(bytes, i, &name).1;
                    true
                }
                _ => false,
            };
            if skipped {
                tree.attrs.truncate(from);
                continue;
            }
            let el = tree.add(Data::Element(
                name.clone(),
                from as u32,
                (tree.attrs.len() - from) as u32,
            ));
            if name == "body" {
                tree.append(ROOT, el);
                open.truncate(1);
                open.push(el);
                body = Some(el);
                continue;
            }
            tree.imply_ends(&mut open, &name);
            tree.append(*open.last().unwrap(), el);
            if RAW.contains(&&*name) || RCDATA.contains(&&*name) {
                let (words_end, past) = find_end(bytes, i, &name);
                if RCDATA.contains(&&*name) {
                    tree.push_text(el, &html[i..words_end], &mut decoded);
                }
                i = past;
            } else if !(VOID.contains(&&*name) || self_closing) {
                open.push(el);
            }
        }
        // A template's contents are no part of the page, as the parser keeps them apart
        for t in tree.select(ROOT, &["template"]) {
            tree.detach(t);
        }
        if body.is_none() {
            let b = tree.new_element("body");
            tree.reparent_children(ROOT, b);
            tree.append(ROOT, b);
        }
        tree
    }

    fn push_text(&mut self, parent: Id, raw: &'a str, decoded: &mut String) {
        let text = if decode_html_entities_into(raw, false, false, decoded) {
            Cow::Owned(decoded.clone())
        } else {
            Cow::Borrowed(raw)
        };
        let count = Count::of(&text);
        let t = self.add(Data::Text(text, count));
        self.append(parent, t);
    }

    /// Closes what opening `name` closes, as the HTML parser does.
    fn imply_ends(&mut self, open: &mut Vec<Id>, name: &str) {
        if CLOSES_P.contains(&name) {
            self.close_in_scope(open, &["p"], &[]);
        }
        match name {
            "li" => self.close_in_scope(open, &["li"], &["ul", "ol"]),
            "dd" | "dt" => self.close_in_scope(open, &["dd", "dt"], &["dl"]),
            "td" | "th" => self.close_in_scope(open, &["td", "th"], &["tr"]),
            "tr" => self.close_in_scope(open, &["tr", "td", "th"], &["thead", "tbody", "tfoot"]),
            "thead" | "tbody" | "tfoot" => {
                self.close_in_scope(open, &["thead", "tbody", "tfoot", "tr", "td", "th"], &[])
            }
            "option" => self.close_in_scope(open, &["option"], &[]),
            "a" => self.close_in_scope(open, &["a"], &[]),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let top = *open.last().unwrap();
                if matches!(
                    self.name(top),
                    Some("h1" | "h2" | "h3" | "h4" | "h5" | "h6")
                ) {
                    open.pop();
                }
            }
            _ => {}
        }
    }

    /// Pops to and past the innermost open tag of `names`, looking no further than `stop` or a
    /// scope's edge.
    fn close_in_scope(&mut self, open: &mut Vec<Id>, names: &[&str], stop: &[&str]) {
        for at in (1..open.len()).rev() {
            let Some(n) = self.name(open[at]) else {
                continue;
            };
            if names.contains(&n) {
                open.truncate(at);
                return;
            }
            if stop.contains(&n) || SCOPE.contains(&n) {
                return;
            }
        }
    }

    fn end(&mut self, open: &mut Vec<Id>, name: &str) {
        if matches!(name, "html" | "head" | "body") {
            return;
        }
        for at in (1..open.len()).rev() {
            let n = self.name(open[at]);
            if n == Some(name) {
                open.truncate(at);
                return;
            }
            // A table's end closes its rows and cells; anything else ends at a scope's edge
            let crosses = |n: &str| match name {
                "table" => n != "table",
                "tr" | "thead" | "tbody" | "tfoot" => matches!(n, "td" | "th"),
                _ => false,
            };
            if n == Some("body") || n.is_some_and(|n| SCOPE.contains(&n) && !crosses(n)) {
                return;
            }
        }
    }

    pub fn add(&mut self, data: Data<'a>) -> Id {
        self.nodes.push(Node::new(data));
        self.nodes.len() - 1
    }

    pub fn new_element(&mut self, name: &'static str) -> Id {
        self.add(Data::Element(Cow::Borrowed(name), 0, 0))
    }

    pub fn detach(&mut self, id: Id) {
        let Node {
            parent, prev, next, ..
        } = self.nodes[id];
        let Some(parent) = link(parent) else { return };
        match link(prev) {
            Some(p) => self.nodes[p].next = next,
            None => self.nodes[parent].first = next,
        }
        match link(next) {
            Some(n) => self.nodes[n].prev = prev,
            None => self.nodes[parent].last = prev,
        }
        let node = &mut self.nodes[id];
        node.parent = NONE;
        node.prev = NONE;
        node.next = NONE;
    }

    pub fn append(&mut self, parent: Id, child: Id) {
        self.detach(child);
        let last = self.nodes[parent].last;
        self.nodes[child].parent = parent as u32;
        self.nodes[child].prev = last;
        match link(last) {
            Some(l) => self.nodes[l].next = child as u32,
            None => self.nodes[parent].first = child as u32,
        }
        self.nodes[parent].last = child as u32;
    }

    pub fn insert_before(&mut self, at: Id, new: Id) {
        self.detach(new);
        let Node { parent, prev, .. } = self.nodes[at];
        let Some(parent) = link(parent) else { return };
        self.nodes[new].parent = parent as u32;
        self.nodes[new].prev = prev;
        self.nodes[new].next = at as u32;
        self.nodes[at].prev = new as u32;
        match link(prev) {
            Some(p) => self.nodes[p].next = new as u32,
            None => self.nodes[parent].first = new as u32,
        }
    }

    pub fn replace_with(&mut self, old: Id, new: Id) {
        if old == new {
            return;
        }
        self.insert_before(old, new);
        self.detach(old);
    }

    pub fn reparent_children(&mut self, from: Id, to: Id) {
        while let Some(c) = self.first_child(from) {
            self.append(to, c);
        }
    }

    pub fn parent(&self, id: Id) -> Option<Id> {
        link(self.nodes[id].parent)
    }

    pub fn first_child(&self, id: Id) -> Option<Id> {
        link(self.nodes[id].first)
    }

    pub fn last_child(&self, id: Id) -> Option<Id> {
        link(self.nodes[id].last)
    }

    pub fn next_sibling(&self, id: Id) -> Option<Id> {
        link(self.nodes[id].next)
    }

    pub fn children(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        std::iter::successors(self.first_child(id), |&c| self.next_sibling(c))
    }

    pub fn element_children(&self, id: Id) -> Vec<Id> {
        self.children(id).filter(|&c| self.is_element(c)).collect()
    }

    pub fn first_element_child(&self, id: Id) -> Option<Id> {
        self.children(id).find(|&c| self.is_element(c))
    }

    pub fn next_element_sibling(&self, id: Id) -> Option<Id> {
        std::iter::successors(self.next_sibling(id), |&c| self.next_sibling(c))
            .find(|&c| self.is_element(c))
    }

    pub fn prev_element_sibling(&self, id: Id) -> Option<Id> {
        std::iter::successors(link(self.nodes[id].prev), |&c| link(self.nodes[c].prev))
            .find(|&c| self.is_element(c))
    }

    /// Every node under `id` in document order, `id` left out.
    pub fn descendants(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        std::iter::successors(self.first_child(id), move |&cur| {
            self.first_child(cur).or_else(|| {
                let mut at = cur;
                loop {
                    if at == id {
                        return None;
                    }
                    if let Some(n) = self.next_sibling(at) {
                        return Some(n);
                    }
                    at = self.parent(at)?;
                }
            })
        })
    }

    /// The elements under `id` named any of `names`, in document order.
    pub fn select(&self, id: Id, names: &[&str]) -> Vec<Id> {
        self.descendants(id)
            .filter(|&d| self.name(d).is_some_and(|n| names.contains(&n)))
            .collect()
    }

    pub fn ancestors(&self, id: Id) -> impl Iterator<Item = Id> + '_ {
        std::iter::successors(self.parent(id), |&p| self.parent(p))
    }

    pub fn is_element(&self, id: Id) -> bool {
        matches!(self.nodes[id].data, Data::Element(..))
    }

    pub fn text_of(&self, id: Id) -> Option<&str> {
        match &self.nodes[id].data {
            Data::Text(t, _) => Some(t),
            _ => None,
        }
    }

    pub fn name(&self, id: Id) -> Option<&str> {
        match &self.nodes[id].data {
            Data::Element(name, ..) => Some(name),
            _ => None,
        }
    }

    pub fn is(&self, id: Id, name: &str) -> bool {
        self.name(id) == Some(name)
    }

    pub fn attrs(&self, id: Id) -> &[Attr<'a>] {
        match self.nodes[id].data {
            Data::Element(_, from, len) => &self.attrs[from as usize..(from + len) as usize],
            _ => &[],
        }
    }

    pub fn attr(&self, id: Id, key: &str) -> Option<&str> {
        self.attrs(id)
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| &**v)
    }

    pub fn has_attr(&self, id: Id, key: &str) -> bool {
        self.attr(id, key).is_some()
    }

    /// Sets `key`; a new one moves the element's run to the pool's end unless it is there already.
    pub fn set_attr(&mut self, id: Id, key: &str, value: Cow<'a, str>) {
        let Data::Element(_, from, len) = self.nodes[id].data else {
            return;
        };
        let (from, end) = (from as usize, (from + len) as usize);
        if let Some(a) = self.attrs[from..end].iter_mut().find(|(k, _)| k == key) {
            a.1 = value;
            return;
        }
        let from = if end == self.attrs.len() {
            from
        } else {
            self.attrs.extend_from_within(from..end);
            self.attrs.len() - (end - from)
        };
        self.attrs.push((Cow::Owned(key.to_string()), value));
        if let Data::Element(_, f, l) = &mut self.nodes[id].data {
            *f = from as u32;
            *l = (self.attrs.len() - from) as u32;
        }
    }

    pub fn remove_attr(&mut self, id: Id, key: &str) {
        if let Data::Element(_, from, len) = &mut self.nodes[id].data {
            let run = &mut self.attrs[*from as usize..(*from + *len) as usize];
            if let Some(at) = run.iter().position(|(k, _)| k == key) {
                run[at..].rotate_left(1);
                *len -= 1;
            }
        }
    }

    pub fn clear_attrs(&mut self, id: Id) {
        if let Data::Element(_, _, len) = &mut self.nodes[id].data {
            *len = 0;
        }
    }

    pub fn rename(&mut self, id: Id, to: &'static str) {
        if let Data::Element(name, ..) = &mut self.nodes[id].data {
            *name = Cow::Borrowed(to);
        }
    }

    /// The words under `id`, as written.
    pub fn text(&self, id: Id) -> String {
        let mut out = String::new();
        for d in std::iter::once(id).chain(self.descendants(id)) {
            if let Some(t) = self.text_of(d) {
                out.push_str(t);
            }
        }
        out
    }

    /// How many characters the words under `id` come to with each run of spaces one space, and none
    /// at either end.
    pub fn char_count(&self, id: Id) -> usize {
        let mut count = 0;
        let mut space = true;
        for d in std::iter::once(id).chain(self.descendants(id)) {
            if let Data::Text(_, c) = self.nodes[d].data
                && c.chars > 0
            {
                // Whitespace opening the node joins a run the last one ended on
                count += c.chars as usize - (space && c.leads) as usize;
                space = c.trails;
            }
        }
        if space && count > 0 {
            count -= 1;
        }
        count
    }

    pub fn is_nonempty_text(&self, id: Id) -> bool {
        self.text_of(id).is_some_and(|t| !t.trim().is_empty())
    }
}

impl<'a> Node<'a> {
    fn new(data: Data<'a>) -> Self {
        Node {
            data,
            parent: NONE,
            first: NONE,
            last: NONE,
            prev: NONE,
            next: NONE,
            score: None,
        }
    }
}

fn lower(s: &str) -> Cow<'_, str> {
    if s.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(s.to_ascii_lowercase())
    } else {
        Cow::Borrowed(s)
    }
}

/// Where the words of a raw tag opened before `from` end, and where the input after its end tag
/// begins.
fn find_end(bytes: &[u8], from: usize, name: &str) -> (usize, usize) {
    let mut at = from;
    while let Some(n) = memchr::memchr(b'<', &bytes[at..]) {
        let lt = at + n;
        let tag = lt + 2;
        if bytes.get(lt + 1) == Some(&b'/')
            && bytes
                .get(tag..tag + name.len())
                .is_some_and(|t| t.eq_ignore_ascii_case(name.as_bytes()))
        {
            let gt = memchr::memchr(b'>', &bytes[tag..]).map_or(bytes.len(), |g| tag + g + 1);
            return (lt, gt);
        }
        at = lt + 1;
    }
    (bytes.len(), bytes.len())
}
