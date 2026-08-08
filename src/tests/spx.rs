use std::collections::VecDeque;

use crate::nodes::Sourcepos;
use crate::parser::Spx;

/// End-aligned resolution inside a length-mismatched entry (byte len !=
/// column span). Its integration producer — the smart '!' cap — was retired
/// with the fork's smart extras; entities still create mismatched entries,
/// so the branch stays and this pins it.
#[test]
fn col_at_end_aligns_inside_length_mismatched_entry() {
    let spxv: VecDeque<(Sourcepos, usize)> = VecDeque::from([
        ((1, 1, 1, 3).into(), 3),  // "wow" — faithful
        ((1, 4, 1, 7).into(), 3),  // capped run: 4 columns, 3 bytes
        ((1, 8, 1, 16).into(), 9), // "abc@y.com" — faithful
    ]);
    let spx = Spx(&spxv);
    assert_eq!(spx.col_at(0), 0); // run start: column before the text
    assert_eq!(spx.col_at(2), 2); // inside faithful: start-aligned
    assert_eq!(spx.col_at(3), 3); // entry boundary
    assert_eq!(spx.col_at(5), 6); // inside mismatched: end-aligned
    assert_eq!(spx.col_at(6), 7); // mismatched entry end
    assert_eq!(spx.col_at(12), 13); // faithful again past it
}
