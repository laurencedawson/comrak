//! What each site syncdown reads knows about itself, one function per question, each matching on
//! the site's id from [`super::sites`]: a site that answers differently has an arm, and every
//! other site, [`sites::PLAIN`] among them, takes the default. Each site's own code is in its module.
//!
//! Nothing here copies or allocates: a url is read as borrowed slices of itself.

mod chan;
mod hacker_news;
mod lemmy;
mod lobsters;

use super::html::HtmlRules;
use super::sites;

/// How `site`'s HTML reads; a site that sends none reads as plain HTML.
pub(crate) fn html(site: u8) -> &'static HtmlRules<'static> {
    match site {
        sites::HACKER_NEWS => &hacker_news::HTML,
        sites::CHAN => &chan::HTML,
        _ => &PLAIN,
    }
}

/// Whether `site`'s links are drawn larger than their words, a target for a finger in a comment.
/// Not in an article, where a link is a word in a sentence.
pub(crate) fn link_size(site: u8) -> bool {
    !matches!(site, sites::READER)
}

/// Whether a link to another site has that site's domain after its words, where they lack it, to
/// say where it goes. Not in an article, where every linked word in a sentence would carry one.
pub(crate) fn link_domain(site: u8) -> bool {
    !matches!(site, sites::READER)
}

/// Plain HTML: no site's tags, and a paragraph opening on `>` is words.
const PLAIN: HtmlRules<'static> = HtmlRules {
    quote_marks: false,
    tags: &[],
};

/// What a resolved address is to a site, read once for both questions a link asks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    /// One of the site's own pages, for the app to open itself.
    Own,
    /// On the site's own host, where the domain after a link's words tells its reader nothing.
    Internal,
    /// Any other address.
    Elsewhere,
}

/// What `url`, already resolved, is to `site`. A relative address is on the site's own host,
/// whatever the site; Lemmy, federated, has no host of its own beyond that, and `PLAIN` no pages.
pub fn page(site: u8, url: &str) -> Page {
    let Some(url) = Url::parse(url) else {
        return Page::Elsewhere;
    };
    let (own, hosts): (bool, &[&str]) = match site {
        sites::HACKER_NEWS => (hacker_news::is_own(&url), hacker_news::HOSTS),
        sites::CHAN => (chan::is_own(&url), chan::HOSTS),
        sites::LEMMY => (lemmy::is_own(&url), &[]),
        sites::LOBSTERS => (lobsters::is_own(&url), lobsters::HOSTS),
        _ => (false, &[]),
    };
    if own {
        Page::Own
    } else if url.on(hosts) {
        Page::Internal
    } else {
        Page::Elsewhere
    }
}

/// An address as borrowed slices of itself: its host, its path without the leading slash, its
/// query and its fragment, each empty where it has none. An `http` or `https` address has its
/// host; a relative one, opening on `/`, is on the page it sits on, so has none. An address with
/// a user, a port, any other scheme, or a host without a scheme (`//host`) is none of a site's
/// pages, so it reads as nothing.
pub(crate) struct Url<'a> {
    pub(crate) host: &'a str,
    pub(crate) path: &'a str,
    pub(crate) query: &'a str,
    pub(crate) fragment: &'a str,
}

impl<'a> Url<'a> {
    pub(crate) fn parse(url: &'a str) -> Option<Self> {
        let (host, rest) = if let Some(rest) = url.strip_prefix('/') {
            if rest.starts_with('/') {
                return None;
            }
            ("", rest)
        } else {
            let (scheme, rest) = url.split_once("://")?;
            if !scheme.eq_ignore_ascii_case("https") && !scheme.eq_ignore_ascii_case("http") {
                return None;
            }
            let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
            let (host, rest) = rest.split_at(end);
            if host.is_empty() || host.contains(['@', ':']) {
                return None;
            }
            (host, rest.strip_prefix('/').unwrap_or(rest))
        };
        let (rest, fragment) = rest.split_once('#').unwrap_or((rest, ""));
        let (path, query) = rest.split_once('?').unwrap_or((rest, ""));
        Some(Url {
            host,
            path,
            query,
            fragment,
        })
    }

    /// Whether the address is on one of `hosts`, in any case, or relative, so on the page's own.
    pub(crate) fn on(&self, hosts: &[&str]) -> bool {
        self.host.is_empty()
            || hosts
                .iter()
                .any(|host| self.host.eq_ignore_ascii_case(host))
    }

    /// The query's value for `key`, the first where it is given twice.
    pub(crate) fn param(&self, key: &str) -> Option<&'a str> {
        self.query
            .split('&')
            .find_map(|pair| pair.strip_prefix(key)?.strip_prefix('='))
    }
}

/// Every byte an ASCII digit, and at least one.
pub(crate) fn digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}
