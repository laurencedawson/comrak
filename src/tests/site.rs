//! Tests for each site's own pages (`blob::site::is_own`), each site's shapes from its own API
//! documentation or routes: what must be the site's, and what must not, lookalikes among them.

use crate::blob::site::is_own;
use crate::blob::sites::{CHAN, HACKER_NEWS, LEMMY, LOBSTERS, PLAIN};

fn own(site: u8, urls: &[&str]) {
    for url in urls {
        assert!(is_own(site, url), "{url} should be the site's own");
    }
}

fn not_own(site: u8, urls: &[&str]) {
    for url in urls {
        assert!(!is_own(site, url), "{url} should not be the site's own");
    }
}

// ── the url ──────────────────────────────────────────────────────────────

/// An address with a user, a port or another scheme is no site's page, whatever follows.
#[test]
fn url_with_a_user_port_or_other_scheme_is_none() {
    not_own(HACKER_NEWS, &[
        "https://x@news.ycombinator.com/item?id=1",
        "https://news.ycombinator.com:8080/item?id=1",
        "ftp://news.ycombinator.com/item?id=1",
        "news.ycombinator.com/item?id=1",
        "//news.ycombinator.com/item?id=1",
        "",
    ]);
}

/// The plain site, and an id no site has, own nothing.
#[test]
fn plain_owns_nothing() {
    not_own(PLAIN, &["https://news.ycombinator.com/item?id=1", "https://lemmy.world/post/1"]);
    not_own(200, &["https://news.ycombinator.com/item?id=1"]);
}

// ── hacker news ──────────────────────────────────────────────────────────

/// An item of any kind by its id, and a user by their name, in either scheme and any host case.
#[test]
fn hacker_news_items_and_users() {
    own(HACKER_NEWS, &[
        "https://news.ycombinator.com/item?id=45678",
        "http://news.ycombinator.com/item?id=1",
        "https://NEWS.ycombinator.com/item?id=1",
        "https://news.ycombinator.com/item?id=45678#45690",
        "https://news.ycombinator.com/item?p=2&id=45678",
        "https://news.ycombinator.com/user?id=pg",
        "https://news.ycombinator.com/user?id=Pg_2",
    ]);
}

/// An item needs a numeric id, a user a name; other pages, and lookalike hosts, are not its own.
#[test]
fn hacker_news_not_its_own() {
    not_own(HACKER_NEWS, &[
        "https://news.ycombinator.com/item",
        "https://news.ycombinator.com/item?id=",
        "https://news.ycombinator.com/item?id=12a",
        "https://news.ycombinator.com/item?xid=12",
        "https://news.ycombinator.com/user?id=",
        "https://news.ycombinator.com/",
        "https://news.ycombinator.com/news",
        "https://news.ycombinator.com/from?site=example.com",
        "https://news.ycombinator.com/item/1",
        "https://ycombinator.com/item?id=1",
        "https://evilnews.ycombinator.com/item?id=1",
        "https://news.ycombinator.com.evil.com/item?id=1",
        "https://hacker-news.firebaseio.com/v0/item/1.json",
    ]);
}

// ── chan ─────────────────────────────────────────────────────────────────

/// A thread, its title after it, a post on it, on either board host.
#[test]
fn chan_threads_and_posts() {
    own(CHAN, &[
        "https://boards.4chan.org/g/thread/100",
        "https://boards.4chan.org/g/thread/100/",
        "https://boards.4chan.org/g/thread/100#p123",
        "https://boards.4chan.org/g/thread/100/some-title",
        "https://boards.4chan.org/g/thread/100/some-title#p123",
        "https://boards.4channel.org/po/thread/570368#p570370",
        "http://boards.4chan.org/3/thread/7",
        "https://BOARDS.4chan.org/g/thread/100",
    ]);
}

/// A board, its catalog, an archive, a malformed thread or post, or another host is not its own.
#[test]
fn chan_not_its_own() {
    not_own(CHAN, &[
        "https://boards.4chan.org/g/",
        "https://boards.4chan.org/g/catalog",
        "https://boards.4chan.org/po/catalog#s=lft",
        "https://boards.4chan.org/g/archive",
        "https://boards.4chan.org/g/thread/",
        "https://boards.4chan.org/g/thread/abc",
        "https://boards.4chan.org/g/thread/100#q123",
        "https://boards.4chan.org/g/thread/100#p",
        "https://boards.4chan.org/g/thread/100/title/extra",
        "https://boards.4chan.org//thread/100",
        "https://4chan.org/g/thread/100",
        "https://boards.4chan.org.evil.com/g/thread/100",
        "https://a.4cdn.org/g/thread/100.json",
        "https://i.4cdn.org/g/123.png",
    ]);
}

// ── lobsters ─────────────────────────────────────────────────────────────

/// Stories, comments, users and tags, and the forms that redirect to them.
#[test]
fn lobsters_stories_comments_users_tags() {
    own(LOBSTERS, &[
        "https://lobste.rs/s/cdnepk",
        "https://lobste.rs/s/cdnepk/",
        "https://lobste.rs/s/cdnepk/passkeys_attestation_trap",
        "https://lobste.rs/s/cdnepk/passkeys_attestation_trap#c_gdrx9l",
        "https://lobste.rs/s/cdnepk/passkeys_attestation_trap/comments/gdrx9l",
        "https://lobste.rs/stories/cdnepk",
        "https://lobste.rs/c/523dmc",
        "https://lobste.rs/comments/523dmc",
        "https://lobste.rs/~pushcx",
        "https://lobste.rs/u/pushcx",
        "https://lobste.rs/t/rust",
        "https://lobste.rs/t/rust,go",
        "https://lobste.rs/t/rust/page/2",
        "http://LOBSTE.RS/s/cdnepk",
    ]);
}

/// Other pages, malformed ones, and other hosts are not its own.
#[test]
fn lobsters_not_its_own() {
    not_own(LOBSTERS, &[
        "https://lobste.rs/",
        "https://lobste.rs/newest",
        "https://lobste.rs/s/",
        "https://lobste.rs/c/",
        "https://lobste.rs/~",
        "https://lobste.rs/t/",
        "https://lobste.rs/t/rust/page/two",
        "https://lobste.rs/s/cdnepk/title/extra",
        "https://lobste.rs/s/cdnepk/title/comments/",
        "https://lobste.rs/s/cd-nepk",
        "https://lobste.rs/hottest.json",
        "https://www.lobste.rs/s/cdnepk",
        "https://lobste.rs.evil.com/s/cdnepk",
        "https://lemmy.world/c/rust",
    ]);
}

// ── lemmy ────────────────────────────────────────────────────────────────

/// Posts, comments, communities, multi-communities and users, on any instance, with or without
/// another instance's name.
#[test]
fn lemmy_any_instance() {
    own(LEMMY, &[
        "https://lemmy.world/post/123",
        "https://lemmy.world/post/123/456",
        "https://feddit.org/comment/789",
        "https://lemmy.world/c/rust",
        "https://lemmy.world/c/rust@programming.dev",
        "https://lemmy.world/m/tech",
        "https://feddit.org/u/Kris",
        "https://lemmy.world/u/some_one@lemmy.ml",
        "https://lemmy.world/post/123/",
        "http://beehaw.org/c/technology",
    ]);
}

/// The same shapes on every instance: Lemmy is federated, so the host is never what makes a link
/// its own.
#[test]
fn lemmy_on_every_instance() {
    for host in [
        "lemmy.ml",
        "lemmy.world",
        "sh.itjust.works",
        "programming.dev",
        "beehaw.org",
        "feddit.org",
        "lemm.ee",
        "sopuli.xyz",
        "lemmy.dbzer0.com",
        "discuss.tchncs.de",
        "infosec.pub",
        "lemmy.blahaj.zone",
    ] {
        own(LEMMY, &[
            &format!("https://{host}/post/1"),
            &format!("https://{host}/post/1/2"),
            &format!("https://{host}/comment/3"),
            &format!("https://{host}/c/rust"),
            &format!("https://{host}/c/rust@lemmy.ml"),
            &format!("https://{host}/m/tech"),
            &format!("https://{host}/u/someone"),
            &format!("https://{host}/u/someone@lemmy.world"),
        ]);
        not_own(LEMMY, &[&format!("https://{host}/c/rust/settings"), &format!("https://{host}/post/x")]);
    }
}

/// A page that is not content, a malformed id, another path, or reddit, whose users share the
/// `/u/` shape, is not its own.
#[test]
fn lemmy_not_its_own() {
    not_own(LEMMY, &[
        "https://lemmy.world/c/rust/settings",
        "https://lemmy.world/m/tech/settings",
        "https://lemmy.world/post/abc",
        "https://lemmy.world/post/",
        "https://lemmy.world/post/1/2/3",
        "https://lemmy.world/comment/x",
        "https://lemmy.world/c/",
        "https://lemmy.world/create_post",
        "https://lemmy.world/communities",
        "https://lemmy.world/",
        "https://reddit.com/u/spez",
        "https://www.reddit.com/u/spez",
        "https://old.REDDIT.com/c/x",
        "https://example.com/blog/post",
    ]);
}
