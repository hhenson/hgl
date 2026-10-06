//! Bound eval captures use ordinary indexing and checked lexical presence guards.
use hgl_program::{compile_tests, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};
fn source(body: &str) -> Vec<(String, String)> {
    let body = body.replace(';', "\n");
    vec![
        (
            "captures.hgl".into(),
            format!(
                r"module captures
native const fn started(value:i64) throws
native const fn started(value:i64) throws {{}}
fn identity(value:i64)->i64 {{start {{started(1)}}
when {{return delta_value(value)}}}}
fn boolean(value:bool)->bool {{when {{return delta_value(value)}}}}
fn text(value:str)->str {{when {{return delta_value(value)}}}}
fn empty(value:i64) {{when {{}}}}
const fn positive(value:i64)->bool {{return value>0}}
test capture {{{body}}}"
            ),
        ),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}
#[test]
fn nullable_payload_and_lexical_scope_are_checked_before_execution() {
    for body in [
        "let result=eval(identity,[1]); let item=result[0]; assert item==1",
        "let result=eval(identity,[1]); let item=result[0]; var alias=item",
        "let result=eval(identity,[1]); let item=result[0]; assert positive(item)",
        "let result=eval(identity,[1]); let item=result[0]; let alias=item; if item!=null {assert alias==1}",
        "let result=eval(identity,[1]); let item=result[0]; if item!=null {assert item==1}; assert item==1",
        "let result=eval(identity,[1]); if true {let item=result[0]}; assert item!=null",
        "let result=eval(identity,[1]); if true {let item=result[0]; let item=result[0]}",
        "var result=eval(identity,[1])",
        "let result=eval(empty,[1])",
    ] {
        assert!(compile_tests(&source(body)).is_err(), "{body}");
    }
}
const CASES: &[(&str, &str, bool, usize, &str)] = &[
    (
        "nested",
        r"let result=eval(identity,[0,_,2])
assert len(result)==3
let first=result[0]
let missing=result[1]
let last=result[2]
if first!=null {if last!=null {assert first!=last} else {assert false}} else {assert false}
assert missing==null
if missing==null {assert first!=null} else {assert false}
if first==null {assert false} else {assert first==0}
assert last==null || positive(last)
assert first!=null && first==0",
        true,
        1,
        "1 executed tests, 8 evaluations, 0 failures",
    ),
    (
        "scope",
        r"let result=eval(identity,[1])
var count=0
if true {let result=eval(identity,[2]); count+=1
let item=result[0]
if item!=null {assert item==2} else {assert false}}
assert count==1
let item=result[0]
if item!=null {assert item==1} else {assert false}
if false {let invalid=@[Missing/Skipped]; eval(identity,[3]); assert false}
else if true {assert true} else {assert false}",
        true,
        2,
        "1 executed tests, 6 evaluations, 0 failures",
    ),
    (
        "false_payload",
        r#"let result=eval(boolean,[false]);let item=result[0]
if item!=null {assert !item} else {assert false}
let words=eval(text,[""]);let word=words[0]
if word!=null {assert word==""} else {assert false}"#,
        true,
        0,
        "0 failures",
    ),
    (
        "failure",
        "let result=eval(identity,[1]);let item=result[0];if item!=null {assert false};eval(identity,[2])",
        false,
        1,
        "ordinary assertion failed",
    ),
    (
        "upper_bound",
        "let result=eval(identity,[1]);let item=result[1];assert item==null",
        false,
        1,
        "out of bounds",
    ),
    (
        "negative_bound",
        "let result=eval(identity,[1]);let item=result[-1];assert item==null",
        false,
        1,
        "out of bounds",
    ),
];
#[test]
fn emitted_branches_bind_owned_captures_and_report_only_executed_steps()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-captures-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    let mut modules = String::new();
    let mut calls = String::new();
    for (name, body, _, _, _) in CASES {
        let suite = compile_tests(&source(body))?;
        fs::write(
            dir.join(format!("src/{name}.rs")),
            emit_tests(&suite).replace("fn main()", "pub fn main()"),
        )?;
        writeln!(modules, "mod {name};")?;
        writeln!(calls, "Some({name:?})=>{name}::main(),")?;
    }
    fs::write(
        dir.join("src/main.rs"),
        format!(
            "{modules}\nstruct Provider;\nmod native {{pub fn started_i64(_:i64)->hgl_types::NodeResult {{println!(\"STARTED\");Ok(())}}}}\nfn main() {{match std::env::args().nth(1).as_deref() {{{calls}_=>panic!(\"unknown test\")}}}}"
        ),
    )?;
    manifest(&root, &dir)?;
    // The gate runs the suite in both profiles; each build follows the profile of this test binary.
    let release = !cfg!(debug_assertions);
    let mut build = Command::new(env!("CARGO"));
    build
        .args(["build", "--offline", "--quiet"])
        // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
        .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(&dir);
    if release {
        build.arg("--release");
    }
    let output = build.output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let binary = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/source-tests"
    ))
    .join(if release { "release" } else { "debug" })
    .join(format!(
        "captured-results-{suffix}{}",
        std::env::consts::EXE_SUFFIX
    ));
    for (name, _, success, starts, diagnostic) in CASES {
        let output = Command::new(&binary).arg(name).output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.success(), *success, "{name}: {text}");
        assert_eq!(text.matches("STARTED").count(), *starts, "{name}: {text}");
        assert!(text.contains(diagnostic), "{name}: {text}");
    }

    fs::remove_dir_all(dir)?;
    Ok(())
}
fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut text = String::from(
        "[package]\nname=\"captured-results\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-testkit",
        "hgl-semantics",
        "hgl-source",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .into_owned();
        writeln!(text, "{name}={{path={path:?}}}")?;
    }
    fs::write(dir.join("Cargo.toml"), unique_package(&text, dir))?;
    Ok(())
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
