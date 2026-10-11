//! HGL semantics: the program model, its checks and the IR the backend reads.

pub mod any_values;
pub mod body_check;
pub mod boxed_uses;
pub mod collection_check;
pub mod collection_values;
pub mod composite_keys;
pub mod constructor_owners;
pub mod delta_check;
pub mod endpoint_check;
pub mod enums;
pub mod eval_data;
pub mod family_types;
pub mod family_values;
pub mod flow_check;
pub mod harness;
pub mod harness_check;
pub mod harness_ir;
pub mod inheritance;
pub use hgl_source::ir;
pub mod library;
pub mod local_check;
pub mod name_check;
pub mod publication_trace;
pub mod recursive_types;
pub mod scalar_keys;
pub mod shape_obligations;
pub mod source_check;
pub mod static_values;
pub mod struct_check;
pub mod struct_declarations;
pub mod struct_names;
pub mod structural_admission;
pub mod tuple_admission;
pub mod tuple_flow;
pub mod tuple_phase;
pub mod tuple_values;
pub mod tuple_writes;
pub mod type_sizes;
pub mod value_access;
pub mod value_bind;
pub mod value_check;
pub mod value_constant;
pub mod value_eval;
pub mod value_operations;
pub mod value_types;
pub mod wiring_locals;

/// Checked explicit ordinary constructor boundaries.
pub mod value_constructors;

/// Shared read-only checked source traversal.
pub mod ir_visit;

/// Checked ordinary operator type algebra.
pub mod value_operators;

/// Independent lexical storage identities for emitted helper callsites.
pub mod call_locals;

/// Cold reservation of lexical complete observation destinations.
pub mod snapshot_locals;
