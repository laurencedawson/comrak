# Fork Changes

Extensions added to this [comrak](https://github.com/kivikakk/comrak) fork.

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
