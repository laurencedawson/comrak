//! Profile where parse time is spent.
//! Run with: cargo run --release --example profile_parse

fn main() {
    use comrak::{blob, parse_document_zerocopy, Options};
    use std::time::Instant;

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

    let input = comrak::benchmarks::long_doc();
    let trimmed = input.trim();

    // Warmup
    for _ in 0..200 {
        parse_document_zerocopy(trimmed, &opts, |_| {});
    }

    // Measure total parse, many iterations
    let iterations = 2000;
    let start = Instant::now();
    for _ in 0..iterations {
        parse_document_zerocopy(trimmed, &opts, |_| {});
    }
    let total = start.elapsed() / iterations;
    println!("long-doc total parse: {:.1} us", total.as_nanos() as f64 / 1000.0);

    // Now with blob rendering
    let start = Instant::now();
    for _ in 0..iterations {
        parse_document_zerocopy(trimmed, &opts, |root| {
            let _ = blob::render_blob(root, trimmed);
        });
    }
    let total_with_blob = start.elapsed() / iterations;
    let blob_time = total_with_blob - total;
    println!("long-doc parse+blob: {:.1} us (blob: {:.1} us)",
        total_with_blob.as_nanos() as f64 / 1000.0,
        blob_time.as_nanos() as f64 / 1000.0);

    // Profile each test
    let inputs = vec![
        ("plain", comrak::benchmarks::PLAIN.to_string()),
        ("simple", comrak::benchmarks::SIMPLE.to_string()),
        ("medium", comrak::benchmarks::MEDIUM.to_string()),
        ("heavy-inline", comrak::benchmarks::heavy_inline()),
        ("complex", comrak::benchmarks::complex()),
        ("long-doc", comrak::benchmarks::long_doc()),
    ];

    println!("\n{:<15} {:>8} {:>8} {:>8}", "test", "parse", "blob", "total");
    println!("{:-<43}", "");
    for (name, input) in &inputs {
        let trimmed = input.trim();
        let iterations = 2000;

        let start = Instant::now();
        for _ in 0..iterations {
            parse_document_zerocopy(trimmed, &opts, |_| {});
        }
        let parse = start.elapsed() / iterations;

        let start = Instant::now();
        for _ in 0..iterations {
            parse_document_zerocopy(trimmed, &opts, |root| {
                let _ = blob::render_blob(root, trimmed);
            });
        }
        let total = start.elapsed() / iterations;
        let blob = total - parse;

        println!("{:<15} {:>6.1} us {:>6.1} us {:>6.1} us",
            name,
            parse.as_nanos() as f64 / 1000.0,
            blob.as_nanos() as f64 / 1000.0,
            total.as_nanos() as f64 / 1000.0);
    }
}
