//! What each site syncdown reads knows about itself, one function per question, each matching on
//! the site's id from [`super::sites`]: a site that answers differently has an arm, and every
//! other site, [`sites::PLAIN`] among them, takes the default. Each site's own code is in its module.
//!
//! Nothing here copies or allocates: a url is read as borrowed slices of itself.

mod chan;
mod hacker_news;
mod lemmy;
mod lobsters;

use super::sites;

/// Whether `url`, already resolved, is one of `site`'s own pages, for the app to open itself.
pub(crate) fn is_own(site: u8, url: &str) -> bool {
    let Some(url) = Url::parse(url) else {
        return false;
    };
    match site {
        sites::HACKER_NEWS => hacker_news::is_own(&url),
        sites::CHAN => chan::is_own(&url),
        sites::LEMMY => lemmy::is_own(&url),
        sites::LOBSTERS => lobsters::is_own(&url),
        _ => false,
    }
}

/// An `http` or `https` address as borrowed slices of itself: its host, its path without the
/// leading slash, its query and its fragment, each empty where it has none. An address with a
/// user, a port or any other scheme is none of a site's pages, so it reads as nothing.
pub(crate) struct Url<'a> {
    pub(crate) host: &'a str,
    pub(crate) path: &'a str,
    pub(crate) query: &'a str,
    pub(crate) fragment: &'a str,
}

impl<'a> Url<'a> {
    pub(crate) fn parse(url: &'a str) -> Option<Self> {
        let (scheme, rest) = url.split_once("://")?;
        if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
            return None;
        }
        let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
        let (rest, query) = rest.split_once('?').unwrap_or((rest, ""));
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if host.is_empty() || host.contains(['@', ':']) {
            return None;
        }
        Some(Url { host, path, query, fragment })
    }

    /// Whether the host is `name`, in any case.
    pub(crate) fn host_is(&self, name: &str) -> bool {
        self.host.eq_ignore_ascii_case(name)
    }

    /// The query's value for `key`, the first where it is given twice.
    pub(crate) fn param(&self, key: &str) -> Option<&'a str> {
        self.query.split('&').find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
    }
}

/// Every byte an ASCII digit, and at least one.
pub(crate) fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
