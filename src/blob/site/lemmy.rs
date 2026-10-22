//! Lemmy, from lemmy-ui's routes and the addresses its server gives what it federates: a post
//! `/post/{id}` (a comment on it `/post/{id}/{comment}`), a comment `/comment/{id}`, a community
//! `/c/{name}`, a multi-community `/m/{name}`, a user `/u/{name}`, a name with `@{instance}` where
//! it is another's. Federated, so on any host but reddit.com, whose users share the `/u/` shape.

use super::{Url, digits};

pub(super) fn is_own(url: &Url<'_>) -> bool {
    // Bytes, so a host's end falling inside a character is no panic
    let host = url.host.as_bytes();
    let reddit = host.eq_ignore_ascii_case(b"reddit.com")
        || host.len() > 11 && host[host.len() - 11..].eq_ignore_ascii_case(b".reddit.com");
    if reddit {
        return false;
    }
    let path = url.path.strip_suffix('/').unwrap_or(url.path);
    let mut segments = path.split('/');
    let (kind, first, second, more) = (segments.next(), segments.next(), segments.next(), segments.next());
    if more.is_some() {
        return false;
    }
    let name = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'@' | b'-'));
    match (kind, first, second) {
        (Some("post"), Some(post), None) => digits(post),
        (Some("post"), Some(post), Some(comment)) => digits(post) && digits(comment),
        (Some("comment"), Some(comment), None) => digits(comment),
        (Some("c" | "m" | "u"), Some(n), None) => name(n),
        _ => false,
    }
}
