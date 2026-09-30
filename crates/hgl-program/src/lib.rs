//! Source linking and Rust emission for closed scalar HGL graphs.
mod emit;
mod files;
use hgl_library as index;
mod resolve;
pub use files::{compile_files, compile_tests_files};
use hgl_source as syntax;

/// A fully resolved graph, with scalar configuration fixed during wiring.
#[derive(Debug)]
pub struct Program(resolve::Plan);

/// Resolve an entry against explicitly supplied modules and implementation parts.
/// Only reachable implementation bodies are admitted by this compiler slice.
pub fn compile(sources: &[(String, String)], entry: &str) -> Result<Program, String> {
    let library = index::load(sources)?;
    let module = library.root.clone();
    resolve::compile(library, &module, entry).map(Program)
}

/// Emit node implementations, native interfaces and graph construction.
pub fn emit_rust(program: &Program) -> String {
    emit::emit(&program.0)
}

mod tests;
pub use tests::{Suite, compile_tests, emit_tests};
