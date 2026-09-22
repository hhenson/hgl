//! Rust value and borrowed-view projections for the HGL native substrate.
//!
//! Helpers have no node context: activation, state and publication belong to
//! the calling HGL node. Target catalogue selection is compiler work.
use hgl_store::{InputId, Store};
use hgl_types::EngineTime;

/// `hgraph.native.bit_and(i64, i64) -> i64`, on current scalar values.
#[inline]
pub fn bit_and_i64(lhs: i64, rhs: i64) -> i64 {
    lhs & rhs
}

/// A live input borrowed for a native call, including before first validity.
/// The caller supplies a live handle from this store, checked at construction.
///
/// A node cannot retain the view beyond the store borrow (NAT-3):
/// ```compile_fail
/// use hgl_native::InputView;
/// use hgl_store::{InputId, Store};
/// use hgl_types::EngineTime;
/// fn retain(store: &Store, input: InputId) -> InputView<'static> {
///     InputView::new(store, input, EngineTime::MIN_START)
/// }
/// ```
#[derive(Debug)]
pub struct InputView<'a> {
    store: &'a Store,
    input: InputId,
    now: EngineTime,
}

impl<'a> InputView<'a> {
    /// Borrow an input's logical observations at the current evaluation time.
    pub fn new(store: &'a Store, input: InputId, now: EngineTime) -> Self {
        Self { store, input, now }
    }

    /// Whether a value is available, independently of modification time.
    #[inline]
    pub fn valid(&self) -> bool {
        self.store.bindings().valid(self.input)
    }

    /// Whether the input and each immediate child are valid (TS-9).
    #[inline]
    pub fn all_valid(&self) -> bool {
        self.store.bindings().all_valid(self.input)
    }

    /// Whether this input reports a change in the current cycle.
    #[inline]
    pub fn modified(&self) -> bool {
        self.store.bindings().modified(self.input, self.now)
    }

    /// Cached logical time, including invalidation within a valid assembly.
    #[inline]
    pub fn last_modified(&self) -> EngineTime {
        self.store.bindings().last_modified(self.input)
    }
}
