//! Parse + blob in a loop under production options, for a sampling profiler:
//!   cargo build --release --example sample_parse
//!   samply record ./target/release/examples/sample_parse prose 4
//! Arguments: corpus (prose, complex, long, heavy) and seconds to run.
use std::time::{Duration, Instant};

fn main() {
    let which = std::env::args().nth(1).unwrap_or_else(|| "prose".into());
    let input = match which.as_str() {
        "complex" => comrak::benchmarks::complex(),
        "long" => comrak::benchmarks::long_doc(),
        "heavy" => comrak::benchmarks::heavy_inline(),
        _ => comrak::benchmarks::prose_doc(),
    };
    let md = input.trim();
    let opts = comrak::benchmarks::production_options();
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(4);
    let start = Instant::now();
    let (mut n, mut bytes) = (0u64, 0usize);
    while start.elapsed() < Duration::from_secs(secs) {
        let b = comrak::parse_document_zerocopy(md, &opts, |root| comrak::blob::render_blob(root, md));
        bytes += b.map(|v| v.len()).unwrap_or(0);
        n += 1;
    }
    let per = start.elapsed().as_nanos() as f64 / n as f64 / 1000.0;
    println!("{which}: {n} iterations, {per:.1} us each, {} blob bytes", bytes / n as usize);
}
