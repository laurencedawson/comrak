# Fork Changes

A [comrak](https://github.com/kivikakk/comrak) fork tailored for
[syncdown](https://github.com/laurencedawson/syncdown), an Android
markdown renderer. Highlights:

| Area | What |
|---|---|
| [Extensions](#extensions) | Lemmy mentions + spoiler directive, invisible-character stripping, smart-punctuation additions (`©`, `®`, `™`, `±`, cap-repetition), `NodeLink::cleaned_url()` |
| [Blob renderer](#blob-rendering) | One-pass AST → compact binary blob feeding syncdown's `FastSpannable`, zero intermediate allocations |
| [Parse entry](#parse-entry-point) | Single scoped `parse_document_zerocopy` — arenas owned internally, AST dropped with the closure |
| [Utility helpers](#utility-modules-used-by-the-renderer) | `clean_url`, `extract_domain`, `is_image_url`, `collapse_whitespace` — public, zero-copy on common paths |
| [Benchmarking](#benchmarking) | `profile_parse` + `alloc_bench` + shared corpus for tracking throughput and allocation pressure |
| Alloc / throughput (throughout) | Zero-copy text nodes via a pooled string arena, `SmallVec` line offsets, `Ast` 128 → 88 bytes, static smart-punct tables, `Cow::to_mut` avoidance, `memchr` fast paths, custom `UnsafeCell` arena with `#[cold]` grow path |

## Extensions


### `lemmy_mention`

Converts Lemmy user and community mentions to links. Context-aware, won't match inside code blocks, code spans, or existing links.

- `@user@instance.com` becomes a link to `https://instance.com/u/user`
- `!community@instance.com` becomes a link to `https://instance.com/c/community`

Name validation follows Lemmy's rules: 3-20 characters, `[a-zA-Z0-9_]`. Domains allow `.`, `-`, `_`, and `:` (for ports).

### `lemmy_spoiler`

Parses Lemmy spoiler blocks using the `:::spoiler` directive. Only matches `:::spoiler`, other `:::` directives are ignored.

```markdown
:::spoiler Click to reveal
Hidden content with **markdown** support
:::
```

Renders as `<details>/<summary>` in HTML.

### `strip_invisible`

Strips invisible Unicode characters before parsing. Zero-copy when input is clean. Preserves ZWJ and VS16 for emoji.

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

### `parse.smart` additions

Symbol replacements and punctuation capping added to the existing smart punctuation feature.

| Input | Output | Description |
|-------|--------|-------------|
| `(c)` | &copy; | Copyright (case-insensitive) |
| `(r)` | &reg; | Registered (case-insensitive) |
| `(tm)` | &trade; | Trademark (case-insensitive) |
| `+-` | &plusmn; | Plus-minus |
| `????` | `???` | Cap repeated `?` at 3 |
| `!!!!` | `!!!` | Cap repeated `!` at 3 |
| `,,` | `,` | Cap repeated `,` at 1 |

### `NodeLink::cleaned_url()`

On-demand URL cleaning via `NodeLink::cleaned_url()`. Returns `Cow::Borrowed` when no cleaning is needed (zero-copy). The AST stores the original URL; consumers call `cleaned_url()` when they want the cleaned version.

Also available as a standalone function: `comrak::clean_url(&str)`.

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

## Blob rendering

A compact binary representation of a parsed markdown document. The walker
visits the AST once and appends text + span metadata directly to a single
buffer, with zero intermediate allocations. The layout is optimised for
cheap decoding on the consumer side — integer offsets, UTF-16 positions,
sorted spans.

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
| `comrak::clean_url(&str)` | `Cow<'_, str>` | Re-export of `comrak::parser::url::clean_url`. On-demand URL cleaning (proxy unwrap, short-URL expansion). See [`NodeLink::cleaned_url()`](#nodelinkcleaned_url) for the full pattern table. |
| `comrak::parser::url::extract_domain(&str)` | `Option<Cow<'_, str>>` | Host extraction for display-suffix dedup. Strips `www.`; zero-copy when the host is already lowercase ASCII. |
| `comrak::image_url::is_image_url(&str)` | `bool` | Heuristic URL → image detection (by known host, path pattern, or file extension). |
| `comrak::text::collapse_whitespace(&str)` | `Cow<'_, str>` | Collapses runs of spaces to a single space. Zero-copy when the input has no double-space. |

### Render entry point

`comrak::blob::render_blob(root, input) -> Option<Vec<u8>>` — renders the
AST at `root` against the original source `input`. Returns `None` when the
document has no spans and the output text equals the input (the caller can
then use the raw input string directly and skip allocating a blob).

### Blob layout

All multi-byte integers are little-endian i32. All text positions are
UTF-16 code units (matching Java `String` indexing directly).

```
┌───────────────────┬───────────────────┬─────────┬─────────┬─────────┬──────────┐
│  text_len (4 B)   │ span_count (4 B)  │  text   │   pad   │  spans  │ url_data │
├───────────────────┼───────────────────┼─────────┼─────────┼─────────┼──────────┤
│ bytes of text     │ number of spans   │ UTF-8   │ 0..3 B  │ 16 B    │ raw URL  │
│ (header, excl.)   │                   │ source  │ to 4B   │ per     │ bytes    │
│                   │                   │         │ align   │ span    │ (indexed │
│                   │                   │         │         │         │ by spans)│
└───────────────────┴───────────────────┴─────────┴─────────┴─────────┴──────────┘
```

Byte offsets, in order:

- `0..4` — `text_len` (i32)
- `4..8` — `span_count` (i32)
- `8..8 + text_len` — text bytes (UTF-8)
- next — zero padding to align to a 4-byte boundary (0..3 bytes)
- next — `span_count × 16` bytes of packed span records
- next — URL bytes referenced by `LINK`/`IMAGE`/`LINK_SIZE` spans (rest of blob)

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

**URL-carrying spans** (`LINK`, `IMAGE`, `LINK_SIZE`) — offset + length into
the trailing `url_data` section:

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
  carrying spans). Longer URLs are silently truncated.
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
and `long_doc()` (a realistic ~13 KB post). Both examples below consume
this module, so adding a new input shape lands in both reports at once.
Also fed to a `bench_all` smoke test that runs under `cargo test`.

### `cargo run --release --example profile_parse`

Parse-only vs parse + blob timing across the corpus, amortised over 2000
iterations after warmup. Catches regressions in either stage independently.

```
test               parse     blob    total
-------------------------------------------
plain              0.8 us    0.1 us    0.9 us
simple             1.1 us    0.2 us    1.3 us
medium             7.1 us    1.1 us    8.3 us
heavy-inline      28.8 us    3.5 us   32.2 us
complex           33.0 us    6.1 us   39.1 us
long-doc         134.7 us   27.2 us  162.0 us
```

(Apple Silicon M-series, release profile.)

### `cargo run --release --example alloc_bench`

Wraps the global allocator with counters, so every `alloc()` during a parse
is recorded. Reports total count and bytes, parse vs blob split, a
tiny/small/medium/large bucket summary, and a 16-bucket power-of-two
histogram for the largest inputs. Confirms a change actually cut
allocations without reaching for an external profiler.

```
long-doc  13060 chars | 1720 allocs  493 KB (38.7x input) | parse 1701 blob 19 | 1058 nodes
  buckets: tiny(1-32)=771 small(33-128)=288 medium(129-1K)=644 large(1K+)=17
  histogram: 1-2=250 9-16=221 17-32=166 33-64=134 65-128=75 …
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
├── text.rs                    ← collapse_whitespace helper
├── benchmarks.rs              ← shared bench corpus + bench_all smoke test
├── parser/
│   └── url.rs                 ← clean_url (proxy unwrap, short URL expansion) + extract_domain
└── tests/
    ├── blob.rs                ← blob renderer tests (90 across 7 sub-modules)
    ├── image_url.rs           ← is_image_url tests
    └── url.rs                 ← clean_url tests

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
│                                line_offsets: SmallVec<[usize; 4]>, NodeLink::cleaned_url()
├── entity.rs                  ← unescape_html memchr fast path, lazy allocation
├── strings.rs                 ← strip_invisible() + zero-copy trim/normalize Cow variants
└── parser/
    ├── mod.rs                 ← fork extensions wired in (lemmy_spoiler handler, strip_invisible
    │                            preprocess pass) + optimisations (fast-path block dispatch, shared
    │                            delimiter arena, skip process_footnotes / fix_zero_end_columns when
    │                            unused, pre-sized arenas)
    ├── inlines.rs             ← zero-copy text via StringArena, static-string smart punctuation,
    │                            Cow::to_mut() avoidance on emphasis delimiters, memchr fast paths
    ├── options.rs             ← new flags: lemmy_mention, lemmy_spoiler, strip_invisible,
    │                            smart-punctuation symbol replacements
    └── autolink.rs            ← Lemmy user / community mention parsing
```

