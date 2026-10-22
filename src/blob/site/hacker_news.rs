//! Hacker News, from its API's documentation: every item (story, comment, job, poll) is one id,
//! its page `news.ycombinator.com/item?id=`, and a user's page `user?id=` by their name.

use super::{Url, digits};

pub(super) fn is_own(url: &Url<'_>) -> bool {
    if !url.host_is("news.ycombinator.com") {
        return false;
    }
    match (url.path, url.param("id")) {
        ("item", Some(id)) => digits(id),
        ("user", Some(name)) => !name.is_empty(),
        _ => false,
    }
}
