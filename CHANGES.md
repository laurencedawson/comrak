# Fork Changes

A [comrak](https://github.com/kivikakk/comrak) fork tailored for
[syncdown](https://github.com/laurencedawson/syncdown), an Android
markdown renderer.

Since October 2026 the fork is upstream main (229fc35, v0.56 and later) with
this fork's changes on top, rebuilt rather than merged: see [Rebase onto
upstream 229fc35](#rebase-onto-upstream-229fc35-october-2026). The fork before
the rebase is tagged `pre-rebase-0.56`, so the commit links below into that
history still resolve. Highlights:

| Area | What |
|---|---|
| [Extensions](#extensions) | Lemmy mentions + spoiler directive, `sanitize_input` (invisible-char + leading-hard-break stripping). Smartypants is retired — see [below](#retired-parsesmart-and-the-forks-smart-additions) |
| [URL resolution](#url-resolution) | `resolve_url()`: proxy/redirect unwrap, pict-rs thumbnail rewrite, video exclusion; runs once at the blob's single emission chokepoint |
| [Sites and app links](#sites-and-app-links-df385a7-3edc676-5fc6ff3-b381692-736f5cd-a721133-89a23ad-e277031-94bb2f6-d0c39d5) | Each site syncdown reads compiled in, one function per question: its HTML's rules, its own pages, written as `APP_LINK` spans for the app to open itself, and its own host, after whose links no domain is written |
| [Blob renderer](#blob-rendering) | One-pass AST → compact binary blob feeding syncdown's `FastSpannable`; streams straight into pre-sized buffers with no intermediate representation |
| [HTML into the blob](#html-into-the-blob-b6d9a01-2f00ef6) | `render_html_blob`: a site's comment HTML straight into the same blob through the same writer, with no markdown between; mdream's entities and tag scan; `HtmlRules`, each site's tags |
| [Articles into the blob](#articles-into-the-blob-6158cdd-ce02d6f) | `render_article_blob`: a whole web page's article, as Readability picks it, into the same blob: its words, the title and lead picture the caller's; dom_smoothie's port of Readability.js run on a tree of our own |
| [Text normalization](#text-normalization) | The blob's rendering policy, flag-free by design: collapse spaces, typed typographic Unicode → ASCII, `(c)/(r)/(tm)/+-` → ©®™± |
| [Parse entry](#parse-entry-point) | Single scoped `parse_document_zerocopy` — arenas owned internally, AST dropped with the closure |
| [Utility helpers](#utility-modules-used-by-the-renderer) | `resolve_url`, `extract_domain`, `is_image_url`, and the `text` normalizers — public, zero-copy on common paths |
| [Benchmarking](#benchmarking) | `profile_parse` + `alloc_bench` + shared corpus (including contraction-dense `prose`) for tracking throughput and allocation pressure |
| Alloc / throughput (throughout) | Inline text borrows a pooled string arena, contiguous borrowed runs merge in place, per-thread pools for block lines and paragraph strings, `Cow::to_mut` avoidance, custom `UnsafeCell` arena. Upstream now carries the slim `Ast` and per-document delimiter arena itself |

## Extensions


### `lemmy_mention` ([f0bf9d9])

Converts Lemmy user and community mentions to links. Context-aware, won't match inside code blocks, code spans, or existing links.

- `@user@instance.com` becomes a link to `https://instance.com/u/user`
- `!community@instance.com` becomes a link to `https://instance.com/c/community`

Name validation follows Lemmy's rules: 3-20 characters, `[a-zA-Z0-9_]`. Domains allow `.`, `-`, `_`, and `:` (for ports).

### `lemmy_spoiler` ([f0bf9d9], [a676324], [82663af])

Parses Lemmy spoiler blocks using the `:::spoiler` directive. Only matches `:::spoiler`, other `:::` directives are ignored. The parser mirrors Lemmy's `^spoiler\s+(.*)$`: a whitespace boundary after `spoiler` is required and the title is mandatory (whitespace-only titles are rejected).

```markdown
:::spoiler Click to reveal
Hidden content with **markdown** support
:::
```

Renders as `<details>/<summary>` in HTML. In the blob it emits a title/content pair — `LEMMY_SPOILER_TITLE` over the title line (alongside `BOLD` and `LINK_SIZE` for a generous touch target) and `LEMMY_SPOILER_CONTENT` over the body. The two sort adjacently so the consumer pairs them via `cache[i±1]` with no extra plumbing; the blob's `has_spoiler_body` flag ([ea8f274]) lets the consumer detect spoiler bodies without scanning spans. The inline `>!…!<` / `||…||` spoiler span was retired ([f753071]).

### `sanitize_input` ([f0bf9d9], [24cd0cc])

Pre-parse author-input hygiene, one flag. Formerly two (`strip_invisible` +
`strip_leading_breaks`), merged because both are unconditional hygiene and
always enabled together. Both strips must run before parsing — invisible
characters would otherwise survive into link destinations and mention targets.

**Invisible-character strip.** Zero-copy when input is clean. Preserves ZWJ and VS16 for emoji.

| Character | Code | Why |
|-----------|------|-----|
| Zero Width Space | `U+200B` | No rendering use |
| BOM / ZWNBSP | `U+FEFF` | File marker |
| Word Joiner | `U+2060` | No rendering use |
| Function Application | `U+2061` | MathML only |
| Invisible Times | `U+2062` | MathML only |
| Invisible Separator | `U+2063` | MathML only |
| Invisible Plus | `U+2064` | MathML only |
| Soft Hyphen | `U+00AD` | Hidden text vector |
| Combining Grapheme Joiner | `U+034F` | Obscure ligature control |
| Mongolian Vowel Separator | `U+180E` | Deprecated |
| Bidi embedding controls | `U+202A`-`U+202E` | Trojan Source vector |
| Bidi isolate controls | `U+2066`-`U+2069` | Trojan Source vector |
| Zero Width Non-Joiner | `U+200C` | Commonly abused |
| Arabic Letter Mark | `U+061C` | Commonly abused |
| LTR Mark | `U+200E` | Commonly abused |
| RTL Mark | `U+200F` | Commonly abused |
| Variation Selectors 1-15 | `U+FE00`-`U+FE0E` | Commonly abused |

Preserves ZWJ (`U+200D`) for emoji sequences and VS16 (`U+FE0F`) for emoji presentation.

**Leading-break strip** ([24cd0cc]). Strips a leading run of whitespace +
hard line breaks (`\` + newline). User content from Lemmy / Reddit sometimes
starts with a stray hard break that would otherwise render as blank space at
the top of the post. Zero-copy when the input has no leading break.

### Retired: `parse.smart` and the fork's smart additions ([eb5580e], [0bb0a38])

The fork once enabled smartypants and extended it with symbol replacements
(`(c)`→©, capping of `????`→`???`, etc. — [f0bf9d9]). Measurement showed the
feature cost 7% (docs-style) to 55% (contraction-dense prose) of total
preprocess: every smart transform splits text into extra AST nodes (4.2x on
prose), and the blob's `prefer_ascii` then allocated again to revert
quotes/dashes/ellipsis to ASCII — work done and undone per contraction.

The retirement contract (pinned by `src/tests/smart_parity.rs`):

| Transform | Fate |
|---|---|
| Smart quotes, ellipsis | Deleted — provably invisible end to end (always reverted) |
| Dashes (`--` squash), guillemets, punctuation capping | Deleted — the reader sees exactly what the author typed (`x >> 2` no longer corrupts to `x » 2`; runs adjacent to mentions no longer silently unlink) |
| `(c)/(r)/(tm)/+-` → ©®™± | Kept — relocated to the blob writer's [text normalization](#text-normalization) |

Upstream's smart code is intact and dormant behind the flag; the fork's own
handlers and lookahead tables are deleted ([0bb0a38]), returning `inlines.rs`
toward upstream. Every fork option set carries an explicit
`parse.smart = false` with a same-line rationale. Measured on the prose
corpus: preprocess 2.27x faster, blob allocations 686 → 6, AST nodes
1,521 → 361.

## URL resolution

Every URL the blob emits passes through one finalizer, **`comrak::resolve_url(&str) -> ResolvedUrl<'_>`** (re-export of `comrak::parser::url::resolve_url`). `ResolvedUrl` is a newtype whose only constructor is `resolve_url`; the blob writer's emission methods (`span_url`/`emit_image`) accept `&ResolvedUrl` and never a raw `&str`. So "every emitted URL is resolved, exactly once" is a compile-time invariant rather than a convention — [7c60125] introduced the single chokepoint, [b0b8def] made it a type and dropped the redundant second resolve. It wraps a `Cow` and derefs to `str`, staying zero-copy when nothing needs rewriting.

It does three things:

**1. Proxy / redirect unwrap and short-URL expansion.**

| Pattern | Example | Result |
|---------|---------|--------|
| Google redirect | `google.com/url?q=<url>` | unwrapped URL |
| Google AMP | `google.com/amp/s/<url>` | `https://<url>` |
| YouTube redirect | `youtube.com/redirect?q=<url>` | unwrapped URL |
| YouTube short | `youtu.be/<id>`, `youtu.be/watch?v=<id>` | `youtube.com/watch?v=<id>` |
| YouTube mobile | `m.youtube.com/...` | `www.youtube.com/...` |
| Facebook redirect | `l.facebook.com/l.php?u=<url>` | unwrapped URL |
| DuckDuckGo proxy | `external-content.duckduckgo.com/iu/?u=<url>` | unwrapped URL |
| Discord proxy | `images-ext-N.discordapp.net/external/?url=<url>` | unwrapped URL |
| Skimresources | `go.skimresources.com/?url=<url>` | unwrapped URL |
| Voyager | `vger.to/<instance>/<path>` | `https://<instance>/<path>` |
| Lemmy v3 proxy | `<instance>/api/v3/image_proxy?url=<url>` | unwrapped URL |
| Lemmy v4 proxy | `<instance>/api/v4/image/proxy?url=<url>` | unwrapped URL |

**2. pict-rs thumbnail rewrite** ([7c60125]). Lemmy pict-rs image URLs gain a `thumbnail=400&format=webp` query so the body shows a server-rendered preview instead of the full-size original (thumbnailing is the only processing Lemmy honors on image URLs; crop/resize are ignored). The full-size original is one tap away because the host's image viewer strips the query. Non-static formats (gif and video) are left untouched.

**3. Video exclusion** ([7c60125], [79f72b4]). Video extensions on image hosts and pict-rs paths (`mp4`, `webm`, `gifv`, …) are kept out of image handling via a shared `path_ext` check:

- Autolink-to-image detection (`is_image_url`) excludes them, so a bare `<…/clip.mp4>` stays a link rather than becoming a broken image embed.
- The thumbnail rewrite skips them (no still frame from a video).
- `![](…/clip.mp4)` image syntax is downgraded to an inline link — the alt text, or the URL when there is no alt — pointing at the original video.

> **Naming note.** The fork's resolver was renamed `clean_url` → `resolve_url` (and the old `NodeLink::cleaned_url()` accessor removed in favour of the `span_url` chokepoint) so it no longer collides with upstream's spec-unescaping `strings::clean_url` ([7c60125]).

## Sites and app links ([df385a7], [3edc676], [5fc6ff3], [b381692], [736f5cd], [a721133], [89a23ad], [e277031], [94bb2f6], [d0c39d5])

Both walks take the site they read for: a `u8` from `blob::sites` (`PLAIN`,
`HACKER_NEWS`, `CHAN`, `LEMMY`, `LOBSTERS`, `READER`), generated from syncdown's
`sites.toml` by its Gradle `generateSites`, as the span types are. Everything a
site knows about itself is in `src/blob/site.rs`, one function per question
matching on the id, every other site taking the default, and each site's code in
`src/blob/site/`:

| Question | Answers |
|---|---|
| `site::html(site)` | How its HTML reads: its [`HtmlRules`](#htmlrules-bca01f9-29b28a5-50226a9-2090141-10e87cf-5fc6ff3) |
| `site::page(site, url)` | What a resolved address is to it, read once: `Own`, one of its pages, for the app to open itself; `Internal`, on its own host, where the domain after a link's words tells its reader nothing; or `Elsewhere` |
| `site::link_size(site)` | Whether its links are drawn larger than their words (`LINK_SIZE`), a finger's target in a comment; every site's but `READER`'s |
| `site::link_domain(site)` | Whether a link elsewhere has the domain after its words; every site's but `READER`'s |

`READER` is an article's site, not an app's: `render_article_blob` writes under
it whatever the app's own, and its links are words in a sentence, at their size
and with no domain after them. It owns no pages and reads HTML as plain. So the
view is always told, it always answers a blob, plain words too; every other
site's plain words answer none, as before.

One `BlobWriter::link` writes every link for both walks, written, autolinked or
a Lemmy mention: a site's own page is an `APP_LINK` span, any other a `LINK`,
its domain after the words where they lack it, unless the link is on the site's
own host, which tells its reader nothing: a 4chan board's archive or
`lobste.rs/newest` carry none. An `APP_LINK` carries its real address in
`url_data` as a `LINK` does, and has no suffix, since the app shows the page
itself.

Each site's own pages, from its own documentation:

| Site | Own pages | From |
|---|---|---|
| Hacker News | `item?id={id}`, `user?id={name}`, on `news.ycombinator.com` | its API's README |
| 4chan | `/{board}/thread/{op}`, its title after it, a trailing slash, a post `#p{no}`; a board's index `/{board}/` and its catalog `/{board}/catalog`, a search after `#s=`, where a `>>>/g/` quotelink goes; on `boards.4chan.org` and `boards.4channel.org` | its API's documentation |
| Lobsters | `/s/{id}`, its title after it, `/s/{id}/{title}/comments/{id}`, `/stories/{id}`, `/c/{id}`, `/comments/{id}`, `/~{name}`, `/u/{name}`, `/t/{tags}` and its pages, on `lobste.rs` | its `config/routes.rb` |
| Lemmy | `/post/{id}`, `/post/{id}/{comment}`, `/comment/{id}`, `/c/{name}`, `/m/{name}`, `/u/{name}`, a name with `@{instance}`, on any host but those whose own pages share the shapes | lemmy-ui's routes |

- Lemmy is federated, so its host never decides; the hosts whose own pages share
  its shapes are left out, each with its subdomains: reddit's `/u/{name}`,
  YouTube's, Patreon's and Rumble's `/c/{channel}`, Tumblr's `/post/{id}`,
  Medium's `/m/`, Lobsters' `/c/{id}`. The list can only name what it knows
  (PeerTube's channels are `/c/{name}` on any host), so an `APP_LINK` the app
  cannot resolve should open as the address it carries. The other sites are on
  their own hosts alone
- A relative address, opening on `/`, is on the site's own host, so the site's
  shapes are its own there too: lemmy-ui writes a completed community as
  `[!rust@lemmy.ml](/c/rust@lemmy.ml)`, and the markdown walk has no page to
  resolve it against. `//host` names another host, so is none
- An address with a user, a port or a scheme other than `http` and `https` is
  no site's page. `PLAIN` owns none
- A footnote's writer is the document's, site and all, so a link in a footnote
  is marked as one in the body
- Nothing is copied or allocated: `site::Url` reads an address as borrowed
  slices of itself, and compares bytes, so a host in another script is read
  whole. `alloc_bench`'s `site/is-own` line counts 0 allocations
- Checked against an oracle written from the same documentation over every
  link the walks write for 3,808 4chan comments, 1,528 Hacker News comments, 58
  Lemmy and 362 Lobsters texts: 3,849 links, 3,370 of them own pages, none
  disagreeing, and no allocation reading all of them as each of the five sites.
  Every 4chan quotelink to a thread was an `APP_LINK`, and nothing else. Boards
  and catalogs, the hosts list, relative addresses and `Page::Internal` came after
  that run

This replaces the `app-link://` scheme a host wrote into markdown for its own
screens ([1b09a35]): `resolve_url` and `extract_domain` now treat it as any
other text. Pinned by `src/tests/site.rs`, and in `src/tests/blob_html.rs` by
`site_link_own_pages_are_app_links`, `site_link_boards_and_catalogs_are_the_apps`
and `site_link_other_pages_stay_addresses`.

## Blob rendering

A compact binary representation of a parsed markdown document. The walker
visits the AST once and appends text + span metadata directly to a single
buffer, building no intermediate representation — the AST streams straight
into pre-sized output buffers (the only per-node allocations are URL rewrites
and footnote text; see [alloc_bench](#cargo-run---release---example-alloc_bench)).
The layout is optimised for cheap decoding on the consumer side — integer
offsets, UTF-16 positions, sorted spans.

### Parse entry point

**`comrak::parse_document_zerocopy(md, opts, f)`** — a scoped parse entry that
moves each paragraph's content into a pooled string arena and lets inline text
borrow it, cutting per-node allocations. Upstream's text coalescing still runs;
contiguous borrowed runs are merged in place first, so it has nothing to copy.
It skips the sourcepos fix-up walks, which the blob never reads. The closure receives the root; the arenas drop on return
so the borrow can't escape. Before they drop, the block contents and paragraph
strings go back to the calling thread, so the next parse on it allocates neither
(up to 256 of each, strings under 4 KB): a feature-heavy document parses with a
third of the allocations it did.

```rust
let blob = comrak::parse_document_zerocopy(md, &opts, |root| {
    comrak::blob::render_blob(root, md, comrak::blob::sites::PLAIN)
});
```

### Utility modules (used by the renderer)

Self-contained helpers used internally by the blob renderer, exposed so
callers can reuse the same logic elsewhere.

| Function | Returns | Notes |
|---|---|---|
| `comrak::resolve_url(&str)` | `ResolvedUrl<'_>` | Re-export of `comrak::parser::url::resolve_url`. URL finalization (proxy unwrap, short-URL expansion, pict-rs thumbnail, video exclusion). Returns a newtype that derefs to `str`; the only way to obtain one, so the blob writer can require it. See [URL resolution](#url-resolution) for the full table. |
| `comrak::parser::url::extract_domain(&str)` | `Option<Cow<'_, str>>` | Host extraction for display-suffix dedup. Strips `www.`; zero-copy when the host is already lowercase ASCII. |
| `comrak::image_url::is_image_url(&str)` | `bool` | Heuristic URL → image detection (by known host, path pattern, or file extension). |
| `comrak::text::collapse_whitespace(&str)` | `Cow<'_, str>` | Collapses runs of spaces to a single space. Zero-copy when the input has no double-space. |
| `comrak::text::prefer_ascii(&str)` | `Cow<'_, str>` | Typed typographic Unicode (curly quotes, en/em dashes, ellipsis) → ASCII. Zero-copy when none present. |
| `comrak::text::typographic_symbols(&str)` | `Cow<'_, str>` | `(c)/(r)/(tm)/+-` → ©®™±. Zero-copy when nothing matches; an `Owned` return always carries non-ASCII (used for the writer's in-place fast-path downgrade). |

### Text normalization

The blob writer's rendering policy, stated once in `src/text.rs`'s module doc
and applied unconditionally to author prose (`Text` nodes) via
`BlobWriter::write_prose` — deliberately not options: rendering opinions never
live in flags. Author text renders as typed, with exactly three exceptions:
runs of spaces collapse to one, typed typographic Unicode normalizes to ASCII
(keeping the consumer's zero-copy `byte[]` path wide), and the four symbol
codes render as their glyphs. The last two act on disjoint character sets, so
the passes commute and the pipeline is idempotent — pinned by a tripwire test
in `smart_parity`. Code spans, code blocks, and raw HTML blocks bypass the
policy structurally (their visit arms call `write_text` directly). When a
symbol substitution introduces non-ASCII into an ASCII-fast-path render, the
writer downgrades its accounting in place rather than re-rendering — prior
offsets remain exact because everything before the substitution was ASCII.

### Render entry point

`comrak::blob::render_blob(root, input, site) -> Option<Vec<u8>>` renders the
AST at `root` against the original source `input`, its links marked as
[`site`'s](#sites-and-app-links-df385a7-3edc676-5fc6ff3-b381692-736f5cd-a721133-89a23ad-e277031-94bb2f6-d0c39d5). Returns `None` when the
document has no spans and the output text equals the input (the caller can
then use the raw input string directly and skip allocating a blob).

### HTML into the blob ([b6d9a01], [2f00ef6])

`comrak::blob::render_html_blob(input, site, base_url) -> Option<Vec<u8>>`
reads a site's comment HTML, as its API sends it, into the same blob, its
relative links resolved against `base_url` as a browser resolves them, with no
markdown between. A second walk, over the HTML in place of comrak's tree,
writes through the one `BlobWriter`, so a comment reads the same either way and
an enhancement to the writer reaches both.

```
comment HTML ──▶ html/mdream/scan.rs ──▶ html.rs ──▶ BlobWriter ──▶ blob
                 (tags, attributes,      (the walk, one pass:
                  entities)               each tag as its markdown)
```

A comment is a few paragraphs written top to bottom, so the walk reads each tag
as the scan finds it and writes it at once, holding only the tags still open.

- One pass, no tree: `src/blob/html.rs`, a child of `blob` so the writer stays
  private. Each tag is written as the writer writes the markdown it stands for
- Read so far: `<p>` (Hacker News opens every paragraph but the first with a
  bare one, closing none), `<br>`, `<b>`/`<strong>`, `<i>`/`<em>`, `<a href>`
  (resolved, the domain suffix, a link whose words are its own picture address
  as the picture), `<pre>` kept as written, comments, every entity, HTML
  whitespace. A paragraph opening on `>` is a quote, `>>` nested, as the site's
  posters write one. Other tags keep their words
- The same shape as `render_blob`: the ASCII fast path, the re-render when the
  output is not ASCII, `None` for unchanged text. No plain-text gate: as for
  markdown's `needs_parse`, that is syncdown's
- `<br>` is a newline and a run of them a blank line at most, as markdown caps
  them; a `<wbr>` inside a word is read through
- From mdream 2.0.3 (MIT, `src/blob/html/mdream/`): `entities.rs` and
  `entities_generated.rs` as written, every WHATWG entity with the standard's
  text and attribute rules; `scan.rs` cut to a whole string, its chunk carries,
  discarded-byte scans and attribute budgets left behind. Its markdown writer,
  selectors, Tailwind, URL and plugin code are not here
- `scan_tag` ([6158cdd]) hands over `Attrs::Only(names)`, at most eight, as the walk reads
  `href` and `class`, or `Attrs::All`, every attribute, as the
  [article tree](#articles-into-the-blob-6158cdd-ce02d6f) reads them. The mode is chosen once
  and each has its own loop, so the walk's is the loop it was: within noise on
  `bench_all`'s HTML corpora, against the scan before it
- Built as htmlrak, a crate of its own mirroring this writer, and moved in
  whole: blobs byte-identical to htmlrak over 1,551 inputs, 1,528 of them
  fetched Hacker News comments
- `tests/blob_html.rs` ([b2f39fe]): htmlrak's tests, and a parity table of 28
  comments as HTML and as markdown, parsed both ways and asserted to read the
  same, so the two walks cannot drift
- Not yet: the invisible-character strip `sanitize_input` gives markdown

#### `HtmlRules` ([bca01f9], [29b28a5], [50226a9], [2090141], [10e87cf], [5fc6ff3])

How a site's HTML reads, compiled in with the site: each site's `HTML` in
`src/blob/site/`, found by `site::html`, so the walk itself names no site and
nothing outside the crate builds one.
Whatever the site, `scheme://` and `www.` addresses
among the words are links, as the autolink makes them in markdown: their
bounds are read by `parser::autolink::url_in` and `www_in`, string-level forms
of `url_match` and `www_match` sharing their domain and delimiter checks.

| Field | Reads |
|---|---|
| `quote_marks` | A paragraph opening on `>` is a quote, its marks no text (Hacker News) |
| `tags` | `TagRule`s by tag and optional class: greentext (a `GREENTEXT` span over its words, on a line of its own, as 4chan colours a line opened on `>`), a strike, a spoiler under a title (Lemmy's block) |

Hacker News reads `quote_marks`; 4chan its greentext, dead links and spoilers
as tags; every other site neither.

Built for 4chan, checked on 3,808 real comments from three boards. The 3,040
without greentext or spoilers read byte-identical to the markdown 4chan's Java
converter wrote, with each `<br>` a hard break, except 25 where that converter
escaped a `_` inside a typed address, leaving a backslash in the link. The 768
with greentext keep every word, and their quotes cover exactly the quoted
lines; markdown cannot say those, since its lazy continuation takes the line
after a quote into it. About 2 us a comment.

### Articles into the blob ([6158cdd], [ce02d6f])

`comrak::blob::render_article_blob(html, base_url) -> Option<Vec<u8>>` reads a
whole web page, picks its article out of it as Readability does, and writes it
into the same blob through the one `BlobWriter`: the article's words, its
links and pictures resolved against `base_url`. The page's title and the
picture it leads with are the caller's to show, as a header above the body;
the article's own copy of that picture is left out, and so is its byline. `None` where
nothing on the page reads as an article. syncdown's `preprocessArticle`.

```
page HTML ──▶ article/tree.rs ──▶ article/smoothie/ ──▶ article.rs ──▶ BlobWriter ──▶ blob
              (parse: mdream's    (pick: Readability    (write: the walk)
               scan into a tree)   scores, keeps, cuts)
```

A page cannot be read like a comment. Readability scores each paragraph and
passes the score up to its parent and grandparent, weighs siblings, climbs to a
parent that scores higher, and cuts sidebars and share bars out, so it needs
the whole page as a tree it can query and change. `tree.rs` is that tree,
`smoothie/` the Readability that works on it, and `article.rs` the walk over
what is left, as `html.rs` is the walk over a comment.

- A tree, as an article's blocks need: `src/blob/article/tree.rs`, an arena
  built by mdream's scan, every attribute kept, with the closes HTML implies
  (a `<p>` at the next block, an `<li>` at the next item, rows and cells,
  headings, links) and no more of the standard's repairs. `<script>`, `<style>`
  and `<template>` are no part of the page. Each element's attributes sit in
  one pool per tree, links are `u32`s, and each text node's share of the
  whitespace-collapsed character count is worked out once, as it is read
- Picking the article: `src/blob/article/smoothie/`, from dom_smoothie 0.19.0
  (MIT), Mykola Humanov's port of Mozilla's Readability.js (Apache-2.0, its
  license and NOTICE beside it). `grab.rs`, `prep.rs`, `helpers.rs` and
  `glob.rs` are dom_smoothie's, moved from dom_query onto the tree with their
  logic as it was, its byte check before each pattern search kept.
  `smoothie.rs` reads the title (`og:title` and the like, else `<title>` with
  the site's name cut off), which the scoring uses to drop a heading that
  repeats it, takes out the element that reads as a byline, whether or not an
  author `<meta>` names one (as Readability.js does; dom_smoothie looked only
  without one), and reads the lead picture (`og:image`). A share element is
  told by a hyphen as well as a space or underscore (`share-toggle`), as
  Readability.js's `\b` tells it and dom_smoothie's split did not. Ours alone,
  as the title is the caller's header: date lines go once the article is
  picked, a short element its class or id names a date or holding a `<time>`
  with a word or two about it ("Updated:"), with a parent it leaves a word or
  two in, an eyebrow's category; a `<time>` in a sentence stays. JSON-LD,
  the readability check and dom_smoothie's text modes are left behind
- The walk, `src/blob/article.rs`: headings, paragraphs, lists nested and
  numbered from their `start`, quotes, `<pre>` as written, inline code,
  rules, emphasis, super- and subscript, links and pictures (`src`, or the
  widest of a `srcset`; a link around a picture is the picture). The lead
  picture is left out where the article shows it already. A link to the
  article's own host takes no domain after it; a `#` or `javascript:` link is
  words. Table rows read across, `a | b`, until tables are `TABLE` spans
- Checked against dom_smoothie itself, on 27 real pages: Wikipedia, MDN, The
  Verge, NPR, Substack, GitHub, blogs, a paywall and two bot walls. The same
  words on every page; the titles and bylines too, but where dom_smoothie read
  them from JSON-LD. At most 4.2 ms a page on an M-series Mac, the blob
  written too, 2.7 times dom_smoothie's speed over the 27, and about 150 KB of
  an arm64 build against dom_smoothie's 1 MB with html5ever under it
- `tests/blob_article.rs` ([ce02d6f]): extraction, the tree, every block and
  inline, links, pictures and the lead picture

### Standalone use (without the fork's parse-side changes)

The blob renderer is decoupled from how the AST is built. `render_blob` walks a
finished tree through comrak's public node API (`node.children()`,
`node.data().value`) and never inspects the parser, so it runs against a stock
parse just as well as the optimized one:

```rust
let arena = comrak::Arena::new();
let root = comrak::parse_document(&arena, md, &options); // vanilla entry, no zero-copy pooling
let blob = comrak::blob::render_blob(root, md, sites::PLAIN); // identical bytes to the zerocopy path
```

[`parse_document_zerocopy`](#parse-entry-point) and the fork's allocation/throughput
commits only change how fast the AST is produced and how much it allocates — the
tree handed to `render_blob` is structurally identical and the emitted bytes are
the same. `sanitize_input` and `lemmy_mention` are likewise optional: drop them
and the renderer still works (mentions just render as ordinary text/links rather
than being linkified).

Lifting `blob.rs` onto a clean comrak needs only:

- **`src/blob.rs`** plus the generated `src/blob/span_types.rs` (the span-type id
  constants — see the [Benchmarking](#benchmarking) note on `generateSpanTypes`).
- **Three self-contained helper modules** it calls, none of which touch the
  parser: [`image_url`](#utility-modules-used-by-the-renderer)
  (`is_image_url`/`is_video_url`), `parser::url` (`resolve_url`/`extract_domain`),
  and `text` (`collapse_whitespace`/`prefer_ascii`/`typographic_symbols`).

The one genuine coupling is the `lemmy_spoiler` extension: `blob.rs` has a match
arm for the fork's `NodeValue::LemmySpoiler`, so compiling it requires that
variant (and its `NodeLemmySpoiler` struct). If you don't need spoiler blocks,
delete that single arm and the renderer builds against unmodified node types.

### Blob layout

All multi-byte integers are little-endian i32. All text positions are
UTF-16 code units (matching Java `String` indexing directly).

```
┌───────────────────┬───────────────────────────┬─────────┬─────────┬─────────┬──────────┐
│  text_len (4 B)   │ span_count (3 B) │ flags │  text   │   pad   │  spans  │ url_data │
├───────────────────┼──────────────────┼───────┼─────────┼─────────┼─────────┼──────────┤
│ bytes of text     │ number of spans  │  1 B  │ UTF-8   │ 0..3 B  │ 16 B    │ raw URL  │
│ (header, excl.)   │ (low 24 bits)    │       │ source  │ to 4B   │ per     │ bytes    │
│                   │                  │       │         │ align   │ span    │ (indexed │
│                   │                  │       │         │         │         │ by spans)│
└───────────────────┴──────────────────┴───────┴─────────┴─────────┴─────────┴──────────┘
```

Byte offsets, in order:

- `0..4` — `text_len` (i32)
- `4..7` — `span_count` (low 24 bits; max 16 M spans)
- `7..8` — `flags` (u8):
  - bit 0: `IS_ASCII` — every byte in the text section is < 0x80
  - bit 1: `HAS_SPOILER_BODY` — at least one `LEMMY_SPOILER_CONTENT` span exists ([ea8f274])
  - bits 2-7: `SITE` — the site the blob was read for, its id from `blob::sites` (`PLAIN` 0
    … `READER` 5), so a reader of the blob knows the rules it was written under. Ids fit six
    bits (0-63), which syncdown's `generateSites` checks; renumbering a site means bumping
    the databases that keep blobs, as renumbering a span type does
- `8..8 + text_len` — text bytes (UTF-8)
- next — zero padding to align to a 4-byte boundary (0..3 bytes)
- next — `span_count × 16` bytes of packed span records
- next: URL bytes referenced by `LINK`/`APP_LINK`/`IMAGE` spans (rest of blob)

### Span record (16 bytes each)

Each span is four packed i32 fields, stored in a contiguous array sorted by
`(start asc, end desc, type asc)`:

```
 Byte:   0    1    2    3    4    5    6    7    8    9   10   11   12   13   14   15
        ┌────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┬────┐
        │      start i32    │      end   i32    │   type  i32       │   data  i32       │
        └────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┴────┘
        │                   │                   │                   │
        │UTF-16 code unit   │UTF-16 code unit   │span type id       │type-specific data
        │(inclusive start)  │(exclusive end)    │(see SpanTypes.java)│(see below)
```

### Data-field encoding (varies by span type)

The `data` field is a 32-bit slot whose meaning depends on `type`:

**URL-carrying spans** (`LINK`, `APP_LINK`, `IMAGE`): offset + length into the trailing
`url_data` section:

```
bit:  31                                                         12 11              0
     ┌─────────────────────────────────────────────────────────────┬──────────────────┐
     │              offset  (20 bits, up to 1 MiB)                 │ length (12 bits) │
     └─────────────────────────────────────────────────────────────┴──────────────────┘
```

**List items** (`LIST_ITEM`) — nesting indent + list number:

```
bit:  31                      16 15                               0
     ┌────────────────────────────┬────────────────────────────────┐
     │    indent (16-bit)         │        number (16-bit)         │
     └────────────────────────────┴────────────────────────────────┘
```

`number`:
- `0` for bullet lists
- `1..n` for ordered lists

**Task items** (`TASK_ITEM`) — a list item drawn as a checkbox, nesting indent + checked:

```
bit:  31                      16 15                               0
     ┌────────────────────────────┬────────────────────────────────┐
     │    indent (16-bit)         │   checked (bit 0: 1 = checked) │
     └────────────────────────────┴────────────────────────────────┘
```

A task item in an ordered list still takes its ordinal, so the item after it numbers on.

**Blockquotes** (`QUOTE`) — nesting depth in the low bits:

```
bit:  31                                                            0
     ┌─────────────────────────────────────────────────────────────┐
     │                       depth (0, 1, 2, …)                   │
     └─────────────────────────────────────────────────────────────┘
```

**All other span types** — `data = 0` (unused).

### Text-buffer semantics

- ASCII-only `write_text` uses byte-length for UTF-16 position accounting
  (fast path). Multi-byte input falls through to a per-code-unit counter.
- Pending newlines are deferred: `nl(n)` queues up to `n` newlines, flushed
  only when more text follows. This avoids trailing whitespace at the end
  of the document and between sibling block elements.
- Images are padded with one blank line on each side, counting any existing
  trailing newlines past spaces, so consecutive images don't stack padding, even
  a space apart. Pinned shape by shape, in both walks, by
  `image_has_one_blank_line_before_it` and `a_to_a_picture_has_one_blank_line_before_it`.
- An empty code block writes nothing and queues no gap, so a document never
  opens on a blank line because of one, and the blocks around it stay one blank
  line apart.
- Zero-length spans (start ≥ end in UTF-16 units) are silently dropped at
  emission time — they carry no information.

### Footnotes

Footnote definitions are buffered during walk, then emitted at the tail
after an `HRULE` separator. Inline references become SUPERSCRIPT spans
containing a 1-based index; definitions are prefixed with the same index.

### Span sort

Before serialization, spans are sorted by `(start asc, end desc, type asc)`.
This guarantees:

- Wider containers precede nested children at the same start position, so a
  consuming layout engine applies outer margins before inner ones.
- The sort key is packed into a single `u64` for comparison: `start<<32 |
  (0xFFFFFF - end)<<8 | type` — a plain u64 compare replaces a 3-field
  tuple walk.
- Spans are emitted after their node's children, so each only has to move
  back past its own descendants: an insertion sort is linear plus those
  moves ([718160c]). After 8 moves per span, which only deep nesting
  reaches, the rest goes to `sort_unstable_by_key` on the same key.

### Limitations

Blob-format bit budgets. All are well above realistic inputs, but worth
knowing before you feed the renderer adversarial content.

- **URL length**: 4095 bytes max per URL (12-bit `length` field in URL-
  carrying spans). Longer URLs are silently truncated, backing off to a
  UTF-8 char boundary so the host's decode never sees a split codepoint.
- **Table cell length**: 65 535 bytes max per cell (u16 length prefix).
  Longer cells are silently truncated, on a char boundary as above.
- **URL data section**: 1 MiB max total (20-bit `offset` field). An
  overflow here would silently wrap; in practice a document would need
  thousands of distinct long URLs to hit it.
- **Text length**: ~16 M UTF-16 code units for stable sort ordering
  (24-bit `end` field in the packed sort key). Beyond that, span ordering
  may reorder equal-start spans incorrectly. The blob itself can still
  hold longer text — only the sort comparator is bounded.
- **Span type id**: 255 max (u8 in the packed sort key). `SpanTypes.java`
  currently uses < 32 ids, plenty of headroom.
- **List item number**: 65 533 max (16-bit), with `0xFFFE` and `0xFFFF`
  reserved as task-list sentinels (unchecked / checked).
- **List indent**: 65 535 max nesting levels (16-bit).
- **Nesting depth**: bounded by memory, not the call stack. The walk keeps
  its own stack of open containers ([443a6fd]); the recursive walk before it
  aborted the process on a 1 MB thread at about 1,570 nested quotes or 780
  nested list items.


## Benchmarking

Three examples and a shared input corpus for tracking parse throughput and
allocation pressure across changes. All three parse with
`benchmarks::production_options()`, the option set syncdown ships with
(mirrored from its JNI shim; keep the two in step): a bench on any other set
hides what production pays, which is how `sanitize_input`'s scan cost 30% of a
prose parse for two optimisation passes without showing.

> **Generated files.** `src/blob/span_types.rs` and `src/blob/sites.rs` are
> written by syncdown's Gradle `generateSpanTypes` and `generateSites` tasks
> and committed here, so `cargo` runs in isolation. Change the ids in
> syncdown's `span_types.toml` and `sites.toml`, never in these files.

### `src/benchmarks.rs`

`benchmarks::html` ([c392115]) holds the same corpora written as the HTML a
site would send, and ten real Hacker News comments as HTML and as markdown;
`bench_all` and `alloc_bench` report both walks side by side.

Public module exposing a range of synthetic inputs: `PLAIN`, `SIMPLE`,
`MEDIUM`, plus generators `deep_nesting()`, `heavy_inline()`, `complex()`,
`long_doc()` (a realistic ~13 KB post), and `prose_doc()` (~13 KB of
contraction-dense comment prose — the shape the other corpora under-weight,
which let smartypants' fragmentation cost stay invisible for two full
optimization passes; weight it for any change touching text-node handling).
Both examples below consume this module, so adding a new input shape lands
in both reports at once. Also fed to a `bench_all` smoke test that runs
under `cargo test`.

### `cargo run --release --example profile_parse`

Parse-only vs parse + blob timing across the corpus, amortised over 2000
iterations after warmup. Catches regressions in either stage independently.

```
test               parse     blob    total
-------------------------------------------
plain              0.4 us    0.1 us    0.5 us
simple             0.9 us    0.3 us    1.3 us
medium             4.7 us    1.2 us    5.9 us
heavy-inline      19.7 us    5.3 us   25.0 us
complex           23.3 us    6.9 us   30.2 us
long-doc          80.1 us   25.3 us  105.5 us
prose             25.9 us    9.7 us   35.6 us
```

Note prose vs long-doc: same size, and prose is well under half the cost —
conversational text is the cheap case since the smartypants retirement, with
feature-heavy markdown correctly the expensive one.

(Apple Silicon M-series, release profile, production options. Indicative; exact figures vary run to run.)

### `cargo run --release --example sample_parse -- prose 4`

Parse + blob in a loop under production options for a sampling profiler: the
corpus (`prose`, `complex`, `long`, `heavy`) and the seconds to run. With
[samply](https://github.com/mstange/samply), `samply record
./target/release/examples/sample_parse prose 4` gives a profile with inlined
frames resolved; build with `CARGO_PROFILE_RELEASE_DEBUG=true` first so they
can be.

### `cargo run --release --example alloc_bench`

Wraps the global allocator with counters, so every `alloc()` during a parse
is recorded. Reports total count and bytes, parse vs blob split, and a
16-bucket power-of-two histogram for the largest inputs. Confirms a change
actually cut allocations without reaching for an external profiler.

```
AstNode: 136 bytes, NodeValue: 40 bytes, Ast: 88 bytes
complex    1586 chars |  91 allocs  104 KB (67.2x input) | parse  55 blob  36 |  298 nodes
long-doc  13060 chars | 125 allocs  369 KB (29.0x input) | parse 118 blob   7 | 1058 nodes
prose     12938 chars |  94 allocs  168 KB (13.4x input) | parse  88 blob   6 |  201 nodes
```

The last line reads each site's own pages and their lookalikes through
`site::page`, which must allocate nothing:

```
site/is-own          15 links, 10 own, 13 on site |     0 allocs      0 bytes
```

These are a thread's second parse: the first also allocates the block contents
and paragraph strings the thread then keeps.

Buckets are the regression canary — a jump in `tiny` almost always means a
hidden `to_owned()` landed on the hot path. Struct sizes for `AstNode`,
`NodeValue`, and `Ast` print at the head of the output so layout
regressions in the node types surface too.

## Files

### New files (added by this fork)

```
src/
├── arena.rs                   ← custom typed Arena: doubling-then-linear growth, UnsafeCell storage,
│                                cold-path grow, documented soundness invariants, drain() for the
│                                parse entry to take back what the next parse reuses
├── blob.rs                    ← binary blob renderer: BlobWriter + visit + render_blob
├── blob/
│   ├── html.rs                ← the walk over a site's HTML + render_html_blob
│   ├── article.rs             ← the walk over a page's article + render_article_blob
│   ├── article/
│   │   ├── tree.rs            ← the page as a tree: mdream's scan, the closes HTML implies
│   │   ├── smoothie.rs        ← where the dom_smoothie files come from; title, byline, lead picture
│   │   └── smoothie/          ← grab.rs, prep.rs, helpers.rs, glob.rs, LICENSE (MIT),
│   │                            LICENSE-readability (Apache-2.0), NOTICE
│   ├── site.rs                ← what each site knows about itself: html, page, Url
│   ├── site/                  ← chan.rs, hacker_news.rs, lemmy.rs, lobsters.rs, from each site's docs
│   ├── sites.rs, span_types.rs ← the site and span type ids, generated by syncdown
│   └── html/
│       ├── mdream.rs          ← where the mdream files come from, what was cut
│       └── mdream/            ← entities.rs, entities_generated.rs, scan.rs, LICENSE (MIT)
├── benchmarks/                ← hn_comments.html and .md, ten real comments both ways
├── image_url.rs               ← is_image_url heuristic (used by blob autolink-to-image)
├── text.rs                    ← the blob text-normalization policy: collapse_whitespace,
│                                prefer_ascii, typographic_symbols
├── benchmarks.rs              ← shared bench corpus + bench_all smoke test
├── parser/
│   └── url.rs                 ← resolve_url (proxy unwrap, short URL expansion, pict-rs thumbnail,
│                                video exclusion) + extract_domain + path_ext
└── tests/
    ├── blob.rs                ← blob renderer tests (format/inline/links/images/block/
    │                            footnotes/edge/production sub-modules)
    ├── blob_article.rs        ← a page's article: extraction, the tree, the walk
    ├── blob_html.rs           ← the walk over HTML, and its parity with markdown
    ├── blob_read.rs           ← a blob read back, the one reader every blob test asserts on
    ├── image_url.rs           ← is_image_url tests
    ├── lemmy.rs               ← lemmy_mention + lemmy_spoiler tests
    ├── site.rs                ← each site's own pages, and the markdown walk's links by site
    ├── smart_parity.rs        ← the smartypants-retirement contract: invariants that must
    │                            never break + the verbatim rules for retired transforms
    ├── strip_invisible.rs     ← sanitize_input invisible-char tests
    ├── strip_leading_breaks.rs ← sanitize_input leading-break tests
    └── url.rs                 ← resolve_url tests

examples/
├── profile_parse.rs           ← parse + blob timing
├── alloc_bench.rs             ← alloc count / bytes / size histogram
└── sample_parse.rs            ← parse + blob loop for a sampling profiler
```

### Modified upstream files

Against upstream 229fc35:

```
src/
├── lib.rs                     ← expose blob/arena/benchmarks/image_url/text, parse_document_zerocopy,
│                                resolve_url; Arena is the fork's arena, StringArena alias
├── arena_tree.rs              ← append_new for freshly created block nodes
├── nodes.rs                   ← NodeValue/Ast generic over the input lifetime (Text, NodeCode.literal,
│                                NodeLink.url/.title as Cow<'a, str>), LemmySpoiler node, per-thread
│                                pools of BlockLines and paragraph strings
├── entity.rs                  ← unescape_html allocates only when an & is present
├── strings.rs                 ← is_invisible / strip_invisible / strip_leading_breaks; clean_url and
│                                clean_title borrow their input; count_newlines no-newline fast path
├── html.rs, xml.rs, cm.rs     ← LemmySpoiler output; cm::format_internal opened to the blob writer
├── scanners.re, scanners.rs   ← the open block-directive fence allows ':' in its info string
└── parser/
    ├── mod.rs                 ← lemmy_spoiler handler and finalize, sanitize_input pre-pass, mentions in
    │                            text postprocessing (Spx::col_at), the blob path (no sourcepos walks,
    │                            string arena, pools, presized arenas, borrowed-run merge), footnote
    │                            skip, first-byte block dispatch, reference early-out
    ├── inlines.rs             ← input borrowed from the string arena: text, delimiter runs, escapes,
    │                            probe text, code literals, link url/title; static delimiter text on
    │                            emphasis truncation; the mergeable-fragment flag
    ├── options.rs             ← new flags: lemmy_mention, lemmy_spoiler, sanitize_input
    └── autolink.rs            ← Lemmy user / community mentions; one '@' scan gates them and emails
```

Test-side: the fork's suites registered in `src/tests.rs`, a LemmySpoiler sourcepos
case, the fuzz option struct, and upstream tests adjusted for `comrak::Arena` and
the node lifetime.

## Performance history

### Rebase onto upstream 229fc35 (October 2026)

Upstream's v0.55 and v0.56 cycle (106 commits after the old fork's last merge,
v0.54.0) reworked the internals the fork had optimised, so a merge meant 69
conflict hunks, most of them two implementations of one idea. The fork was
rebuilt instead: upstream main, then the blob and its parser hooks ([ed9fedb]),
then each fork optimisation re-applied only where it still won on top of
upstream. Every step was measured on the host benches (production options,
median of seven runs, about 1% noise), with blob output compared byte for byte
over 1412 documents (CommonMark, GFM and cmark-gfm examples plus progit).

Upstream already has its own version of these, so the fork's is gone:

| Fork | Upstream |
|---|---|
| `Option<Box<BlockContent>>`, `Ast` 128 → 88 bytes ([8879b6f], [fdb9711]) | `Option<Box<BlockLines>>`, the same sizes (cb6a554) |
| Shared delimiter arena, cached character tables ([e55712e], [3f2090d]) | One delimiter arena and `ByteTables` per document (be66bf5) |
| Reused `spx` buffer ([8538bfa]) | `spx` built only when a detector needs it (0c0b3c0) |
| One-table `find_special_char` ([1a0bef0]) | Context-aware scan, as fast here (be16fc7) |
| `memchr` line and backtick scans ([e84ccaa]) | `memchr-n` matchers, as fast or faster here |

Re-applied or new on this base, each figure against the commit before it:

| Commit | Change | Effect |
|---|---|---|
| [6961578] | Skip `process_footnotes` when unused ([86bbe90]) | long-doc -10%, spec -11% |
| [73a4752] | Blob path skips sourcepos walks ([4fa2f0c]) | long-doc -7%, complex -7% |
| [a36fd6e] | The fork's node arena ([a9f62c0], [78b4480]) | memory -10 to -20% |
| [70696f8] | Adjacent text joins into an exact-size String ([9dda1aa]) | prose -6% |
| [5af620c] | First-byte block dispatch, reference early-out ([1a59dfb]) | long-doc -10%, complex -12% |
| [cfabaed] | `unescape_html` allocates only with an `&` ([0d89195]) | prose -3% |
| [58507cc], [1568d2d] | Node lifetime; inline text borrows a string arena ([9131489], [e3d01ef]) | heavy-inline -18%, allocations halved |
| [09f3fd8] | Static delimiter text on emphasis truncation ([14cb107]) | heavy-inline -13% |
| [397b5c8] | Thread pools, no exact-size copy on the blob path ([2dc034c]) | medium -7%, prose -7% |
| [ecd63ce] | Borrowed runs merge without a copy; borrowed probe text; pooled block literals; presized arenas | long-doc allocations 255 → 125 |
| [0c2e270] | `count_newlines` fast path; one `@` gate ([0612fe4]) | -5 to -6% |
| [c4022c3] | `add_child` appends without detach ([be8cd6a]) | heavy-inline -4% |

Measured and dropped: skipping text coalescing ([a2680c9]) and widening text at
creation ([03f4511]) are worth about nothing now, and skipping coalescing
half-linked emails after `_` or `.` (the GFM example `a.b-c_d@a.b` linked only
`d@a.b`), so coalescing stays and those two documents now render differently
from the old fork. `memchr` in place of upstream's backtick matcher was slower.

Against the old fork: allocations at or below it on every corpus (long-doc
125 vs 134, prose 94 vs 98), parse prose -12% and progit -9%, spec examples
level, long-doc and medium +7%, heavy-inline and complex +10%. The remaining gap
is upstream's inline parser with no separable item. The rebase also brings
upstream's pathological-input fixes and the iterative email autolinker
(GHSA-xg9p-p4jc-c46g). `process_lemmy_mentions` is still recursive in the same
shape and should follow.

Then the blob renderer, each figure render-only against the commit before it.
Every commit's blob output is byte-identical to the one before it, on the
corpus and on 626 generated documents of nested quotes, lists, emphasis,
links and footnotes:

| Commit | Change | Effect |
|---|---|---|
| [77105cf] | `collapse_whitespace` reuses one double-space finder | prose -5%, complex -3% |
| [45fb812] | Link hosts end at a byte `memchr` | prose -10% |
| [3ee03e2] | The whitespace-paragraph check stops at the first visible character | prose -4% |
| [718160c] | Spans sort by insertion, with a budget | long-doc -15%, complex -13%, medium -8% |
| [443a6fd] | The walk keeps its own stack, so deep nesting no longer aborts | heavy-inline -6%, long-doc +5%, complex +3% |

Old fork, rebase ([1faaa47]) and blob work ([443a6fd]), run back to back,
median of seven, microseconds:

| Corpus | Parse + render | Render |
|---|---|---|
| prose | 42.1 → 35.6 → 34.6 (-18%) | 12.3 → 9.5 → 8.3 (-33%) |
| long-doc | 99.9 → 104.9 → 101.9 (+2%) | 25.2 → 25.0 → 21.5 (-15%) |
| complex | 28.2 → 30.1 → 29.5 (+5%) | 7.0 → 6.8 → 6.1 (-13%) |
| heavy-inline | 23.4 → 25.1 → 25.7 (+10%) | 5.1 → 5.0 → 4.7 (-8%) |
| medium | 5.7 → 5.9 → 5.9 (+4%) | 1.2 → 1.2 → 1.1 (-8%) |

The blob work adds no allocations. syncdown's device suite passes on
[443a6fd] (Pixel 9 Pro XL).

### Parse squeeze (October 2026)

A sampled profile of old fork against [443a6fd] on heavy-inline, diffed per
inlined function, found two concentrated costs rather than a spread. Each
figure is parse-only against the commit before it:

| Commit | Change | Effect |
|---|---|---|
| [6ef2dfb] | Plain text nodes are built in the caller's frame | heavy-inline -6% |
| [0bda756] | Text runs without an `@` skip the mention and email detectors | heavy-inline -5%, complex -2%, medium -2% |
| [710d6a9] | `find_special_char` scans with `position` between candidates | prose -2% |

`make_inline` had the same machine code and call count in both trees but took
twice the time: its caller writes the node value with 8-byte stores and it
copies it with 16-byte loads, which the store buffer cannot forward. Only the
plain-text site takes the inlined copy, since inlining every caller cost
complex and medium 1-2%. Upstream's `spx` gating (0c0b3c0, above) still built
the sourcepos list for every text node when autolink or mentions were on, then
moved it into detectors that return without an `@`; the sizing walk now checks
for one first. `find_special_char` keeps upstream's context check, only the
scan between candidates changed.

Interleaved against [443a6fd], median of seven: parse heavy-inline -12%,
prose -3%, complex, long-doc and medium -2%, spec and progit level. Render is
untouched but reads +1-4% from code layout. Totals against the old fork:

| Corpus | Old fork | Now |
|---|---|---|
| prose | 42.1 | 33.8 (-20%) |
| long-doc | 99.9 | 99.7 (level) |
| complex | 28.2 | 28.9 (+2%) |
| heavy-inline | 23.4 | 23.1 (-1%) |
| medium | 5.7 | 5.8 (+2%) |

Blob output is byte-identical on the corpus and the nested corpus, allocations
are unchanged, and syncdown's device suite passes on [710d6a9]. Measured and
dropped: a larger node arena presize (heavy-inline +9%) and a thread-local pool
reusing arena chunks across parses (±1-2%). What remains of the complex and
long-doc parse gap is diffuse (`open_new_blocks`, `process_emphasis`,
`merge_borrowed_text_runs`).

### Before the rebase

The old fork's throughput and allocation work (tagged `pre-rebase-0.56`),
grouped by theme. Each change was benchmarked against the shared corpus before
landing; the figures are from the commit messages (device wins measured on a
Pixel 9 Pro XL).

- **Smartypants retirement** — precompiled `resolve_url` gate searchers ([463834a], blob render -18% on link-heavy docs), retire `parse.smart` with symbols relocated to the blob writer ([eb5580e], prose preprocess 2.27x, blob allocs 686 → 6, AST nodes 4.2x fewer), delete the fork's smart handlers and lookahead tables ([0bb0a38], -129 lines of divergence; ends the hyphenated-word fragmentation the [18276c3] union tables caused), saturating profile_parse subtraction ([836a992]).
- **Zero-copy / fewer allocations** — pooled zero-copy text nodes ([e3d01ef], 51% fewer allocs), `Cow` for `NodeCode.literal` + `NodeLink.url`/`.title` with lazy cleaning ([32fbb41]), `Cow::to_mut()` avoidance on emphasis delimiters ([14cb107], 30% fewer allocs), static strings for smart-punct runs ([b0d366d]), lazy `unescape_html` ([0d89195]), reused per-text `VecDeque` ([8538bfa], 34% fewer allocs), pre-sized join buffer ([9dda1aa]), zero-copy `extract_domain` ([cb91390]).
- **Node & struct layout** — `Ast` 128 → 88 bytes via `Option<Box<BlockContent>>` ([8879b6f]), `SmallVec<[usize; 4]>` line offsets ([fdb9711]).
- **Arena** — input-sized presizing ([9d7fbf5], [6e9d38a]), right-sized then shared per-paragraph delimiter arenas ([86dcf06], [e55712e]), custom arena with doubling-then-linear growth ([a9f62c0], 19% less memory), `UnsafeCell` over `RefCell` ([78b4480]), detach-free `append_new` ([be8cd6a]).
- **Parse hot path** — smart-punct lookahead in `find_special_char` ([18276c3], since removed with the retirement), cached character lookup tables ([3f2090d]), `memchr` for backtick/line scanning ([e84ccaa]) and inline detectors ([312d49c]), fast-path block dispatch + early-out reference resolution ([1a59dfb]).
- **Skipping whole subsystems** — `parse_document_raw` to skip postprocessing ([a2680c9], since folded into the scoped entry), skip `process_footnotes` when none present ([86bbe90], up to 50%), skip / reuse the `fix_zero_end_columns` pass ([4fa2f0c], [c3eda59]).
- **Blob renderer** — packed u64 span sort key ([6cfb246]), pre-sized span/url buffers ([cec39b5], 63% fewer blob allocs), pure-ASCII fast path ([308f3d0]), flags packed into header byte 7 ([021dc48]), typographic-to-ASCII normalization ([bef9d53]), `0x01` replacement-span anchor ([ce4d33a]), no leading newline for a top-of-doc image ([d4d8d85]), faster blob parse path ([f13f7fe]), resolve each emitted URL exactly once via a `ResolvedUrl` newtype ([b0b8def]), table cells written straight into the url section ([18c8e82], blob allocs on a table 63 → 36), the text view unchecked since only `&str` is ever written ([351337f]).
- **Sampled profile, October 2026** — the first pass with a sampling profiler ([61ae442]) on production options, which the benches had never used ([7a60416]). `sanitize_input`'s invisible-character scan was 30% of a prose parse and cannot match ASCII, so ASCII input passes in one SIMD check ([a5df793]; prose 67 → 44 us with the next); the inline scanner's dead smart-pair lookup was a second table load per byte, so without `parse.smart` it is one `position` over one table ([1a0bef0], the scanner's share halved); block contents and paragraph strings go back to the thread for the next parse ([2dc034c], on [d6aee3a] and [a713510]; complex 227 → 91 allocs, long-doc 362 → 134, complex 29.9 → 27.9 us). Measured and declined: keeping the node arena's buffer itself across parses, neutral on macOS and on Android (Samsung S24 FE, both inside noise), so it is not kept. Device parse: prose 97 → 71 us, complex 60 → 50.

<!-- Commit links — fork is github.com/laurencedawson/comrak -->
[463834a]: https://github.com/laurencedawson/comrak/commit/463834a
[836a992]: https://github.com/laurencedawson/comrak/commit/836a992
[eb5580e]: https://github.com/laurencedawson/comrak/commit/eb5580e
[0bb0a38]: https://github.com/laurencedawson/comrak/commit/0bb0a38
[f0bf9d9]: https://github.com/laurencedawson/comrak/commit/f0bf9d9
[a676324]: https://github.com/laurencedawson/comrak/commit/a676324
[82663af]: https://github.com/laurencedawson/comrak/commit/82663af
[ea8f274]: https://github.com/laurencedawson/comrak/commit/ea8f274
[f753071]: https://github.com/laurencedawson/comrak/commit/f753071
[24cd0cc]: https://github.com/laurencedawson/comrak/commit/24cd0cc
[7c60125]: https://github.com/laurencedawson/comrak/commit/7c60125
[79f72b4]: https://github.com/laurencedawson/comrak/commit/79f72b4
[b0b8def]: https://github.com/laurencedawson/comrak/commit/b0b8def
[e3d01ef]: https://github.com/laurencedawson/comrak/commit/e3d01ef
[32fbb41]: https://github.com/laurencedawson/comrak/commit/32fbb41
[14cb107]: https://github.com/laurencedawson/comrak/commit/14cb107
[b0d366d]: https://github.com/laurencedawson/comrak/commit/b0d366d
[0d89195]: https://github.com/laurencedawson/comrak/commit/0d89195
[8538bfa]: https://github.com/laurencedawson/comrak/commit/8538bfa
[9dda1aa]: https://github.com/laurencedawson/comrak/commit/9dda1aa
[cb91390]: https://github.com/laurencedawson/comrak/commit/cb91390
[8879b6f]: https://github.com/laurencedawson/comrak/commit/8879b6f
[fdb9711]: https://github.com/laurencedawson/comrak/commit/fdb9711
[9d7fbf5]: https://github.com/laurencedawson/comrak/commit/9d7fbf5
[6e9d38a]: https://github.com/laurencedawson/comrak/commit/6e9d38a
[86dcf06]: https://github.com/laurencedawson/comrak/commit/86dcf06
[e55712e]: https://github.com/laurencedawson/comrak/commit/e55712e
[a9f62c0]: https://github.com/laurencedawson/comrak/commit/a9f62c0
[78b4480]: https://github.com/laurencedawson/comrak/commit/78b4480
[be8cd6a]: https://github.com/laurencedawson/comrak/commit/be8cd6a
[18276c3]: https://github.com/laurencedawson/comrak/commit/18276c3
[3f2090d]: https://github.com/laurencedawson/comrak/commit/3f2090d
[e84ccaa]: https://github.com/laurencedawson/comrak/commit/e84ccaa
[312d49c]: https://github.com/laurencedawson/comrak/commit/312d49c
[1a59dfb]: https://github.com/laurencedawson/comrak/commit/1a59dfb
[a2680c9]: https://github.com/laurencedawson/comrak/commit/a2680c9
[86bbe90]: https://github.com/laurencedawson/comrak/commit/86bbe90
[4fa2f0c]: https://github.com/laurencedawson/comrak/commit/4fa2f0c
[c3eda59]: https://github.com/laurencedawson/comrak/commit/c3eda59
[6cfb246]: https://github.com/laurencedawson/comrak/commit/6cfb246
[cec39b5]: https://github.com/laurencedawson/comrak/commit/cec39b5
[308f3d0]: https://github.com/laurencedawson/comrak/commit/308f3d0
[021dc48]: https://github.com/laurencedawson/comrak/commit/021dc48
[bef9d53]: https://github.com/laurencedawson/comrak/commit/bef9d53
[ce4d33a]: https://github.com/laurencedawson/comrak/commit/ce4d33a
[d4d8d85]: https://github.com/laurencedawson/comrak/commit/d4d8d85
[f13f7fe]: https://github.com/laurencedawson/comrak/commit/f13f7fe
[a5df793]: https://github.com/laurencedawson/comrak/commit/a5df793
[1a0bef0]: https://github.com/laurencedawson/comrak/commit/1a0bef0
[18c8e82]: https://github.com/laurencedawson/comrak/commit/18c8e82
[351337f]: https://github.com/laurencedawson/comrak/commit/351337f
[d6aee3a]: https://github.com/laurencedawson/comrak/commit/d6aee3a
[a713510]: https://github.com/laurencedawson/comrak/commit/a713510
[2dc034c]: https://github.com/laurencedawson/comrak/commit/2dc034c
[7a60416]: https://github.com/laurencedawson/comrak/commit/7a60416
[61ae442]: https://github.com/laurencedawson/comrak/commit/61ae442
[1b09a35]: https://github.com/laurencedawson/comrak/commit/1b09a35
[03f4511]: https://github.com/laurencedawson/comrak/commit/03f4511
[0612fe4]: https://github.com/laurencedawson/comrak/commit/0612fe4
[9131489]: https://github.com/laurencedawson/comrak/commit/9131489
[ed9fedb]: https://github.com/laurencedawson/comrak/commit/ed9fedb
[6961578]: https://github.com/laurencedawson/comrak/commit/6961578
[73a4752]: https://github.com/laurencedawson/comrak/commit/73a4752
[a36fd6e]: https://github.com/laurencedawson/comrak/commit/a36fd6e
[70696f8]: https://github.com/laurencedawson/comrak/commit/70696f8
[5af620c]: https://github.com/laurencedawson/comrak/commit/5af620c
[cfabaed]: https://github.com/laurencedawson/comrak/commit/cfabaed
[58507cc]: https://github.com/laurencedawson/comrak/commit/58507cc
[1568d2d]: https://github.com/laurencedawson/comrak/commit/1568d2d
[09f3fd8]: https://github.com/laurencedawson/comrak/commit/09f3fd8
[397b5c8]: https://github.com/laurencedawson/comrak/commit/397b5c8
[ecd63ce]: https://github.com/laurencedawson/comrak/commit/ecd63ce
[0c2e270]: https://github.com/laurencedawson/comrak/commit/0c2e270
[c4022c3]: https://github.com/laurencedawson/comrak/commit/c4022c3
[1faaa47]: https://github.com/laurencedawson/comrak/commit/1faaa47
[77105cf]: https://github.com/laurencedawson/comrak/commit/77105cf
[45fb812]: https://github.com/laurencedawson/comrak/commit/45fb812
[3ee03e2]: https://github.com/laurencedawson/comrak/commit/3ee03e2
[718160c]: https://github.com/laurencedawson/comrak/commit/718160c
[443a6fd]: https://github.com/laurencedawson/comrak/commit/443a6fd
[6ef2dfb]: https://github.com/laurencedawson/comrak/commit/6ef2dfb
[0bda756]: https://github.com/laurencedawson/comrak/commit/0bda756
[710d6a9]: https://github.com/laurencedawson/comrak/commit/710d6a9
[b6d9a01]: https://github.com/laurencedawson/comrak/commit/b6d9a01
[2f00ef6]: https://github.com/laurencedawson/comrak/commit/2f00ef6
[b2f39fe]: https://github.com/laurencedawson/comrak/commit/b2f39fe
[c392115]: https://github.com/laurencedawson/comrak/commit/c392115
[bca01f9]: https://github.com/laurencedawson/comrak/commit/bca01f9
[29b28a5]: https://github.com/laurencedawson/comrak/commit/29b28a5
[50226a9]: https://github.com/laurencedawson/comrak/commit/50226a9
[2090141]: https://github.com/laurencedawson/comrak/commit/2090141
[10e87cf]: https://github.com/laurencedawson/comrak/commit/10e87cf
[df385a7]: https://github.com/laurencedawson/comrak/commit/df385a7
[3edc676]: https://github.com/laurencedawson/comrak/commit/3edc676
[5fc6ff3]: https://github.com/laurencedawson/comrak/commit/5fc6ff3
[b381692]: https://github.com/laurencedawson/comrak/commit/b381692
[736f5cd]: https://github.com/laurencedawson/comrak/commit/736f5cd
[a721133]: https://github.com/laurencedawson/comrak/commit/a721133
[89a23ad]: https://github.com/laurencedawson/comrak/commit/89a23ad
[e277031]: https://github.com/laurencedawson/comrak/commit/e277031
[94bb2f6]: https://github.com/laurencedawson/comrak/commit/94bb2f6
[d0c39d5]: https://github.com/laurencedawson/comrak/commit/d0c39d5
[6158cdd]: https://github.com/laurencedawson/comrak/commit/6158cdd
[ce02d6f]: https://github.com/laurencedawson/comrak/commit/ce02d6f
