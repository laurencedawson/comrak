//! From mdream 2.0.3 (MIT, see `mdream/LICENSE`), by way of htmlrak: the HTML tokenizer's pieces
//! the walk needs, its markdown writer, selectors, Tailwind, URL and plugin code left behind.
//!
//! - `entities.rs` and `entities_generated.rs`: every WHATWG entity, decoded with the HTML
//!   standard's rules for text and for attributes, as written
//! - `scan.rs`: comments, doctypes and a start tag's attributes, cut to a whole string read at
//!   once: the chunk carries, discarded-byte scans and attribute budgets are left behind, and only
//!   the attributes asked for are kept
//!
//! mdream's own two-space indent is kept, so the files stay comparable with its.

pub(crate) mod entities;
pub(crate) mod scan;
