//! Canonical ordinary value metadata cannot depend on parsing, name resolution or provider discovery.
pub use diagnostics::Issue;
pub use literals::{Literal, ParsedLiteral, TemporalLiteral};
pub use type_shape::{
    EnumType, FamilyType, Nominal, NominalDefinition, RecursiveType, Ty, Window, WindowKind,
    application, delta_argument,
};
pub mod diagnostics;
pub mod execution_phase;
pub mod literals;
pub mod nominal_batch;
pub mod ordinary_shape;
pub mod scalar_shape;
pub mod time_context;
pub mod type_shape;
pub mod type_syntax;

/// Cold owning native data supplied only by a selected provider context.
pub mod native_value;

pub mod native_contracts;
