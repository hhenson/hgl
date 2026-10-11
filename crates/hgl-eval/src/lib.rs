//! Checked IR and ordinary execution cannot depend on source parsing or semantic binding.
pub mod any_values;
pub mod collection_values;
pub mod composite_keys;
pub mod family_values;
pub mod harness_ir;
pub use hgl_value_model::ir;
pub mod ir_visit;
pub mod publication_trace;
pub mod scalar_keys;
pub mod value_constant;
pub mod value_eval;
pub mod value_operations;
