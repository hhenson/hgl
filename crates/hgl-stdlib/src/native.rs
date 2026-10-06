//! Rust value and borrowed-view projections for the HGL native substrate.
//!
//! Helpers have no node context: activation, state and publication belong to
//! the calling HGL node. The scalar contract is generated from shared HGL.
use hgl_store::{InputId, Store};
use hgl_types::EngineTime;

mod scalar_interface;
pub use scalar_interface::Native;

/// Rust implementation of the shared scalar value contract.
/// A provider must implement the declared result and argument types exactly:
/// ```compile_fail
/// use crate::native::Native;
/// struct WrongResult;
/// impl Native for WrongResult {
///     fn bit_and(lhs: i64, rhs: i64) -> f64 { (lhs & rhs) as f64 }
/// }
/// ```
/// ```compile_fail
/// use crate::native::Native;
/// struct WrongInput;
/// impl Native for WrongInput {
///     fn bit_and(lhs: i32, rhs: i64) -> i64 { i64::from(lhs) & rhs }
/// }
/// ```
#[derive(Debug)]
pub struct StandardNative;

impl Native for StandardNative {
    #[inline]
    fn bit_and(lhs: i64, rhs: i64) -> i64 {
        lhs & rhs
    }
}

/// `native const fn hgraph.native.bit_and(i64, i64) -> i64`.
#[inline]
pub fn bit_and_i64(lhs: i64, rhs: i64) -> i64 {
    StandardNative::bit_and(lhs, rhs)
}

/// A live input borrowed for a native call, including before first validity.
/// The caller supplies a live handle from this store, checked at construction.
///
/// A node cannot retain the view beyond the store borrow (NAT-3):
/// ```compile_fail
/// use crate::native::InputView;
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
