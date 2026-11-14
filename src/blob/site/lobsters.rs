//! Lobsters, from its `config/routes.rb`: a story `/s/{id}`, its title after it, and a comment on it
//! `/s/{id}/{title}/comments/{id}`; a comment `/c/{id}`; a user `/~{name}`; a tag or tags `/t/{tags}`
//! and their pages. `/stories/{id}`, `/comments/{id}` and `/u/{name}` redirect to them, so are its
//! own too. Only on `lobste.rs`.

use super::{Url, digits};

/// The site's one host.
pub(super) const HOSTS: &[&str] = &["lobste.rs"];

pub(super) fn is_own(url: &Url<'_>) -> bool {
    if !url.on(HOSTS) {
        return false;
    }
    let path = url.path.strip_suffix('/').unwrap_or(url.path);
    let mut split = path.split('/');
    let segments: [&str; 5] = [0; 5].map(|_| split.next().unwrap_or(""));
    if split.next().is_some() {
        return false;
    }
    let id = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric());
    match segments {
        // A story, its title, and a comment on it
        ["s", story, _, "", ""] => id(story),
        ["s", story, _, "comments", comment] => id(story) && id(comment),
        ["stories", story, "", "", ""] => id(story),
        ["c", comment, "", "", ""] | ["comments", comment, "", "", ""] => id(comment),
        ["u", name, "", "", ""] => !name.is_empty(),
        [user, "", "", "", ""] => user.strip_prefix('~').is_some_and(|name| !name.is_empty()),
        ["t", tags, "", "", ""] => !tags.is_empty(),
        ["t", tags, "page", page, ""] => !tags.is_empty() && digits(page),
        _ => false,
    }
}
