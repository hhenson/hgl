//! Rust source generation from checked, closed HGL graph plans.
//!
//! The frontend must validate types, indices, method names and execution phases
//! before constructing this IR. Emission performs no source-language checking.
mod emit;
use hgl_rust_ir as ir;

pub use emit::{emit, emit_prepared_test_body, emit_shared, emit_test_body, shared_layouts};
pub use hgl_rust_ir::{DeltaEntry, Kind, Native, Node, Plan, Statement, Value};
