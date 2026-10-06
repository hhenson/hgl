//! Source linking and Rust emission for closed scalar HGL graphs.
use hgl_rust as emit;
mod files;
use hgl_library as index;
mod resolve;
pub use files::{compile_files, compile_tests_files};
use hgl_source as syntax;

/// A fully resolved graph, with scalar configuration fixed during wiring.
#[derive(Debug)]
pub struct Program(hgl_rust::Plan);

/// Resolve an entry against explicitly supplied modules and implementation parts.
/// Only reachable implementation bodies are admitted by this compiler slice.
pub fn compile(sources: &[(String, String)], entry: &str) -> Result<Program, String> {
    hgl_source_check::ensure_sources(sources)?;
    let library = index::load(sources)?;
    hgl_enums::validate(&library)?;
    let module = library.root.clone();
    resolve::compile(library, &module, entry)
        .map(Program)
        .map_err(|e| hgl_diagnostics::render_issue(sources, e))
}

/// Emit node implementations, native interfaces and graph construction.
pub fn emit_rust(program: &Program) -> String {
    emit::emit(&program.0)
}

mod tests;
pub use tests::{Suite, compile_module_suite, compile_suite, compile_tests, emit_tests};

/// Whole-source diagnostics, including deferred bounds and instantiated call failures.
pub fn diagnostics(sources: &[(String, String)]) -> Vec<hgl_diagnostics::Diagnostic> {
    hgl_source_check::with_semantics(sources, |library, decl| {
        resolve::validate_declaration(library.clone(), decl)
    })
}
/// Render the ordinary whole-source diagnostics for command-line checking.
pub fn check_sources(sources: &[(String, String)]) -> Result<(), String> {
    hgl_diagnostics::ensure(diagnostics(sources))
}

/// Ordinary diagnostics for one target module and its production dependencies.
pub fn module_diagnostics(sources: &[(String, String)]) -> Vec<hgl_diagnostics::Diagnostic> {
    hgl_source_check::with_module_semantics(sources, |library, decl| {
        resolve::validate_declaration(library.clone(), decl)
    })
}
