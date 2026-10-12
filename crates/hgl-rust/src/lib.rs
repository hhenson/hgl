//! Rust source generation from checked, closed HGL graph plans.
//!
//! The frontend must validate types, indices, method names and execution phases
//! before constructing this IR. Emission performs no source-language checking.
use hgl_rust_codegen::any_expression;
pub use hgl_rust_codegen::any_prepared;
pub use hgl_rust_codegen::any_values;
pub mod capacity;
pub use hgl_rust_codegen::checked_data;
pub use hgl_rust_codegen::collections;
pub use hgl_rust_codegen::composite_keys;
pub use hgl_rust_codegen::deltas;
pub use hgl_rust_codegen::direct_deltas;
mod cold_wiring;
pub use hgl_rust_codegen::cold_transport;
mod emit;
mod native_images;
pub use hgl_rust_codegen::enums;
pub use hgl_rust_codegen::execution_proof;
pub use hgl_rust_codegen::families;
pub use hgl_rust_codegen::finite_domains;
pub mod generators;
pub use hgl_rust_codegen::key_origins;
pub use hgl_rust_codegen::keyed;
pub use hgl_rust_codegen::layouts;
pub use hgl_rust_codegen::mutation_bounds;
pub use hgl_rust_codegen::native_results;
pub use hgl_rust_codegen::observed;
pub mod preparation;
pub use hgl_rust_codegen::prepared_values;
pub use hgl_rust_codegen::pure_bounds;
pub use hgl_rust_codegen::recursive_convert;
pub use hgl_rust_codegen::scalars;
pub use hgl_rust_codegen::snapshot_slots;
pub use hgl_rust_codegen::snapshot_views;
pub use hgl_rust_codegen::snapshots;
pub use hgl_rust_codegen::source_slots;
pub use hgl_rust_codegen::structs;
pub use hgl_rust_codegen::structural_publication;
pub use hgl_rust_codegen::tuple_adapter;
pub use hgl_rust_codegen::type_data;
pub use hgl_rust_codegen::value_calls;
pub use hgl_rust_codegen::value_convert;
pub use hgl_rust_codegen::values;
pub use hgl_rust_codegen::windows;
use hgl_semantics::ir;

pub use emit::{
    emit, emit_prepared_test_body, emit_shared, emit_test_body, shared_layouts,
    shared_suite_layouts, shared_test_layouts,
};
pub use hgl_semantics::ir::{DeltaEntry, Kind, Native, Node, Plan, Statement, Value};

/// Ordered prepared hooks preserve source evaluation and storage ownership.
pub use hgl_rust_codegen::prepared_hooks;

use hgl_rust_codegen::constructor_owners;

use hgl_rust_codegen::append;
use hgl_rust_codegen::native_values;
