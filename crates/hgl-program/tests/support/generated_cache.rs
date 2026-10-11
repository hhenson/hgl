//! Stable generated projects let Cargo validate and reuse unchanged executables.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

pub(super) struct Project {
    pub(super) path: PathBuf,
    _lock: fs::File,
}
impl Project {
    pub(super) fn open(root: &Path, batch: usize) -> io::Result<Self> {
        let path = root
            .join("target/source-test-projects")
            .join(format!("corpus-batch-{batch:03}"));
        fs::create_dir_all(path.join("src"))?;
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path.join(".lock"))?;
        lock.lock()?;
        Ok(Self { path, _lock: lock })
    }
}
pub(super) fn write(path: impl AsRef<Path>, bytes: impl AsRef<[u8]>) -> io::Result<()> {
    let path = path.as_ref();
    match fs::read(path) {
        Ok(previous) if previous == bytes.as_ref() => Ok(()),
        Ok(_) => fs::write(path, bytes),
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::write(path, bytes),
        Err(error) => Err(error),
    }
}

pub(super) fn workspace_lock(root: &Path, project: &Path) -> io::Result<()> {
    let bytes = fs::read(root.join("Cargo.lock"))?;
    let snapshot = project.join(".workspace-lock");
    let lock = project.join("Cargo.lock");
    match fs::read(&snapshot) {
        Ok(previous) if previous == bytes && lock.try_exists()? => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    write(lock, &bytes)?;
    write(snapshot, bytes)
}
