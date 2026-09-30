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
    let program = hgl_program::compile_files(files, libraries, entry)?;
    if let Some(path) = output {
        std::fs::write(path, hgl_program::emit_rust(&program)).map_err(|e| e.to_string())?;
    }
    Ok(())
}
