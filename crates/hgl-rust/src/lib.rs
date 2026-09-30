//! Rust source generation from checked, closed HGL graph plans.
//!
//! The frontend must validate types, indices, method names and execution phases
//! before constructing this IR. Emission performs no source-language checking.
mod emit;
mod ir;

pub use emit::{emit, emit_test_body};
pub use ir::{Kind, Native, Node, Plan, Statement, Value};
