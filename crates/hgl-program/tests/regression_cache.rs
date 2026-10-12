//! Cache slots are reproducible across fresh processes and test scheduling.
#[path = "support/generated_cache.rs"]
mod generated_cache;
use std::{collections::BTreeSet, fs, process::Command, time::SystemTime};

#[test]
fn regression_slots_preserve_named_test_and_invocation_identity()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "hgl-regression-slots-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    let project = generated_cache::Project::open(&root, "slot-control")?;
    generated_cache::write(
        project.path.join("Cargo.toml"),
        "[package]\nname=\"slot-control\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n",
    )?;
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    generated_cache::workspace_lock(&workspace, &project.path)?;
    let support = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support");
    let code = format!(
        "#[path={:?}] mod generated_cache;\n#[path={:?}] mod regression_cache;\n{}",
        support.join("generated_cache.rs").to_string_lossy(),
        support.join("regression_cache.rs").to_string_lossy(),
        PROBE
    );
    generated_cache::write(project.path.join("src/main.rs"), code)?;
    let target = root.join("target/control-build");
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["build", "--offline", "--quiet"])
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_TARGET_DIR", &target)
        .current_dir(&project.path);
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        command.arg("--release");
        "release"
    };
    let built = command.output()?;
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
    );
    let binary = target
        .join(profile)
        .join(format!("slot-control{}", std::env::consts::EXE_SUFFIX));
    let run = |order: &str| -> Result<String, Box<dyn std::error::Error>> {
        let output = Command::new(&binary).arg(&root).arg(order).output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(String::from_utf8(output.stdout)?)
    };
    let first = run("forward")?;
    let second = run("reverse")?;
    assert_eq!(
        first, second,
        "thread scheduling must not change project identities"
    );
    assert_eq!(
        first.lines().collect::<BTreeSet<_>>().len(),
        4,
        "each named test owns two distinct invocation slots"
    );
    assert_eq!(
        fs::read_dir(root.join("target/source-test-projects"))?.count(),
        5,
        "repeated processes must reuse the same slots"
    );
    drop(project);
    fs::remove_dir_all(root)?;
    Ok(())
}

const PROBE: &str = r#"fn main()->Result<(),Box<dyn std::error::Error>> {
    let root=std::path::PathBuf::from(std::env::args().nth(1).ok_or("root")?);
    let order=if std::env::args().nth(2).as_deref()==Some("reverse") {["second","first"]} else {["first","second"]};
    let mut threads=Vec::new();
    for name in order {
        let root=root.clone();
        threads.push(std::thread::Builder::new().name(name.into()).spawn(move ||->std::io::Result<Vec<String>> {
            let mut names=Vec::new();
            for _ in 0..2 {
                let project=regression_cache::open(&root,"probe")?;
                names.push(project.path.file_name().unwrap().to_str().unwrap().to_owned());
            }
            Ok(names)
        })?);
    }
    let mut names=Vec::new();
    for thread in threads {names.extend(thread.join().unwrap()?);}
    names.sort();
    for name in names {println!("{name}");}
    Ok(())
}"#;
