//! Hacker News, from its API's documentation: every item (story, comment, job, poll) is one id,
//! its page `news.ycombinator.com/item?id=`, and a user's page `user?id=` by their name.

use super::{Url, digits};
use crate::blob::html::HtmlRules;

/// A paragraph opening on `>` is a quote, as the site's posters write one; it has no tags of its own.
pub(super) const HTML: HtmlRules<'static> = HtmlRules { quote_marks: true, tags: &[] };

/// The site's one host.
pub(super) const HOSTS: &[&str] = &["news.ycombinator.com"];

pub(super) fn is_own(url: &Url<'_>) -> bool {
    if !url.on(HOSTS) {
        return false;
    }
    match (url.path, url.param("id")) {
        ("item", Some(id)) => digits(id),
        ("user", Some(name)) => !name.is_empty(),
        _ => false,
    }
}
