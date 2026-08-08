# Fork Changes

A [comrak](https://github.com/kivikakk/comrak) fork tailored for
[syncdown](https://github.com/laurencedawson/syncdown), an Android
markdown renderer. Highlights:

| Area | What |
|---|---|
| [Extensions](#extensions) | Lemmy mentions + spoiler directive, `sanitize_input` (invisible-char + leading-hard-break stripping). Smartypants is retired — see [below](#retired-parsesmart-and-the-forks-smart-additions) |
| [URL resolution](#url-resolution) | `resolve_url()` — proxy/redirect unwrap, pict-rs thumbnail rewrite, video exclusion; runs once at the blob's single emission chokepoint |
| [Blob renderer](#blob-rendering) | One-pass AST → compact binary blob feeding syncdown's `FastSpannable`; streams straight into pre-sized buffers with no intermediate representation |
| [Text normalization](#text-normalization) | The blob's rendering policy, flag-free by design: collapse spaces, typed typographic Unicode → ASCII, `(c)/(r)/(tm)/+-` → ©®™± |
| [Parse entry](#parse-entry-point) | Single scoped `parse_document_zerocopy` — arenas owned internally, AST dropped with the closure |
| [Utility helpers](#utility-modules-used-by-the-renderer) | `resolve_url`, `extract_domain`, `is_image_url`, and the `text` normalizers — public, zero-copy on common paths |
| [Benchmarking](#benchmarking) | `profile_parse` + `alloc_bench` + shared corpus (including contraction-dense `prose`) for tracking throughput and allocation pressure |
| Alloc / throughput (throughout) | Zero-copy text nodes via a pooled string arena, `SmallVec` line offsets, slimmed `Ast`, `Cow::to_mut` avoidance, `memchr` fast paths, custom `UnsafeCell` arena with `#[cold]` grow path |

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
| YouTube short | `youtu.be/<id>` | `youtube.com/watch?v=<id>` |
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
pools paragraph strings and lets text nodes borrow from `md` directly, cutting
per-node allocations. The closure receives the root; the arenas drop on return
so the borrow can't escape.

```rust
let blob = comrak::parse_document_zerocopy(md, &opts, |root| {
    comrak::blob::render_blob(root, md)
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

`comrak::blob::render_blob(root, input) -> Option<Vec<u8>>` — renders the
AST at `root` against the original source `input`. Returns `None` when the
document has no spans and the output text equals the input (the caller can
then use the raw input string directly and skip allocating a blob).

### Standalone use (without the fork's parse-side changes)

The blob renderer is decoupled from how the AST is built. `render_blob` walks a
finished tree through comrak's public node API (`node.children()`,
`node.data().value`) and never inspects the parser, so it runs against a stock
parse just as well as the optimized one:

```rust
let arena = comrak::Arena::new();
let root = comrak::parse_document(&arena, md, &options); // vanilla entry, no zero-copy pooling
let blob = comrak::blob::render_blob(root, md);          // identical bytes to the zerocopy path
```

[`parse_document_zerocopy`](#parse-entry-point) and the 27 allocation/throughput
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
  - bit 1: `NEEDS_REFLOW` — at least one `IMAGE` or `LEMMY_SPOILER_TITLE` span exists
  - bit 2: `HAS_SPOILER_BODY` — at least one `LEMMY_SPOILER_CONTENT` span exists ([ea8f274])
  - bits 3-7: reserved (zero)
- `8..8 + text_len` — text bytes (UTF-8)
- next — zero padding to align to a 4-byte boundary (0..3 bytes)
- next — `span_count × 16` bytes of packed span records
- next — URL bytes referenced by `LINK`/`IMAGE` spans (rest of blob)

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

**URL-carrying spans** (`LINK`, `IMAGE`) — offset + length into the trailing
`url_data` section:

```
bit:  31                                                         12 11              0
     ┌─────────────────────────────────────────────────────────────┬──────────────────┐
     │              offset  (20 bits, up to 1 MiB)                 │ length (12 bits) │
     └─────────────────────────────────────────────────────────────┴──────────────────┘
```

**List items** (`LIST_ITEM`) — nesting indent + list number / task-list state:

```
bit:  31                      16 15                               0
     ┌────────────────────────────┬────────────────────────────────┐
     │    indent (16-bit)         │        number (16-bit)         │
     └────────────────────────────┴────────────────────────────────┘
```

`number`:
- `0` for bullet lists
- `1..n` for ordered lists
- `0xFFFE` for unchecked task list items
- `0xFFFF` for checked task list items

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
  trailing newlines so consecutive images don't stack padding.
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


## Benchmarking

Two examples and a shared input corpus for tracking parse throughput and
allocation pressure across changes.

> **First-time builds.** `src/blob/span_types.rs` is generated by syncdown's
> Gradle `generateSpanTypes` task and gitignored here. Run
> `./gradlew :syncdown:generateSpanTypes` from the syncdown repo once before
> running `cargo` commands in isolation — or copy a previously generated
> file.

### `src/benchmarks.rs`

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
plain              0.5 us    0.1 us    0.6 us
simple             0.8 us    0.3 us    1.1 us
medium             4.3 us    1.1 us    5.5 us
heavy-inline      15.6 us    4.9 us   20.5 us
complex           21.2 us    7.1 us   28.2 us
long-doc          75.0 us   24.9 us  100.0 us
prose             43.7 us   10.8 us   54.5 us
```

Note prose vs long-doc: same size, and prose is now roughly half the cost —
conversational text is the cheap case since the smartypants retirement, with
feature-heavy markdown correctly the expensive one.

(Apple Silicon M-series, release profile. Indicative; exact figures vary run to run.)

### `cargo run --release --example alloc_bench`

Wraps the global allocator with counters, so every `alloc()` during a parse
is recorded. Reports total count and bytes, parse vs blob split, a
tiny/small/medium/large bucket summary, and a 16-bucket power-of-two
histogram for the largest inputs. Confirms a change actually cut
allocations without reaching for an external profiler.

```
AstNode: 144 bytes, NodeValue: 40 bytes, Ast: 96 bytes
long-doc  13060 chars | 362 allocs  399 KB (31.4x input) | parse 355 blob   7 | 1058 nodes
prose     12938 chars | 138 allocs  174 KB (13.8x input) | parse 132 blob   6 |  361 nodes
  buckets: tiny(1-32)=93 small(33-128)=200 medium(129-1K)=56 large(1K+)=13
```

Buckets are the regression canary — a jump in `tiny` almost always means a
hidden `to_owned()` landed on the hot path. Struct sizes for `AstNode`,
`NodeValue`, and `Ast` print at the head of the output so layout
regressions in the node types surface too.

## Files

### New files (added by this fork)

```
src/
├── arena.rs                   ← custom typed Arena: doubling-then-linear growth, UnsafeCell storage,
│                                cold-path grow, documented soundness invariants
├── blob.rs                    ← binary blob renderer: BlobWriter + visit + render_blob
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
    ├── image_url.rs           ← is_image_url tests
    ├── lemmy.rs               ← lemmy_mention + lemmy_spoiler tests
    ├── smart_parity.rs        ← the smartypants-retirement contract: invariants that must
    │                            never break + the verbatim rules for retired transforms
    ├── strip_invisible.rs     ← sanitize_input invisible-char tests
    ├── strip_leading_breaks.rs ← sanitize_input leading-break tests
    └── url.rs                 ← resolve_url tests

examples/
├── profile_parse.rs           ← parse + blob timing
└── alloc_bench.rs             ← alloc count / bytes / bucket histogram
```

`src/blob/span_types.rs` is generated externally (see Benchmarking note
above) and not tracked in this repo.

### Modified upstream files

```
src/
├── lib.rs                     ← expose blob/arena/benchmarks/image_url/text,
│                                parse_document_zerocopy, Arena/StringArena aliases,
│                                arena_capacities() helper
├── nodes.rs                   ← Ast 128 → 88 bytes (BlockContent to Option<Box<_>>),
│                                line_offsets: SmallVec<[usize; 4]>, NodeLink/NodeCode String → Cow
├── entity.rs                  ← unescape_html memchr fast path, lazy allocation
├── html.rs                    ← render_lemmy_spoiler emits <details>/<summary>
├── strings.rs                 ← strip_invisible() + zero-copy trim/normalize Cow variants
└── parser/
    ├── mod.rs                 ← fork extensions wired in (lemmy_spoiler handler, sanitize_input
    │                            preprocess pass) + optimisations (fast-path block dispatch, shared
    │                            delimiter arena, skip process_footnotes / fix_zero_end_columns when
    │                            unused, pre-sized arenas)
    ├── inlines.rs             ← zero-copy text via StringArena, Cow::to_mut() avoidance on emphasis
    │                            delimiters, memchr fast paths (the fork's smart-punctuation additions
    │                            were removed with the smartypants retirement)
    ├── options.rs             ← new flags: lemmy_mention, lemmy_spoiler, sanitize_input
    └── autolink.rs            ← Lemmy user / community mention parsing
```

## Performance history

The fork's throughput and allocation work, grouped by theme. Each change was
benchmarked against the shared corpus before landing; the figures are from the
commit messages (device wins measured on a Pixel 9 Pro XL).

- **Smartypants retirement** — precompiled `resolve_url` gate searchers ([463834a], blob render -18% on link-heavy docs), retire `parse.smart` with symbols relocated to the blob writer ([eb5580e], prose preprocess 2.27x, blob allocs 686 → 6, AST nodes 4.2x fewer), delete the fork's smart handlers and lookahead tables ([0bb0a38], -129 lines of divergence; ends the hyphenated-word fragmentation the [18276c3] union tables caused), saturating profile_parse subtraction ([836a992]).
- **Zero-copy / fewer allocations** — pooled zero-copy text nodes ([e3d01ef], 51% fewer allocs), `Cow` for `NodeCode.literal` + `NodeLink.url`/`.title` with lazy cleaning ([32fbb41]), `Cow::to_mut()` avoidance on emphasis delimiters ([14cb107], 30% fewer allocs), static strings for smart-punct runs ([b0d366d]), lazy `unescape_html` ([0d89195]), reused per-text `VecDeque` ([8538bfa], 34% fewer allocs), pre-sized join buffer ([9dda1aa]), zero-copy `extract_domain` ([cb91390]).
- **Node & struct layout** — `Ast` 128 → 88 bytes via `Option<Box<BlockContent>>` ([8879b6f]), `SmallVec<[usize; 4]>` line offsets ([fdb9711]).
- **Arena** — input-sized presizing ([9d7fbf5], [6e9d38a]), right-sized then shared per-paragraph delimiter arenas ([86dcf06], [e55712e]), custom arena with doubling-then-linear growth ([a9f62c0], 19% less memory), `UnsafeCell` over `RefCell` ([78b4480]), detach-free `append_new` ([be8cd6a]).
- **Parse hot path** — smart-punct lookahead in `find_special_char` ([18276c3], since removed with the retirement), cached character lookup tables ([3f2090d]), `memchr` for backtick/line scanning ([e84ccaa]) and inline detectors ([312d49c]), fast-path block dispatch + early-out reference resolution ([1a59dfb]).
- **Skipping whole subsystems** — `parse_document_raw` to skip postprocessing ([a2680c9], since folded into the scoped entry), skip `process_footnotes` when none present ([86bbe90], up to 50%), skip / reuse the `fix_zero_end_columns` pass ([4fa2f0c], [c3eda59]).
- **Blob renderer** — packed u64 span sort key ([6cfb246]), pre-sized span/url buffers ([cec39b5], 63% fewer blob allocs), pure-ASCII fast path ([308f3d0]), flags packed into header byte 7 ([021dc48]), typographic-to-ASCII normalization ([bef9d53]), `0x01` replacement-span anchor ([ce4d33a]), no leading newline for a top-of-doc image ([d4d8d85]), faster blob parse path ([f13f7fe]), resolve each emitted URL exactly once via a `ResolvedUrl` newtype ([b0b8def]).

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

