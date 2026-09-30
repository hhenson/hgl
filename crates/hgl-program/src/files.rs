use std::path::{Path, PathBuf};

/// Load HGL modules from explicit files and library roots, then check an entry.
/// Library test/example directories are excluded from the compilation unit.
pub fn compile_files(
    files: &[PathBuf],
    libraries: &[PathBuf],
    entry: &str,
) -> Result<crate::Program, String> {
    let mut paths = files.to_vec();
    for root in libraries {
        collect(root, &mut paths)?;
    }
    let sources = paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(p)
                .map(|text| (p.display().to_string(), text))
                .map_err(|e| format!("{}: {e}", p.display()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let program = crate::compile(&sources, entry)?;
    Ok(program)
}
fn collect(path: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = std::fs::read_dir(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            if !matches!(
                entry.file_name().and_then(|s| s.to_str()),
                Some("tests" | "examples")
            ) {
                collect(&entry, out)?;
            }
        } else if entry.extension().is_some_and(|s| s == "hgl") {
            out.push(entry);
        }
    }
    Ok(())
}
