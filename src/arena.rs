//! A typed arena with doubling-then-linear growth, reducing memory waste for
//! large documents. Drop-in replacement for `typed_arena::Arena` for the node
//! allocator.
//!
//! # Soundness
//!
//! `Arena` stores its bookkeeping in `UnsafeCell` rather than `RefCell` to
//! avoid the borrow-counter overhead on the hot parse path. That means the
//! caller must not provoke the following patterns — all of which are
//! structurally impossible through this crate's public surface:
//!
//! 1. Concurrent access from another thread. `Arena` is `!Sync` via
//!    `UnsafeCell`; the compiler rejects sharing it across threads.
//! 2. Reading `T` values through a shared `&self` method (e.g. `len()`)
//!    while another `&mut T` obtained from `alloc()` is still live. The
//!    only shared-access method here reads `Vec` headers, never `T`
//!    storage, so no `T` aliasing ever occurs.
//! 3. Mutating the `ChunkList` while a raw `*mut T` derived from it is
//!    pending dereference. `alloc()` scopes its `ChunkList` borrow into an
//!    inner block that ends before the returned `&mut T` materializes.
//!
//! Previous chunks in `rest` are pointer-stable: when `current` fills up
//! we `mem::replace` it into `rest` rather than growing in place, so
//! outstanding `&T` / `&mut T` references into older chunks remain valid
//! for the lifetime of the arena.

use std::cell::UnsafeCell;
use std::fmt;

/// A typed arena with doubling-then-linear growth.
///
/// Growth policy: the first two chunks grow by 2× (to reach working size
/// fast), subsequent chunks grow by the current capacity (linear). This
/// settles to ~1.25× amortized growth on large inputs, reducing trailing
/// memory waste vs pure doubling.
pub struct Arena<T> {
    chunks: UnsafeCell<ChunkList<T>>,
}

struct ChunkList<T> {
    /// Vec that receives the next `push`. Pointer-stable until it fills up;
    /// at that point it's moved into `rest`.
    current: Vec<T>,
    /// Filled-and-sealed chunks. Never modified again. References into them
    /// remain valid for the arena's lifetime.
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

    /// Create a new arena, pre-sizing the first chunk for `n` values.
    pub fn with_capacity(n: usize) -> Self {
        Arena {
            chunks: UnsafeCell::new(ChunkList {
                current: Vec::with_capacity(n.max(1)),
                rest: Vec::new(),
            }),
        }
    }

    /// Allocate a value in the arena and return a unique `&mut T` pointing
    /// at it, borrowing the arena.
    ///
    /// Grows `current` by moving it into `rest` and allocating a fresh
    /// larger `Vec` when full — never reallocates `current` in place, so
    /// prior `&T`/`&mut T` references stay valid.
    #[inline]
    pub fn alloc(&self, value: T) -> &mut T {
        // Stage 1: take a scoped `&mut ChunkList` borrow just long enough to
        // push and obtain a raw pointer to the new slot. The borrow ends at
        // the closing brace so it can't co-exist with the `&mut T` below.
        let slot_ptr: *mut T = unsafe {
            let chunks = &mut *self.chunks.get();
            if chunks.current.len() == chunks.current.capacity() {
                grow(chunks);
            }
            chunks.current.push(value);
            // SAFETY: we just pushed, so `last_mut()` is `Some`.
            std::ptr::from_mut(chunks.current.last_mut().unwrap_unchecked())
        };
        // Stage 2: materialize the unique `&mut T`.
        // SAFETY: `slot_ptr` points to a freshly created slot — no other
        // reference to it exists, `current`'s buffer will not be realloc'd
        // while this reference lives (growth moves it into `rest`, which
        // is pointer-stable), and lifetime is bound to `&self`.
        unsafe { &mut *slot_ptr }
    }

    /// Return the total number of items allocated across all chunks.
    pub fn len(&self) -> usize {
        // SAFETY: reads only `Vec` headers (`len()`), never dereferences any
        // `T` value. No `T` aliasing can arise from this access even when
        // callers hold live `&mut T` from earlier `alloc()` calls.
        let chunks = unsafe { &*self.chunks.get() };
        chunks.current.len() + chunks.rest.iter().map(|c| c.len()).sum::<usize>()
    }
}

/// Move the full `current` Vec into `rest` and replace it with a fresh,
/// larger one. Split from `alloc` and marked `#[cold]` so the compiler keeps
/// the fast path lean — growth happens only a handful of times per parse.
#[cold]
#[inline(never)]
fn grow<T>(chunks: &mut ChunkList<T>) {
    let new_cap = if chunks.rest.len() < 2 {
        chunks.current.capacity() * 2
    } else {
        chunks.current.capacity()
    }
    .max(16);
    let old = std::mem::replace(&mut chunks.current, Vec::with_capacity(new_cap));
    chunks.rest.push(old);
}
