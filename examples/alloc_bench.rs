//! Allocation tracking benchmark for blob rendering.
//! Run with: cargo run --release --example alloc_bench

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

struct TrackingAlloc;

static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static TRACKING: AtomicUsize = AtomicUsize::new(0);
// Size buckets: tiny (1-32), small (33-128), medium (129-1024), large (1025+)
static BUCKET_TINY: AtomicUsize = AtomicUsize::new(0);
static BUCKET_SMALL: AtomicUsize = AtomicUsize::new(0);
static BUCKET_MEDIUM: AtomicUsize = AtomicUsize::new(0);
static BUCKET_LARGE: AtomicUsize = AtomicUsize::new(0);

// Fine-grained histogram: 16 buckets (powers of 2: 1-2, 3-4, 5-8, 9-16, 17-32, 33-64, 65-128, 129-256, 257-512, 513-1024, 1025-2048, 2049-4096, 4097-8192, 8193-16384, 16385-32768, 32769+)
static HISTOGRAM: [AtomicUsize; 16] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
];

fn size_to_bucket(size: usize) -> usize {
    if size == 0 { return 0; }
    let bits = usize::BITS - (size - 1).leading_zeros();
    (bits as usize).min(15)
}

unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if TRACKING.load(Ordering::Relaxed) != 0 {
            ALLOC_COUNT.fetch_add(1, Ordering::Relaxed);
            ALLOC_BYTES.fetch_add(layout.size(), Ordering::Relaxed);
            match layout.size() {
                1..=32 => { BUCKET_TINY.fetch_add(1, Ordering::Relaxed); }
                33..=128 => { BUCKET_SMALL.fetch_add(1, Ordering::Relaxed); }
                129..=1024 => { BUCKET_MEDIUM.fetch_add(1, Ordering::Relaxed); }
                _ => { BUCKET_LARGE.fetch_add(1, Ordering::Relaxed); }
            }
            HISTOGRAM[size_to_bucket(layout.size())].fetch_add(1, Ordering::Relaxed);
        }
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

fn reset() {
    ALLOC_COUNT.store(0, Ordering::Relaxed);
    ALLOC_BYTES.store(0, Ordering::Relaxed);
    BUCKET_TINY.store(0, Ordering::Relaxed);
    BUCKET_SMALL.store(0, Ordering::Relaxed);
    BUCKET_MEDIUM.store(0, Ordering::Relaxed);
    BUCKET_LARGE.store(0, Ordering::Relaxed);
    for h in &HISTOGRAM { h.store(0, Ordering::Relaxed); }
}

fn print_histogram() {
    let labels = ["1-2", "3-4", "5-8", "9-16", "17-32", "33-64", "65-128",
                   "129-256", "257-512", "513-1K", "1K-2K", "2K-4K", "4K-8K", "8K-16K", "16K-32K", "32K+"];
    print!("  histogram:");
    for (i, label) in labels.iter().enumerate() {
        let v = HISTOGRAM[i].load(Ordering::Relaxed);
        if v > 0 {
            print!(" {}={}", label, v);
        }
    }
    println!();
}

fn start_tracking() {
    reset();
    TRACKING.store(1, Ordering::Relaxed);
}

fn stop_tracking() -> (usize, usize) {
    TRACKING.store(0, Ordering::Relaxed);
    (ALLOC_COUNT.load(Ordering::Relaxed), ALLOC_BYTES.load(Ordering::Relaxed))
}

fn main() {
    use comrak::{parse_document_raw, Arena, StringArena, Options};
    use comrak::nodes::AstNode;
    println!("AstNode size: {} bytes", std::mem::size_of::<AstNode>());

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

    let inputs: Vec<(&str, String)> = vec![
        ("plain", comrak::blob_bench::PLAIN.to_string()),
        ("simple", comrak::blob_bench::SIMPLE.to_string()),
        ("medium", comrak::blob_bench::MEDIUM.to_string()),
        ("deep-nesting", comrak::blob_bench::deep_nesting()),
        ("heavy-inline", comrak::blob_bench::heavy_inline()),
        ("complex", comrak::blob_bench::complex()),
        ("long-doc", comrak::blob_bench::long_doc()),
    ];

    // Warmup
    for (_, input) in &inputs {
        let (nc, sc) = comrak::arena_capacities(input.trim().len());
        let (arena, string_arena) = (Arena::with_capacity(nc), StringArena::with_capacity(sc));
        let root = parse_document_raw(&arena, &string_arena, input.trim(), &opts);
        let _ = comrak::blob::render_blob(root, input.trim(), false);
    }

    println!("{:<20} {:>8} {:>10} {:>12}", "test", "chars", "allocs", "bytes");
    println!("{:-<52}", "");

    for (name, input) in &inputs {
        let trimmed = input.trim();

        // Combined: parse + blob
        start_tracking();
        let (nc, sc) = comrak::arena_capacities(trimmed.len());
        let (arena, string_arena) = (Arena::with_capacity(nc), StringArena::with_capacity(sc));
        let root = parse_document_raw(&arena, &string_arena, trimmed, &opts);
        let (parse_count, parse_bytes) = (
            ALLOC_COUNT.load(Ordering::Relaxed),
            ALLOC_BYTES.load(Ordering::Relaxed),
        );
        let _ = comrak::blob::render_blob(root, trimmed, false);
        let (total_raw_count, total_raw_bytes) = stop_tracking();
        let blob_count = total_raw_count - parse_count;
        let blob_bytes = total_raw_bytes - parse_bytes;

        let total_count = parse_count + blob_count;
        let total_bytes = parse_bytes + blob_bytes;
        println!("{:<20} {:>6} chars | {:>5} allocs {:>6} KB | parse {:>5} blob {:>4}",
            name, trimmed.len(), total_count, total_bytes / 1024, parse_count, blob_count);

        if name == &"long-doc" || name == &"heavy-inline" {
            let tiny = BUCKET_TINY.load(Ordering::Relaxed);
            let small = BUCKET_SMALL.load(Ordering::Relaxed);
            let medium = BUCKET_MEDIUM.load(Ordering::Relaxed);
            let large = BUCKET_LARGE.load(Ordering::Relaxed);
            println!("  buckets: tiny(1-32)={} small(33-128)={} medium(129-1K)={} large(1K+)={}",
                tiny, small, medium, large);
            print_histogram();
        }
    }
}
