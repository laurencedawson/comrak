//! A typed arena that grows by 1.25x instead of 2x, reducing memory waste.
//! Drop-in replacement for typed_arena::Arena for the node allocator.

use std::cell::RefCell;
use std::fmt;

/// A typed arena with 1.25x growth factor.
pub struct Arena<T> {
    chunks: RefCell<ChunkList<T>>,
}

struct ChunkList<T> {
    current: Vec<T>,
    rest: Vec<Vec<T>>,
}

impl<T: fmt::Debug> fmt::Debug for Arena<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Arena").field("len", &self.len()).finish()
    }
}

impl<T> Arena<T> {
    /// Create a new arena with default capacity.
    pub fn new() -> Self {
        Self::with_capacity(8)
    }

    /// Create a new arena with capacity for `n` values.
    pub fn with_capacity(n: usize) -> Self {
        Arena {
            chunks: RefCell::new(ChunkList {
                current: Vec::with_capacity(n.max(1)),
                rest: Vec::new(),
            }),
        }
    }

    /// Allocate a value in the arena and return a mutable reference.
    pub fn alloc(&self, value: T) -> &mut T {
        let mut chunks = self.chunks.borrow_mut();
        if chunks.current.len() == chunks.current.capacity() {
            // Double for first 2 growths (reach working size fast), then grow
            // by current capacity (linear). Fewer chunks than 1.25x for large
            // docs, less waste than 2x doubling at the end.
            let new_cap = if chunks.rest.len() < 2 {
                chunks.current.capacity() * 2
            } else {
                chunks.current.capacity()
            }.max(16);
            let old = std::mem::replace(&mut chunks.current, Vec::with_capacity(new_cap));
            chunks.rest.push(old);
        }
        let len = chunks.current.len();
        chunks.current.push(value);
        // SAFETY: we never give out references to existing items, and we never
        // reallocate current while references are outstanding (we move to rest instead).
        unsafe { &mut *chunks.current.as_mut_ptr().add(len) }
    }

    /// Return the total number of items allocated.
    pub fn len(&self) -> usize {
        let chunks = self.chunks.borrow();
        chunks.current.len() + chunks.rest.iter().map(|c| c.len()).sum::<usize>()
    }
}
