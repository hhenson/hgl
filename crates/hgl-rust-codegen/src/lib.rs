//! Checked value, layout and hook emission, isolated from executable program assembly.
/// Prepared expression emission used by program capacity planning.
pub mod any_expression;
mod any_expression_leaves;
pub mod any_prepared;
pub mod any_values;
pub mod checked_data;
pub mod collections;
pub mod composite_keys;
/// Checked constructor ownership and reservation expressions.
pub mod constructor_owners;
pub mod deltas;
pub mod direct_deltas;
pub mod enums;
pub mod execution_proof;
pub mod families;
pub mod finite_domains;
mod identifiers;
pub mod key_origins;
pub mod keyed;
pub mod layouts;
pub mod mutation_bounds;
pub mod observed;
mod owner_comparisons;
pub mod prepared_hooks;
mod prepared_global_writes;
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
/// Append generated code to an infallible String sink.
pub fn append(code: &mut String, text: std::fmt::Arguments<'_>) {
    use std::fmt::Write as _;
    code.write_fmt(text)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}

/// Pure native target module naming.
pub mod native_image;
