//! Ordinary metadata and contextual scalar materialization independent of source syntax.
pub use diagnostics::Issue;
pub use literals::{Literal, ParsedLiteral, TemporalLiteral};
pub use type_shape::{
    EnumType, FamilyType, Nominal, NominalDefinition, RecursiveType, Ty, Window, WindowKind,
    application, delta_argument,
};
pub mod diagnostics;
pub mod literals;
pub mod nominal_batch;
pub mod ordinary_shape;
pub mod scalar_shape;
pub mod time_context;
pub mod type_shape;
pub mod type_syntax;
