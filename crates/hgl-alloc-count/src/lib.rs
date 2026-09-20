//! A global allocator that counts, for tests only.
//!
//! The runtime must not allocate on the per-tick path
//! (`docs/explorations/0009-designing-for-speed.md`). A test proves that by
//! installing [`CountingAllocator`], warming a scenario up, and asserting that
//! [`count_in`] reports zero for the cycles that follow:
//!
//! ```
//! use hgl_alloc_count::{CountingAllocator, count_in};
//!
//! #[global_allocator]
//! static ALLOCATOR: CountingAllocator = CountingAllocator;
//!
//! let (_, none) = count_in(|| 2 + 2);
//! let (_, some) = count_in(|| vec![1_u8, 2, 3]);
//! assert_eq!((none, some), (0, 1));
//! ```
//!
//! Counts are kept per thread, so tests running side by side do not see each
//! other's allocations.
//!
//! This is the one crate besides the store that may contain `unsafe`: a global
//! allocator cannot be written without it. It is never linked into the
//! runtime.
#![expect(
    unsafe_code,
    reason = "GlobalAlloc is an unsafe trait; every method forwards unchanged to the system allocator"
)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    // Const-initialised and without a destructor, so touching it never allocates.
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

/// The system allocator, counting every allocation made on each thread.
///
/// Frees are not counted: a tick that frees has allocated somewhere.
#[derive(Debug, Clone, Copy, Default)]
pub struct CountingAllocator;

fn record() {
    // A thread that is shutting down has no counter left; its allocations are
    // of no interest to a test.
    let _unavailable = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
}

// SAFETY: every method forwards its arguments unchanged to `System`, so the
// allocator contract the caller upholds is exactly the one `System` requires.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: the caller guarantees `layout` has a non-zero size.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record();
        // SAFETY: the caller guarantees `layout` has a non-zero size.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        record();
        // SAFETY: the caller guarantees `pointer` came from this allocator with
        // `layout`, and that `new_size` is valid for it.
        unsafe { System.realloc(pointer, layout, new_size) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        // SAFETY: the caller guarantees `pointer` came from this allocator with
        // `layout`.
        unsafe { System.dealloc(pointer, layout) }
    }
}

/// The number of allocations this thread has made so far.
///
/// Always zero unless [`CountingAllocator`] is the program's global allocator.
pub fn allocations() -> u64 {
    ALLOCATIONS.try_with(Cell::get).unwrap_or(0)
}

/// Run `work` and return its result with the number of allocations it made on
/// this thread.
pub fn count_in<R>(work: impl FnOnce() -> R) -> (R, u64) {
    let before = allocations();
    let result = work();
    (result, allocations() - before)
}
