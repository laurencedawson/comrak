//! Benchmark test data and helpers for blob rendering.
//! Mirrors the Android instrumentation test suite in MarkdownBenchmarkTest.java.

/// The options the host app parses with, so every bench and profile sees production's cost.
/// Mirrors syncdown's `production_options` in its JNI shim; keep the two in step.
pub fn production_options() -> crate::Options<'static> {
    let mut opts = crate::Options::default();
    opts.extension.strikethrough = true;
    opts.extension.table = true;
    opts.extension.autolink = true;
    opts.extension.superscript = true;
    opts.extension.subscript = true;
    opts.extension.footnotes = true;
    opts.extension.lemmy_mention = true;
    opts.extension.lemmy_spoiler = true;
    opts.parse.sanitize_input = true;
    opts.parse.smart = false; // rendering policy lives in the blob writer (src/text.rs)
    opts
}

/// Plain text with no markdown syntax.
pub const PLAIN: &str = "Just some plain text with no markdown formatting at all.";

/// Single line with bold, italic, and inline code.
pub const SIMPLE: &str = "**bold** and *italic* and `code`";

/// Typical comment with headings, lists, blockquote, code block, and link.
pub const MEDIUM: &str = "\
# Heading 1\n\n\
Some **bold** text with *italic* and `inline code`.\n\n\
## Heading 2\n\n\
> A blockquote with **nested bold**\n\n\
- item one\n\
- item two\n\
  - nested item\n\
- item three\n\n\
1. first\n\
2. second\n\
3. third\n\n\
[a link](https://example.com)\n\n\
```\ncode block\nwith lines\n```\n\n\
---\n\n\
~~strikethrough~~ normal text\n";

/// All heading levels, paragraphs with formatting, nested blockquotes, lists, code, table, hrule.
pub fn complex() -> String {
    let mut s = String::new();
    for i in 1..=6 {
        for _ in 0..i {
            s.push('#');
        }
        s.push_str(&format!(" Heading {}\n\n", i));
    }
    for i in 0..10 {
        s.push_str(&format!(
            "Paragraph {} with **bold**, *italic*, ~~strikethrough~~, `code`, and a [link](https://example.com/{}).\n\n",
            i, i
        ));
    }
    s.push_str("> level 1\n> > level 2\n> > > level 3\n\n");
    for i in 0..20 {
        s.push_str(&format!("- item {}\n", i));
    }
    s.push('\n');
    for i in 1..=10 {
        s.push_str(&format!("{}. ordered {}\n", i, i));
    }
    s.push_str("\n```java\npublic class Foo {\n    void bar() {}\n}\n```\n\n");
    s.push_str("| a | b | c |\n|---|---|---|\n| 1 | 2 | 3 |\n| 4 | 5 | 6 |\n\n");
    s.push_str("---\n");
    s
}

/// 50 repeated bold/italic/code groups — stress test for inline parsing.
pub fn heavy_inline() -> String {
    let mut s = String::new();
    for i in 0..50 {
        s.push_str(&format!("**b{}** *i{}* `c{}` ", i, i, i));
    }
    s
}

/// 10 levels of nested blockquotes.
pub fn deep_nesting() -> String {
    let mut s = String::from("> level 1\n");
    for i in 2..=10 {
        for _ in 0..i {
            s.push_str("> ");
        }
        s.push_str(&format!("level {}\n", i));
    }
    s
}

/// ~13K char document simulating a detailed post with 10 sections.
pub fn long_doc() -> String {
    let mut s = String::new();
    s.push_str("# Introduction\n\n");
    s.push_str("This is a long document with many sections, ");
    s.push_str("covering a wide range of **markdown** features.\n\n");

    for section in 1..=10 {
        s.push_str(&format!("## Section {}\n\n", section));
        for p in 0..5 {
            s.push_str(&format!(
                "Paragraph {} with **bold text**, *italic text*, ~~strikethrough~~, \
                 `inline code`, and a [link](https://example.com/{}/{}). \
                 Some more text to pad this out to a realistic length, \
                 because real posts tend to have longer paragraphs.\n\n",
                p, section, p
            ));
        }
        s.push_str(&format!("> A relevant quote for section {}\n", section));
        s.push_str("> > With nested context\n\n");
        s.push_str("- point one\n- point two\n- point three\n  - sub-point\n\n");
        s.push_str(&format!("```\ncode_example_{}()\n```\n\n", section));
        s.push_str("---\n\n");
    }

    s.push_str("## Conclusion\n\n");
    s.push_str("Final paragraph with ^superscript^ and ~subscript~ for good measure.\n");
    s
}

/// ~13K chars of contraction-dense comment prose. The other corpora are
/// markdown-feature soup with almost no apostrophes, quotes, or dashes, which
/// let smartypants' fragmentation cost (4x node count on text like this) stay
/// invisible across two optimization passes. Weight this corpus whenever a
/// change touches text-node handling.
pub fn prose_doc() -> String {
    let mut s = String::new();
    for i in 0..40 {
        s.push_str(&format!(
            "I don't think that's what they're saying -- it's more that you can't \
             just \"quote the docs\" and call it done... The author's point in \
             section {i} wasn't about performance, it's about correctness. \
             Here's the [relevant thread](https://example.com/thread/{i}) which \
             shouldn't be ignored -- and yes, it's been discussed before...\n\n"
        ));
    }
    s
}

/// Each site's own pages and their lookalikes, for `alloc_bench` to read through [`is_own`] and
/// [`is_internal`].
pub const SITE_LINKS: &[(u8, &str)] = {
    use crate::blob::sites::{CHAN, HACKER_NEWS, LEMMY, LOBSTERS};
    &[
        (HACKER_NEWS, "https://news.ycombinator.com/item?id=45678901"),
        (HACKER_NEWS, "https://news.ycombinator.com/user?id=dang"),
        (HACKER_NEWS, "https://news.ycombinator.com/newest"),
        (CHAN, "https://boards.4chan.org/g/thread/110063208/some-title#p110072890"),
        (CHAN, "https://boards.4channel.org/g/thread/110063208"),
        (CHAN, "https://boards.4chan.org/g/catalog#s=sqt"),
        (CHAN, "https://boards.4chan.org/g/archive"),
        (LEMMY, "https://lemmy.ml/post/123/456"),
        (LEMMY, "https://lemmy.world/c/rust@programming.dev"),
        (LEMMY, "/c/rust@lemmy.ml"),
        (LEMMY, "https://www.reddit.com/u/spez"),
        (LEMMY, "https://www.youtube.com/c/veritasium"),
        (LOBSTERS, "https://lobste.rs/s/abc123/a_title/comments/def456"),
        (LOBSTERS, "https://lobste.rs/t/rust,go/page/2"),
        (LOBSTERS, "https://lobste.rs/about"),
    ]
};

/// Whether `url` is `site`'s own page, `blob::site::is_own` open to the benches.
pub fn is_own(site: u8, url: &str) -> bool {
    crate::blob::site::is_own(site, url)
}

/// Whether `url` is on `site`'s own host, `blob::site::is_internal` open to the benches.
pub fn is_internal(site: u8, url: &str) -> bool {
    crate::blob::site::is_internal(site, url)
}

/// The same corpora written as the HTML a site would send for them, for the walk over HTML
/// (`blob::render_html_blob`), so the two walks are timed on the same comments. The walk reads no
/// heading, list, rule, table or strike yet, so those tags are scanned and their words kept.
pub mod html {
    /// The site the corpora are read as: Hacker News, a paragraph opening on `>` a quote.
    pub const SITE: u8 = crate::blob::sites::HACKER_NEWS;

    /// Plain text with no markdown syntax.
    pub const PLAIN: &str = "Just some plain text with no markdown formatting at all.";

    /// Single line with bold, italic, and inline code.
    pub const SIMPLE: &str = "<b>bold</b> and <i>italic</i> and <code>code</code>";

    /// Typical comment with headings, lists, blockquote, code block, and link.
    pub const MEDIUM: &str = "\
    <h1>Heading 1</h1>\n\
    <p>Some <b>bold</b> text with <i>italic</i> and <code>inline code</code>.</p>\n\
    <h2>Heading 2</h2>\n\
    <p>&gt; A blockquote with <b>nested bold</b></p>\n\
    <ul>\n<li>item one</li>\n<li>item two\n<ul>\n<li>nested item</li>\n</ul>\n</li>\n<li>item three</li>\n</ul>\n\
    <ol>\n<li>first</li>\n<li>second</li>\n<li>third</li>\n</ol>\n\
    <p><a href=\"https://example.com\">a link</a></p>\n\
    <pre><code>code block\nwith lines\n</code></pre>\n\
    <hr>\n\
    <p><s>strikethrough</s> normal text</p>\n";

    /// All heading levels, paragraphs with formatting, nested blockquotes, lists, code, table, hrule.
    pub fn complex() -> String {
        let mut s = String::new();
        for i in 1..=6 {
            s.push_str(&format!("<h{i}>Heading {i}</h{i}>\n"));
        }
        for i in 0..10 {
            s.push_str(&format!(
                "<p>Paragraph {} with <b>bold</b>, <i>italic</i>, <s>strikethrough</s>, <code>code</code>, and a <a href=\"https://example.com/{}\">link</a>.</p>\n",
                i, i
            ));
        }
        s.push_str("<p>&gt; level 1</p>\n<p>&gt;&gt; level 2</p>\n<p>&gt;&gt;&gt; level 3</p>\n");
        s.push_str("<ul>\n");
        for i in 0..20 {
            s.push_str(&format!("<li>item {}</li>\n", i));
        }
        s.push_str("</ul>\n<ol>\n");
        for i in 1..=10 {
            s.push_str(&format!("<li>ordered {}</li>\n", i));
        }
        s.push_str("</ol>\n");
        s.push_str("<pre><code>public class Foo {\n    void bar() {}\n}\n</code></pre>\n");
        s.push_str("<table><tr><th>a</th><th>b</th><th>c</th></tr><tr><td>1</td><td>2</td><td>3</td></tr><tr><td>4</td><td>5</td><td>6</td></tr></table>\n");
        s.push_str("<hr>\n");
        s
    }

    /// 50 repeated bold/italic/code groups: stress test for inline parsing.
    pub fn heavy_inline() -> String {
        let mut s = String::new();
        for i in 0..50 {
            s.push_str(&format!("<b>b{}</b> <i>i{}</i> <code>c{}</code> ", i, i, i));
        }
        s
    }

    /// 10 levels of nested blockquotes, a paragraph each, as Hacker News' posters write them.
    pub fn deep_nesting() -> String {
        let mut s = String::from("&gt; level 1");
        for i in 2..=10 {
            s.push_str("<p>");
            for _ in 0..i {
                s.push_str("&gt;");
            }
            s.push_str(&format!(" level {}", i));
        }
        s
    }

    /// ~14K char document simulating a detailed post with 10 sections.
    pub fn long_doc() -> String {
        let mut s = String::new();
        s.push_str("<h1>Introduction</h1>\n");
        s.push_str("<p>This is a long document with many sections, ");
        s.push_str("covering a wide range of <b>markdown</b> features.</p>\n");

        for section in 1..=10 {
            s.push_str(&format!("<h2>Section {}</h2>\n", section));
            for p in 0..5 {
                s.push_str(&format!(
                    "<p>Paragraph {} with <b>bold text</b>, <i>italic text</i>, <s>strikethrough</s>, \
                     <code>inline code</code>, and a <a href=\"https://example.com/{}/{}\">link</a>. \
                     Some more text to pad this out to a realistic length, \
                     because real posts tend to have longer paragraphs.</p>\n",
                    p, section, p
                ));
            }
            s.push_str(&format!("<p>&gt; A relevant quote for section {}</p>\n", section));
            s.push_str("<p>&gt;&gt; With nested context</p>\n");
            s.push_str("<ul>\n<li>point one</li>\n<li>point two</li>\n<li>point three\n<ul>\n<li>sub-point</li>\n</ul>\n</li>\n</ul>\n");
            s.push_str(&format!("<pre><code>code_example_{}()\n</code></pre>\n", section));
            s.push_str("<hr>\n");
        }

        s.push_str("<h2>Conclusion</h2>\n");
        s.push_str("<p>Final paragraph with <sup>superscript</sup> and <sub>subscript</sub> for good measure.</p>\n");
        s
    }

    /// ~14K chars of contraction-dense comment prose, its apostrophes and quotes written as Hacker
    /// News writes them, as entities: the corpus to weight whenever a change touches text or entities.
    pub fn prose_doc() -> String {
        let mut s = String::new();
        for i in 0..40 {
            if i > 0 {
                s.push_str("<p>");
            }
            s.push_str(&format!(
                "I don&#x27;t think that&#x27;s what they&#x27;re saying -- it&#x27;s more that you can&#x27;t \
                 just &quot;quote the docs&quot; and call it done... The author&#x27;s point in \
                 section {i} wasn&#x27;t about performance, it&#x27;s about correctness. \
                 Here&#x27;s the <a href=\"https:&#x2F;&#x2F;example.com&#x2F;thread&#x2F;{i}\" rel=\"nofollow\">relevant thread</a> which \
                 shouldn&#x27;t be ignored -- and yes, it&#x27;s been discussed before..."
            ));
        }
        s
    }

    /// Ten real comments from Algolia's Hacker News API, as fetched on 22 Oct 2026, each read alone.
    pub fn hn_comments() -> Vec<&'static str> {
        include_str!("benchmarks/hn_comments.html").split("\n=====\n").collect()
    }

    /// The same ten comments as markdown, as the stopgap Java converter wrote them, for the walk
    /// over comrak's tree.
    pub fn hn_comments_markdown() -> Vec<&'static str> {
        include_str!("benchmarks/hn_comments.md").split("\n=====\n").collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Options, blob, parse_document_zerocopy};

    fn default_opts() -> Options<'static> {
        let mut opts = Options::default();
        opts.extension.strikethrough = true;
        opts.extension.table = true;
        opts.extension.autolink = true;
        opts.extension.superscript = true;
        opts.extension.subscript = true;
        opts.extension.spoiler = true;
        opts.extension.tasklist = true;
        #[cfg(feature = "shortcodes")]
        {
            opts.extension.shortcodes = true;
        }
        opts.extension.footnotes = true;
        opts
    }

    fn parse_and_render(input: &str) -> Option<Vec<u8>> {
        parse_document_zerocopy(input, &default_opts(), |root| {
            blob::render_blob(root, input, crate::blob::sites::PLAIN)
        })
    }

    #[test]
    fn plain_returns_none() {
        assert!(
            parse_and_render(PLAIN).is_none(),
            "plain text should return None"
        );
    }

    #[test]
    fn simple_produces_blob() {
        let blob = parse_and_render(SIMPLE);
        assert!(blob.is_some(), "simple markdown should produce a blob");
    }

    #[test]
    fn medium_produces_blob() {
        let blob = parse_and_render(MEDIUM);
        assert!(blob.is_some());
    }

    #[test]
    fn complex_produces_blob() {
        let blob = parse_and_render(&complex());
        assert!(blob.is_some());
    }

    #[test]
    fn heavy_inline_produces_blob() {
        let blob = parse_and_render(&heavy_inline());
        assert!(blob.is_some());
    }

    #[test]
    fn deep_nesting_produces_blob() {
        let blob = parse_and_render(&deep_nesting());
        assert!(blob.is_some());
    }

    #[test]
    fn long_doc_produces_blob() {
        let blob = parse_and_render(&long_doc());
        assert!(blob.is_some());
        let blob = blob.unwrap();
        // Blob should have header + text + spans
        assert!(blob.len() > 8, "blob too small: {} bytes", blob.len());
    }

    #[test]
    fn bench_all() {
        let opts = default_opts();
        let inputs: Vec<(&str, String)> = vec![
            ("plain", PLAIN.to_string()),
            ("simple", SIMPLE.to_string()),
            ("medium", MEDIUM.to_string()),
            ("deep-nesting", deep_nesting()),
            ("heavy-inline", heavy_inline()),
            ("complex", complex()),
            ("long-doc", long_doc()),
            ("prose", prose_doc()),
        ];

        for (name, input) in &inputs {
            let trimmed = input.trim();

            // Warmup
            for _ in 0..100 {
                let _ = parse_document_zerocopy(trimmed, &opts, |root| {
                    blob::render_blob(root, trimmed, crate::blob::sites::PLAIN)
                });
            }

            let iterations = 500;
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let _ = parse_document_zerocopy(trimmed, &opts, |root| {
                    blob::render_blob(root, trimmed, crate::blob::sites::PLAIN)
                });
            }
            let elapsed = start.elapsed() / iterations;
            eprintln!(
                "{} ({} chars): {:.1} us",
                name,
                trimmed.len(),
                elapsed.as_nanos() as f64 / 1000.0
            );
        }

        // The same corpora as HTML, through the walk over HTML
        let html_inputs: Vec<(&str, String)> = vec![
            ("html/plain", html::PLAIN.to_string()),
            ("html/simple", html::SIMPLE.to_string()),
            ("html/medium", html::MEDIUM.to_string()),
            ("html/deep-nesting", html::deep_nesting()),
            ("html/heavy-inline", html::heavy_inline()),
            ("html/complex", html::complex()),
            ("html/long-doc", html::long_doc()),
            ("html/prose", html::prose_doc()),
        ];
        for (name, input) in &html_inputs {
            let trimmed = input.trim();
            for _ in 0..100 {
                let _ = blob::render_html_blob(trimmed, html::SITE, None);
            }
            let iterations = 500;
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let _ = blob::render_html_blob(trimmed, html::SITE, None);
            }
            let elapsed = start.elapsed() / iterations;
            eprintln!(
                "{} ({} chars): {:.1} us",
                name,
                trimmed.len(),
                elapsed.as_nanos() as f64 / 1000.0
            );
        }

        // Ten real Hacker News comments both ways, the ten read in turn as one iteration
        let html_comments = html::hn_comments();
        let md_comments = html::hn_comments_markdown();
        let iterations = 500;
        for _ in 0..100 {
            html_comments.iter().for_each(|c| drop(blob::render_html_blob(c, html::SITE, None)));
        }
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            html_comments.iter().for_each(|c| drop(blob::render_html_blob(c, html::SITE, None)));
        }
        let html_elapsed = start.elapsed() / iterations;
        for _ in 0..100 {
            md_comments.iter().for_each(|c| drop(parse_and_render(c)));
        }
        let start = std::time::Instant::now();
        for _ in 0..iterations {
            md_comments.iter().for_each(|c| drop(parse_and_render(c)));
        }
        let md_elapsed = start.elapsed() / iterations;
        eprintln!(
            "hn-comments: html {:.1} us, markdown {:.1} us",
            html_elapsed.as_nanos() as f64 / 1000.0,
            md_elapsed.as_nanos() as f64 / 1000.0
        );
    }

    #[test]
    fn html_corpora_produce_blobs() {
        assert!(blob::render_html_blob(html::SIMPLE, html::SITE, None).is_some());
        assert!(blob::render_html_blob(&html::long_doc(), html::SITE, None).is_some());
        assert_eq!(html::hn_comments().len(), 10);
        assert_eq!(html::hn_comments_markdown().len(), 10);
    }
}
