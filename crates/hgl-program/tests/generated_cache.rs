//! Reused generated executables still obey Cargo invalidation and process isolation.
#[path = "support/generated_cache.rs"]
mod generated_cache;
use std::{
    fs,
    path::Path,
    process::Command,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn cached_projects_preserve_inputs_and_validate_rebuilds() -> Result<(), Box<dyn std::error::Error>>
{
    let root = std::env::temp_dir().join(format!(
        "hgl-cache-controls-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(root.join("dependency/src"))?;
    let manifest = "[package]\nname=\"cache-control\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[features]\nsecond=[]\ndefault=[]\n[dependencies]\nleaf={path=\"../../../dependency\"}\n";
    let project = generated_cache::Project::open(&root, 0)?;
    let source = project.path.join("src/main.rs");
    generated_cache::write(project.path.join("Cargo.toml"), manifest)?;
    generated_cache::write(
        root.join("dependency/Cargo.toml"),
        "[package]\nname=\"leaf\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    )?;
    generated_cache::write(root.join("dependency/src/lib.rs"), "pub const VALUE:i64=1;")?;
    let code = "fn main(){println!(\"{}:{}:{}:{}\",include_str!(\"../value.hgl\"),leaf::VALUE,env!(\"CARGO_PKG_VERSION\"),if cfg!(feature=\"second\"){2}else{1});}";
    generated_cache::write(&source, code)?;
    generated_cache::write(project.path.join("value.hgl"), "first")?;
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "first:1:0.1.0:1");
    fs::copy(project.path.join("Cargo.lock"), root.join("Cargo.lock"))?;
    generated_cache::workspace_lock(&root, &project.path)?;
    assert!(fresh(&project.path)?);

    let old = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    fs::File::open(&source)?.set_times(fs::FileTimes::new().set_modified(old))?;
    generated_cache::write(&source, code)?;
    assert_eq!(fs::metadata(&source)?.modified()?, old);
    assert!(fresh(&project.path)?);
    generated_cache::write(project.path.join("value.hgl"), "second")?;
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "second:1:0.1.0:1");
    generated_cache::write(root.join("dependency/src/lib.rs"), "pub const VALUE:i64=2;")?;
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "second:2:0.1.0:1");
    generated_cache::write(
        project.path.join("Cargo.toml"),
        manifest.replace("default=[]", "default=[\"second\"]"),
    )?;
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "second:2:0.1.0:2");
    generated_cache::write(&source, code.replace("leaf::VALUE", "leaf::VALUE+1"))?;
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "second:3:0.1.0:2");

    let workspace_lock = fs::read_to_string(root.join("Cargo.lock"))?.replace(
        "name = \"leaf\"\nversion = \"0.1.0\"",
        "name = \"leaf\"\nversion = \"0.2.0\"",
    );
    generated_cache::write(
        root.join("dependency/Cargo.toml"),
        "[package]\nname=\"leaf\"\nversion=\"0.2.0\"\nedition=\"2024\"\n",
    )?;
    generated_cache::write(root.join("Cargo.lock"), &workspace_lock)?;
    generated_cache::workspace_lock(&root, &project.path)?;
    assert_eq!(
        fs::read_to_string(project.path.join("Cargo.lock"))?,
        workspace_lock
    );
    assert!(!fresh(&project.path)?);
    assert_eq!(run(&project.path)?, "second:3:0.1.0:2");

    let competing = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(project.path.join(".lock"))?;
    assert!(competing.try_lock().is_err());
    drop(project);
    competing.try_lock()?;
    competing.unlock()?;
    drop(competing);
    fs::remove_dir_all(root)?;
    Ok(())
}
fn cargo(dir: &Path, command: &str) -> Result<std::process::Output, Box<dyn std::error::Error>> {
    let mut cargo = Command::new(env!("CARGO"));
    cargo
        .args([command, "--offline"])
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(dir);
    if !cfg!(debug_assertions) {
        cargo.arg("--release");
    }
    if command == "build" {
        cargo.arg("--message-format=json");
    } else {
        cargo.arg("--quiet");
    }
    let output = cargo.output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
fn fresh(dir: &Path) -> Result<bool, Box<dyn std::error::Error>> {
    let output = cargo(dir, "build")?;
    let text = String::from_utf8(output.stdout)?;
    let artifact = text
        .lines()
        .find(|line| {
            line.contains("\"reason\":\"compiler-artifact\"")
                && line.contains("\"name\":\"cache-control\"")
        })
        .ok_or("missing Cargo artifact")?;
    Ok(artifact.contains("\"fresh\":true"))
}
fn run(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    Ok(String::from_utf8(cargo(dir, "run")?.stdout)?
        .trim()
        .to_owned())
}
