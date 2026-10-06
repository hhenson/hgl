use std::path::PathBuf;

/// Load HGL modules from explicit files and library roots, then check an entry.
/// Library test/example directories are excluded from the compilation unit.
pub fn compile_files(
    files: &[PathBuf],
    libraries: &[PathBuf],
    entry: &str,
) -> Result<crate::Program, String> {
    crate::compile(&crate::library_files::sources(files, libraries)?, entry)
}
/// Load and check all named tests from explicit files and their source library.
pub fn compile_tests_files(
    files: &[PathBuf],
    libraries: &[PathBuf],
) -> Result<crate::Suite, String> {
    crate::compile_tests(&crate::library_files::sources(files, libraries)?)
}
