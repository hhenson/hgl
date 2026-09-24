//! The first complete HGL source-to-Rust compiler slice.
mod check;
mod emit;
mod lex;
mod model;
mod parse;

pub use check::check;
pub use emit::emit_rust;

/// One shared source or explicitly selected implementation part.
#[derive(Debug, Clone)]
pub struct Source {
    /// Filename used in diagnostics.
    pub name: String,
    /// Unmodified UTF-8 source.
    pub text: String,
}

/// A source error, before any native code is emitted or executed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Filename from the supplied source.
    pub source: String,
    /// Inclusive byte offset.
    pub start: usize,
    /// Exclusive byte offset.
    pub end: usize,
    /// The violated language rule or unsupported feature.
    pub message: String,
}

/// Resolved functions and calls, independent of runtime storage.
#[derive(Debug)]
pub struct CheckedModule {
    name: String,
    sources: Vec<String>,
    functions: Vec<model::Function>,
    bodies: Vec<Vec<model::Statement>>,
}
