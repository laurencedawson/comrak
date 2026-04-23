//! Benchmark test data and helpers for blob rendering.
//! Mirrors the Android instrumentation test suite in MarkdownBenchmarkTest.java.

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
        for _ in 0..i { s.push('#'); }
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
        for _ in 0..i { s.push_str("> "); }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{parse_document_raw, Arena, Options, blob};

    fn default_opts() -> Options<'static> {
        let mut opts = Options::default();
        opts.extension.strikethrough = true;
        opts.extension.table = true;
        opts.extension.autolink = true;
        opts.extension.superscript = true;
        opts.extension.subscript = true;
        opts.extension.spoiler = true;
        opts.extension.tasklist = true;
        opts.extension.shortcodes = true;
        opts.extension.footnotes = true;
        opts.parse.smart = true;
        opts
    }

    fn parse_and_render(input: &str) -> Option<Vec<u8>> {
        let (nc, sc) = crate::arena_capacities(input.len());
        let (arena, string_arena) = (Arena::with_capacity(nc), crate::StringArena::with_capacity(sc));
        let opts = default_opts();
        let root = parse_document_raw(&arena, &string_arena, input, &opts);
        blob::render_blob(root, input)
    }

    #[test]
    fn plain_returns_none() {
        assert!(parse_and_render(PLAIN).is_none(), "plain text should return None");
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
        ];

        for (name, input) in &inputs {
            let trimmed = input.trim();

            // Warmup
            for _ in 0..100 {
                let (nc, sc) = crate::arena_capacities(trimmed.len());
                let (arena, string_arena) = (Arena::with_capacity(nc), crate::StringArena::with_capacity(sc));
                let root = parse_document_raw(&arena, &string_arena, trimmed, &opts);
                let _ = blob::render_blob(root, trimmed);
            }

            let iterations = 500;
            let start = std::time::Instant::now();
            for _ in 0..iterations {
                let (nc, sc) = crate::arena_capacities(trimmed.len());
                let (arena, string_arena) = (Arena::with_capacity(nc), crate::StringArena::with_capacity(sc));
                let root = parse_document_raw(&arena, &string_arena, trimmed, &opts);
                let _ = blob::render_blob(root, trimmed);
            }
            let elapsed = start.elapsed() / iterations;
            eprintln!("{} ({} chars): {:.1} us",
                name, trimmed.len(),
                elapsed.as_nanos() as f64 / 1000.0);
        }
    }

}
