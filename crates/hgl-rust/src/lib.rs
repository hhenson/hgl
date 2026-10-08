//! Rust source generation from checked, closed HGL graph plans.
//!
//! The frontend must validate types, indices, method names and execution phases
//! before constructing this IR. Emission performs no source-language checking.
pub mod capacity;
pub mod checked_data;
pub mod collections;
pub mod composite_keys;
pub mod deltas;
pub mod direct_deltas;
mod emit;
pub mod enums;
pub mod execution_proof;
pub mod families;
pub mod finite_domains;
pub mod generators;
pub mod key_origins;
pub mod keyed;
pub mod layouts;
pub mod mutation_bounds;
pub mod observed;
pub mod preparation;
pub mod prepared_values;
pub mod pure_bounds;
pub mod recursive_convert;
pub mod scalars;
pub mod snapshot_slots;
pub mod snapshot_views;
pub mod snapshots;
pub mod source_slots;
pub mod structs;
pub mod structural_publication;
pub mod tuple_adapter;
pub mod type_data;
pub mod value_calls;
pub mod value_convert;
pub mod values;
pub mod windows;
use hgl_semantics::ir;

pub use emit::{emit, emit_prepared_test_body, emit_shared, emit_test_body, shared_layouts};
pub use hgl_semantics::ir::{DeltaEntry, Kind, Native, Node, Plan, Statement, Value};
