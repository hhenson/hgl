use std::path::{Path, PathBuf};

pub(crate) fn run(
    files: &[PathBuf],
    libraries: &[PathBuf],
    entry: &str,
    command: &str,
    output: Option<&Path>,
) -> Result<(), String> {
    if command == "doc" {
        return Err("library documentation export is not yet supported".into());
    }
    if command == "emit-tests" {
        let suite = hgl_program::compile_tests_files(files, libraries)?;
        let path = output.ok_or("emit-tests requires --out")?;
        return std::fs::write(path, hgl_program::emit_tests(&suite)).map_err(|e| e.to_string());
    }
    let program = hgl_program::compile_files(files, libraries, entry)?;
    if let Some(path) = output {
        std::fs::write(path, hgl_program::emit_rust(&program)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
