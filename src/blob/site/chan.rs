//! 4chan, from its API's documentation: a thread's page is `/{board}/thread/{op}` on either of the
//! site's board hosts, the thread's title (its `semantic_url`) after it, a post `#p{no}` on it.
//! The catalog and a board's own pages hold no one post, so are not the app's.

use super::{Url, digits};
use crate::blob::html::{HtmlRules, Reads, TagRule};

/// Greentext in the site's green, a dead link struck through, a spoiler hidden; its dead links open
/// on `>>`, so a paragraph's marks are words.
pub(super) const HTML: HtmlRules<'static> = HtmlRules {
    quote_marks: false,
    tags: &[
        TagRule { tag: "span", class: Some("quote"), reads_as: Reads::Greentext },
        TagRule { tag: "span", class: Some("deadlink"), reads_as: Reads::Strike },
        TagRule { tag: "s", class: None, reads_as: Reads::Spoiler("Spoiler") },
    ],
};

/// The site's board hosts, where its threads are.
pub(super) const HOSTS: &[&str] = &["boards.4chan.org", "boards.4channel.org"];

pub(super) fn is_own(url: &Url<'_>) -> bool {
    if !url.on(HOSTS) {
        return false;
    }
    let mut path = url.path.split('/');
    let board = path.next().unwrap_or("");
    let thread = path.next() == Some("thread") && path.next().is_some_and(digits);
    // The title, if any, and then nothing more but a trailing slash
    let _title = path.next();
    let ends = path.next().is_none_or(str::is_empty) && path.next().is_none();
    let post = url.fragment.is_empty() || url.fragment.strip_prefix('p').is_some_and(digits);
    !board.is_empty() && board.bytes().all(|b| b.is_ascii_alphanumeric()) && thread && ends && post
}
