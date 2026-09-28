use crate::CheckedModule;
pub use hgl_documentation::Documentation;
pub(crate) use hgl_documentation::{normalize, validate};
impl CheckedModule {
    /// Documents in source order, including separate selected implementation docs.
    pub fn documentation(&self) -> &[Documentation] {
        &self.documentation
    }
}
/// Emit Google-style HGL documentation as reStructuredText for Sphinx.
pub fn emit_documentation(module: &CheckedModule) -> String {
    hgl_documentation::render(&module.documentation)
}
