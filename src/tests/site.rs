//! Tests for each site's own pages (`blob::site::page`), each site's shapes from its own API
//! documentation or routes: what must be the site's, and what must not, lookalikes among them;
//! and for its own host, after whose links no domain is written.

use crate::benchmarks::production_options;
use crate::blob::site::{Page, page};
use crate::blob::sites::{CHAN, HACKER_NEWS, LEMMY, LOBSTERS, PLAIN, READER};
use crate::blob::{APP_LINK, LINK, LINK_SIZE, render_blob};
use crate::parse_document_zerocopy;
use crate::tests::blob_read::read;

fn own(site: u8, urls: &[&str]) {
    for url in urls {
        assert_eq!(page(site, url), Page::Own, "{url} should be the site's own");
    }
}

fn not_own(site: u8, urls: &[&str]) {
    for url in urls {
        assert_ne!(
            page(site, url),
            Page::Own,
            "{url} should not be the site's own"
        );
    }
}

// ── the url ──────────────────────────────────────────────────────────────

/// An address with a user, a port or another scheme is no site's page, whatever follows.
#[test]
fn url_with_a_user_port_or_other_scheme_is_none() {
    not_own(
        HACKER_NEWS,
        &[
            "https://x@news.ycombinator.com/item?id=1",
            "https://news.ycombinator.com:8080/item?id=1",
            "ftp://news.ycombinator.com/item?id=1",
            "news.ycombinator.com/item?id=1",
            "//news.ycombinator.com/item?id=1",
            "",
        ],
    );
}

/// The plain site, and an id no site has, own nothing.
#[test]
fn plain_owns_nothing() {
    not_own(
        PLAIN,
        &[
            "https://news.ycombinator.com/item?id=1",
            "https://lemmy.world/post/1",
        ],
    );
    not_own(200, &["https://news.ycombinator.com/item?id=1"]);
}

// ── hacker news ──────────────────────────────────────────────────────────

/// An item of any kind by its id, and a user by their name, in either scheme and any host case.
#[test]
fn hacker_news_items_and_users() {
    own(
        HACKER_NEWS,
        &[
            "https://news.ycombinator.com/item?id=45678",
            "http://news.ycombinator.com/item?id=1",
            "https://NEWS.ycombinator.com/item?id=1",
            "https://news.ycombinator.com/item?id=45678#45690",
            "https://news.ycombinator.com/item?p=2&id=45678",
            "https://news.ycombinator.com/user?id=pg",
            "https://news.ycombinator.com/user?id=Pg_2",
        ],
    );
}

/// An item needs a numeric id, a user a name; other pages, and lookalike hosts, are not its own.
#[test]
fn hacker_news_not_its_own() {
    not_own(
        HACKER_NEWS,
        &[
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
        ],
    );
}

// ── chan ─────────────────────────────────────────────────────────────────

/// A thread, its title after it, a post on it, on either board host.
#[test]
fn chan_threads_and_posts() {
    own(
        CHAN,
        &[
            "https://boards.4chan.org/g/thread/100",
            "https://boards.4chan.org/g/thread/100/",
            "https://boards.4chan.org/g/thread/100#p123",
            "https://boards.4chan.org/g/thread/100/some-title",
            "https://boards.4chan.org/g/thread/100/some-title#p123",
            "https://boards.4channel.org/po/thread/570368#p570370",
            "http://boards.4chan.org/3/thread/7",
            "https://BOARDS.4chan.org/g/thread/100",
        ],
    );
}

/// A board's index, with or without its slash, and its catalog, a search after `#`, on either host.
#[test]
fn chan_boards_and_catalogs() {
    own(
        CHAN,
        &[
            "https://boards.4chan.org/g/",
            "https://boards.4chan.org/g",
            "https://boards.4channel.org/po/",
            "https://boards.4chan.org/g/catalog",
            "https://boards.4chan.org/g/catalog/",
            "https://boards.4chan.org/po/catalog#s=lft",
        ],
    );
}

/// A board's archive or a page of its index, a malformed thread or post, the site with no board, or
/// another host is not its own.
#[test]
fn chan_not_its_own() {
    not_own(
        CHAN,
        &[
            "https://boards.4chan.org/",
            "https://boards.4chan.org/g/archive",
            "https://boards.4chan.org/g/2",
            "https://boards.4chan.org/g/catalog/extra",
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
        ],
    );
}

// ── lobsters ─────────────────────────────────────────────────────────────

/// Stories, comments, users and tags, and the forms that redirect to them.
#[test]
fn lobsters_stories_comments_users_tags() {
    own(
        LOBSTERS,
        &[
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
        ],
    );
}

/// Other pages, malformed ones, and other hosts are not its own.
#[test]
fn lobsters_not_its_own() {
    not_own(
        LOBSTERS,
        &[
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
        ],
    );
}

// ── lemmy ────────────────────────────────────────────────────────────────

/// Posts, comments, communities, multi-communities and users, on any instance, with or without
/// another instance's name.
#[test]
fn lemmy_any_instance() {
    own(
        LEMMY,
        &[
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
        ],
    );
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
        own(
            LEMMY,
            &[
                &format!("https://{host}/post/1"),
                &format!("https://{host}/post/1/2"),
                &format!("https://{host}/comment/3"),
                &format!("https://{host}/c/rust"),
                &format!("https://{host}/c/rust@lemmy.ml"),
                &format!("https://{host}/m/tech"),
                &format!("https://{host}/u/someone"),
                &format!("https://{host}/u/someone@lemmy.world"),
            ],
        );
        not_own(
            LEMMY,
            &[
                &format!("https://{host}/c/rust/settings"),
                &format!("https://{host}/post/x"),
            ],
        );
    }
}

/// A page that is not content, a malformed id, another path, or reddit, whose users share the
/// `/u/` shape, is not its own.
#[test]
fn lemmy_not_its_own() {
    not_own(
        LEMMY,
        &[
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
        ],
    );
}

/// A host typed in another script is read whole: its end, checked for reddit's, may fall inside
/// a character, and Lemmy's shapes still hold on it.
#[test]
fn lemmy_non_ascii_host() {
    own(
        LEMMY,
        &[
            "https://ééééééé.com/c/rust",
            "https://ééééééé.reddit.com.example/c/rust",
        ],
    );
    not_own(LEMMY, &["https://éé.reddit.com/u/spez"]);
}

/// Hosts whose own pages share Lemmy's shapes are not its own, under any subdomain and in any
/// case; a host that only ends in one's letters is an instance like any other.
#[test]
fn lemmy_not_on_hosts_sharing_its_shapes() {
    not_own(
        LEMMY,
        &[
            "https://www.youtube.com/c/veritasium",
            "https://m.youtube.com/c/veritasium",
            "https://YouTube.com/c/veritasium",
            "https://www.patreon.com/c/somecreator",
            "https://rumble.com/c/SomeChannel",
            "https://staff.tumblr.com/post/123456",
            "https://medium.com/m/signin",
            "https://lobste.rs/c/523dmc",
        ],
    );
    own(
        LEMMY,
        &[
            "https://myyoutube.com/c/rust",
            "https://youtube.com.example/c/rust",
        ],
    );
}

/// A relative address is on the site's own host, so the site's shapes are its own there too, as
/// lemmy-ui writes a completed community or user; `//host` names another host, and a path without
/// its slash is no address.
#[test]
fn relative_address_is_on_the_sites_own_host() {
    own(
        LEMMY,
        &[
            "/post/1",
            "/post/1/2",
            "/comment/3",
            "/c/rust@lemmy.ml",
            "/u/me@lemmy.ml",
            "/c/rust/",
        ],
    );
    own(HACKER_NEWS, &["/item?id=1", "/user?id=pg"]);
    own(CHAN, &["/g/thread/100#p5"]);
    own(LOBSTERS, &["/s/cdnepk", "/~pushcx"]);
    not_own(
        LEMMY,
        &[
            "/",
            "/search",
            "/communities",
            "//lemmy.world/post/1",
            "post/1",
        ],
    );
    not_own(PLAIN, &["/post/1"]);
}

// ── internal ─────────────────────────────────────────────────────────────

/// A link on the site's own host, in any case, or relative, is internal: the domain after its
/// words would tell the site's reader nothing. Another host, a lookalike among them, is not, and
/// Lemmy, federated, has no host of its own.
#[test]
fn internal_is_the_sites_own_host_or_relative() {
    for (site, url) in [
        (HACKER_NEWS, "https://news.ycombinator.com/newest"),
        (HACKER_NEWS, "https://NEWS.ycombinator.com/from?site=x"),
        (HACKER_NEWS, "/newest"),
        (CHAN, "https://boards.4chan.org/g/"),
        (CHAN, "https://boards.4channel.org/po/catalog#s=lft"),
        (LOBSTERS, "https://lobste.rs/newest"),
        (LEMMY, "/communities"),
    ] {
        assert_ne!(
            page(site, url),
            Page::Elsewhere,
            "{url} should be internal to site {site}"
        );
    }
    for (site, url) in [
        (HACKER_NEWS, "https://ycombinator.com/"),
        (HACKER_NEWS, "https://news.ycombinator.com.evil.com/"),
        (HACKER_NEWS, "//news.ycombinator.com/newest"),
        (CHAN, "https://4chan.org/"),
        (CHAN, "https://i.4cdn.org/g/1.png"),
        (LOBSTERS, "https://www.lobste.rs/"),
        (LEMMY, "https://lemmy.world/communities"),
        (PLAIN, "https://news.ycombinator.com/newest"),
    ] {
        assert_eq!(
            page(site, url),
            Page::Elsewhere,
            "{url} should not be internal to site {site}"
        );
    }
    // Relative under any site, the plain one too: on the page's own host, so no domain after it
    assert_eq!(page(PLAIN, "/redirect?u=https://x.com/a"), Page::Internal);
}

// ── the markdown walk ────────────────────────────────────────────────────

/// Every link in `md` read as `site`, its kind and address, and the text shown.
fn links(md: &str, site: u8) -> (String, Vec<(i32, String)>) {
    let opts = production_options();
    let b = read(
        &parse_document_zerocopy(md, &opts, |root| render_blob(site, root, md)).expect("a blob"),
    );
    let found = b
        .span_iter()
        .filter(|s| s.typ == LINK || s.typ == APP_LINK)
        .map(|s| (s.typ, s.url.clone().unwrap()))
        .collect();
    (b.text, found)
}

/// A Lemmy post, comment, community or user linked from any instance is the app's; a link
/// elsewhere is a link, its domain after its words, the app's none.
#[test]
fn markdown_lemmy_links() {
    let (text, found) = links(
        "[a post](https://lemmy.ml/post/1) [a comment](https://sh.itjust.works/comment/2) \
         [rust](https://programming.dev/c/rust) [elsewhere](https://example.com/a)",
        LEMMY,
    );
    assert_eq!(text, "a post a comment rust elsewhere (example.com)");
    assert_eq!(
        found,
        vec![
            (APP_LINK, "https://lemmy.ml/post/1".to_string()),
            (APP_LINK, "https://sh.itjust.works/comment/2".to_string()),
            (APP_LINK, "https://programming.dev/c/rust".to_string()),
            (LINK, "https://example.com/a".to_string()),
        ]
    );
}

/// A Lemmy mention, a link comrak makes of `@user@instance` and `!community@instance`, is the app's.
#[test]
fn markdown_lemmy_mentions() {
    let (_, found) = links("hi @someone@lemmy.ml, see !rust@programming.dev", LEMMY);
    assert_eq!(found.len(), 2);
    assert!(found.iter().all(|(t, _)| *t == APP_LINK), "{found:?}");
}

/// A bare address is autolinked and marked the same, reddit's `/u/` left a link.
#[test]
fn markdown_lemmy_autolinks_and_reddit() {
    let (_, found) = links(
        "https://lemmy.world/post/9 and https://www.reddit.com/u/spez",
        LEMMY,
    );
    assert_eq!(
        found,
        vec![
            (APP_LINK, "https://lemmy.world/post/9".to_string()),
            (LINK, "https://www.reddit.com/u/spez".to_string()),
        ]
    );
}

/// Lobsters' stories and comments are the app's. Read as Lemmy, neither is: a comment's `/c/{id}`
/// shares a community's shape, so `lobste.rs` is among the hosts Lemmy leaves out.
#[test]
fn markdown_lobsters_links() {
    let md = "[story](https://lobste.rs/s/cdnepk/passkeys) and https://lobste.rs/c/523dmc";
    let (_, found) = links(md, LOBSTERS);
    assert!(found.iter().all(|(t, _)| *t == APP_LINK), "{found:?}");
    let (_, found) = links(md, LEMMY);
    assert!(found.iter().all(|(t, _)| *t == LINK), "{found:?}");
}

/// lemmy-ui writes a community or user it completes as a link to its relative page: the app's,
/// its address kept as written, and no domain after its words.
#[test]
fn markdown_lemmy_relative_links() {
    let (text, found) = links(
        "[!rust@lemmy.ml](/c/rust@lemmy.ml) [@me@lemmy.ml](/u/me@lemmy.ml) [p](/post/123) [s](/search)",
        LEMMY,
    );
    assert_eq!(text, "!rust@lemmy.ml @me@lemmy.ml p s");
    assert_eq!(
        found,
        vec![
            (APP_LINK, "/c/rust@lemmy.ml".to_string()),
            (APP_LINK, "/u/me@lemmy.ml".to_string()),
            (APP_LINK, "/post/123".to_string()),
            (LINK, "/search".to_string()),
        ]
    );
}

/// A link on the site's own host shows no domain after its words, the site's reader knowing where
/// they are; read as another site, the same link does.
#[test]
fn markdown_internal_links_show_no_domain() {
    let md = "[newest](https://lobste.rs/newest) [story](https://lobste.rs/s/cdnepk) [else](https://example.com/a)";
    let (text, found) = links(md, LOBSTERS);
    assert_eq!(text, "newest story else (example.com)");
    assert_eq!(
        found.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
        vec![LINK, APP_LINK, LINK]
    );
    let (text, _) = links(md, PLAIN);
    assert_eq!(
        text,
        "newest (lobste.rs) story (lobste.rs) else (example.com)"
    );
}

/// The reader's links are words in a sentence: no domain after them and no larger size, as a
/// comment's have.
#[test]
fn markdown_reader_links_are_their_words_alone() {
    let md = "see [the book](https://rust-lang.org/learn) now";
    let opts = production_options();
    let blob = |site| {
        read(
            &parse_document_zerocopy(md, &opts, |root| render_blob(site, root, md))
                .expect("a blob"),
        )
    };
    let reader = blob(READER);
    assert_eq!(reader.text, "see the book now");
    assert!(!reader.span_iter().any(|s| s.typ == LINK_SIZE));
    let plain = blob(PLAIN);
    assert_eq!(plain.text, "see the book (rust-lang.org) now");
    assert!(plain.span_iter().any(|s| s.typ == LINK_SIZE));
}

/// `md` read as `site`: its blob, where it answers one.
fn blob(md: &str, site: u8) -> Option<crate::tests::blob_read::Blob> {
    let opts = production_options();
    parse_document_zerocopy(md, &opts, |root| render_blob(site, root, md)).map(|b| read(&b))
}

/// Every site answers a blob for plain words, the view reading every body from one: the words as
/// they are, with no span but the reader's tile over them.
#[test]
fn every_site_answers_a_blob_for_plain_words() {
    for site in [PLAIN, HACKER_NEWS, CHAN, LEMMY, LOBSTERS, READER] {
        let b = blob("just some words", site).expect("a blob");
        assert_eq!(b.text, "just some words", "{site}");
        assert_eq!(b.spans.is_empty(), site != READER, "{site}: {:?}", b.spans);
    }
}

/// The reader's blob carries its line heights as a span over each run of lines, its words alone as
/// every block's span: from the text's start to its end, only newlines between one and the next, so
/// no span holds a blank line between blocks and a long body may be cut there; a one-run body
/// carries one. No other site writes any.
#[test]
fn reader_blob_tiles_each_run_of_lines() {
    for (md, blocks) in [
        ("some words", 1),
        ("> some quoted words", 1),
        ("# A title", 1),
        ("- an item\n- another", 2),
        ("one\n\ntwo\n\n# three\n\n> four", 4),
        ("```\ncode\nmore code\n```", 1),
    ] {
        let b = blob(md, READER).unwrap();
        let len = b.text.encode_utf16().count() as i32;
        let tiles: Vec<(i32, i32)> = b
            .span_iter()
            .filter(|s| s.typ == crate::blob::READER_BLOCK)
            .map(|s| (s.start, s.end))
            .collect();
        assert_eq!(tiles.len(), blocks, "{md:?}: {tiles:?}");
        assert_eq!(tiles[0].0, 0, "{md:?}");
        assert_eq!(tiles[tiles.len() - 1].1, len, "{md:?}");
        let utf16: Vec<u16> = b.text.encode_utf16().collect();
        for pair in tiles.windows(2) {
            let between = &utf16[pair[0].1 as usize..pair[1].0 as usize];
            assert!(
                !between.is_empty() && between.iter().all(|&c| c == b'\n' as u16),
                "{md:?}: {tiles:?}"
            );
        }
    }
    for site in [PLAIN, HACKER_NEWS, CHAN, LEMMY, LOBSTERS] {
        let b = blob("> some **quoted** words", site).unwrap();
        assert!(
            !b.span_iter().any(|s| s.typ == crate::blob::READER_BLOCK),
            "site {site}"
        );
    }
}

/// A blob says the site it was read for, in bits 2-7 of its flags byte, beside the flags it had.
#[test]
fn a_blob_stores_its_site_in_its_flags() {
    for site in [PLAIN, HACKER_NEWS, CHAN, LEMMY, LOBSTERS, READER] {
        let b = blob("some **bold** words", site).unwrap();
        assert_eq!(b.flags >> 2, site, "site {site}");
        assert!(b.ascii(), "site {site}");
    }
}

/// The reader owns no pages, as the plain site does not.
#[test]
fn reader_owns_nothing() {
    assert_eq!(
        page(READER, "https://news.ycombinator.com/item?id=1"),
        Page::Elsewhere
    );
    assert_eq!(page(READER, "https://lobste.rs/s/cdnepk"), Page::Elsewhere);
}

/// The plain site marks nothing.
#[test]
fn markdown_plain_marks_nothing() {
    let (_, found) = links("[a post](https://lemmy.ml/post/1)", PLAIN);
    assert_eq!(found, vec![(LINK, "https://lemmy.ml/post/1".to_string())]);
}

/// A relative link shows no domain after its words under any site, even one carrying an address
/// in its query, which is not its host.
#[test]
fn markdown_relative_link_shows_no_domain() {
    for site in [PLAIN, LEMMY, CHAN] {
        let (text, _) = links("[x](/redirect?u=https://x.com/a) [y](/search)", site);
        assert_eq!(text, "x y", "site {site}");
    }
}

/// A footnote's links are the site's too: an own page in one is marked and shows no domain, as
/// in the body.
#[test]
fn markdown_footnote_links_are_the_sites() {
    let md = "text[^1]\n\n[^1]: see [post](https://lemmy.world/post/1) and [else](https://example.com/a)";
    let (text, _) = links(md, LEMMY);
    assert!(
        text.ends_with("1 see post and else (example.com)"),
        "{text:?}"
    );
    let (text, _) = links(md, PLAIN);
    assert!(
        text.ends_with("1 see post (lemmy.world) and else (example.com)"),
        "{text:?}"
    );
}
