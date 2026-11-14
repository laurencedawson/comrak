//! 4chan, from its API's documentation: a thread's page is `/{board}/thread/{op}` on either of the
//! site's board hosts, the thread's title (its `semantic_url`) after it, a post `#p{no}` on it; a
//! board's index `/{board}/` and its catalog `/{board}/catalog`, a search after `#s=`, where a
//! `>>>/g/` quotelink goes.

use super::{Url, digits};
use crate::blob::html::{HtmlRules, Reads, TagRule};

/// Greentext in the site's green, a dead link struck through, a spoiler hidden; its dead links open
/// on `>>`, so a paragraph's marks are words.
pub(super) const HTML: HtmlRules<'static> = HtmlRules {
    quote_marks: false,
    tags: &[
        TagRule {
            tag: "span",
            class: Some("quote"),
            reads_as: Reads::Greentext,
        },
        TagRule {
            tag: "span",
            class: Some("deadlink"),
            reads_as: Reads::Strike,
        },
        TagRule {
            tag: "s",
            class: None,
            reads_as: Reads::Spoiler("Spoiler"),
        },
    ],
};

/// The site's board hosts, where its threads are.
pub(super) const HOSTS: &[&str] = &["boards.4chan.org", "boards.4channel.org"];

pub(super) fn is_own(url: &Url<'_>) -> bool {
    if !url.on(HOSTS) {
        return false;
    }
    let path = url.path.strip_suffix('/').unwrap_or(url.path);
    let mut split = path.split('/');
    let [board, page, op, title] = [0; 4].map(|_| split.next().unwrap_or(""));
    if split.next().is_some()
        || board.is_empty()
        || !board.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return false;
    }
    match (page, op, title) {
        // The board's index, and its catalog with whatever search after `#`
        ("" | "catalog", "", "") => true,
        // A thread, its title after it, and a post `#p{no}` on it
        ("thread", op, _) => {
            digits(op)
                && (url.fragment.is_empty() || url.fragment.strip_prefix('p').is_some_and(digits))
        }
        _ => false,
    }
}
