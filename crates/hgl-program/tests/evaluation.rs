//! Run the unmodified shared suite and probe the harness independently of it.
use hgl_program::{compile_tests, compile_tests_files, emit_tests};
use std::{
    fs,
    path::Path,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn source(body: &str) -> Vec<(String, String)> {
    vec![("eval.hgl".into(), format!("module example\n{body}"))]
}

#[test]
fn eval_reports_type_and_phase_errors_before_emission() {
    for (body, diagnostic) in [
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { assert eval(id, [true]) == [1] }",
            "expected i64",
        ),
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { assert eval(id, [1]) == [true] }",
            "expected output",
        ),
        (
            "fn id(x: i64) -> i64 { when { return x } }\ntest bad { eval(id, 1) }",
            "requires a sequence",
        ),
        (
            "fn sink(x: i64) { when {} }\ntest bad { assert eval(sink, [1]) == [1] }",
            "outputless",
        ),
        ("test { export fn helper() {} }", "test context"),
        ("test { use other }", "test context"),
        (
            "test { native const fn native(x: i64) -> i64 }",
            "test context",
        ),
        ("test { test { fn nested() {} } }", "test context"),
        (
            "fn bad(x: i64) -> i64 { inject out\n when { passivate(out)\n return x } }\ntest bad_call { eval(bad, [1]) }",
            "activity requires",
        ),
        (
            "fn bad(x: i64) -> datetime { when { return last_modified(x, x) } }\ntest bad_call { eval(bad, [1]) }",
            "exactly one",
        ),
    ] {
        let error = compile_tests(&source(body)).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
}

#[test]
fn production_cannot_see_test_helpers() {
    let input = source(
        "test { fn helper(x: i64) -> i64 { when { return x } } }\nexport fn main() { helper(1) }",
    );
    assert!(hgl_program::compile(&input, "main").is_err());
}

#[test]
fn actual_stdlib_and_harness_regressions_run_on_rust() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let library = root.join("external/hgraph_std/hgl/hgraph");
    let mut parts = fs::read_dir(library.join("tests"))?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    parts.retain(|p| p.extension().is_some_and(|e| e == "hgl"));
    parts.sort();
    parts.push(root.join("native/stdlib/rust.hgl"));
    let suite = compile_tests_files(&parts, &[library])?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-eval-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    module(&dir, "standard", &emit_tests(&suite))?;
    module(
        &dir,
        "regression",
        &emit_tests(&compile_tests(&source(REGRESSIONS))?),
    )?;
    for (name, expected) in [("wrong", "[2]"), ("long", "[1, _]"), ("short", "[]")] {
        let input = source(&format!(
            "fn id(x: i64) -> i64 {{ when {{ return x }} }}\ntest mismatch {{ assert eval(id, [1]) == {expected} }}"
        ));
        module(&dir, name, &emit_tests(&compile_tests(&input)?))?;
    }
    let throwing = source(
        "native const fn raise_error(message: str) throws\nnative const fn raise_error(message: str) throws {}\nfn fail(ts: i64) -> i64 { when { raise_error(\"deliberate node failure\")\nreturn ts } }\ntest fails { eval(fail, [1]) }",
    );
    module(&dir, "throwing", &emit_tests(&compile_tests(&throwing)?))?;
    manifest(&root, &dir)?;
    fs::write(
        dir.join("src/main.rs"),
        r#"
struct Provider;
mod native { pub use hgl_std_native::*; }
mod standard; mod regression; mod wrong; mod long; mod short; mod throwing;
fn main() { match std::env::args().nth(1).as_deref() {
Some("throwing") => throwing::main(), Some("standard") => standard::main(), Some("regression") => regression::main(),
Some("wrong") => wrong::main(), Some("long") => long::main(), Some("short") => short::main(),
_ => panic!("unknown test image") } }
"#,
    )?;
    let build = Command::new(env!("CARGO"))
        .args(["build", "--offline", "--quiet"])
        .current_dir(&dir)
        .output()?;
    assert!(
        build.status.success(),
        "{}",
        String::from_utf8_lossy(&build.stderr)
    );
    let binary = dir
        .join("target/debug")
        .join(format!("eval-regressions{}", std::env::consts::EXE_SUFFIX));
    for (name, success, message) in [
        ("standard", true, "45 tests, 84 evaluations, 0 failures"),
        ("regression", true, "0 failures"),
        ("wrong", false, "cycle 0"),
        ("long", false, "cycle 1"),
        ("short", false, "cycle 0"),
        ("throwing", false, "deliberate node failure"),
    ] {
        let output = Command::new(&binary).arg(name).output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.success(), success, "{name}: {text}");
        assert!(text.contains(message), "{name}: {text}");
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}
fn module(dir: &Path, name: &str, code: &str) -> std::io::Result<()> {
    fs::write(
        dir.join(format!("src/{name}.rs")),
        code.replace("fn main() {", "pub fn main() {"),
    )
}
fn manifest(root: &Path, dir: &Path) -> std::io::Result<()> {
    let mut lines = vec!["[package]\nname=\"eval-regressions\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]".to_owned()];
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-testkit",
        "hgl-std-native",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        lines.push(format!("{name}={{path=\"{path}\"}}"));
    }
    fs::write(dir.join("Cargo.toml"), lines.join("\n"))
}
const REGRESSIONS: &str = r#"
fn id(x: i64) -> i64 { when { return x } }
fn floating(x: f64) -> f64 { when { return x } }
fn counter(x: i64) -> i64 {
    state count: i64 = 0
    when { count += 1
        return count }
}
fn delayed(const value: i64, const delay: duration) -> i64 {
    inject alarm
    start { alarm.schedule(delay) }
    when { return value }
}
fn sink(x: i64) { when {} }
fn production(x: i64) -> i64 { return id(x) }
test {
    fn id(x: i64) -> i64 { when { return x + 10 } }
    fn forward(x: i64) -> i64 { return helper(x) }
}
test {
    fn helper(x: i64) -> i64 { when { return x } }
}
test silent { assert eval(helper, [_, _, _]) == [_, _, _] }
test empty { assert eval(helper, []) == [] }
test same_value_ticks { assert eval(helper, [1, _, 1, _]) == [1, _, 1, _] }
test promoted { assert eval(floating, [1, _, 2.5]) == [1, _, 2.5] }
test state_is_fresh {
    assert eval(counter, [1, 1, _, 1]) == [1, 2, _, 3]
    assert eval(counter, [1]) == [1]
}
test output_horizon { assert eval(delayed, 7, 3us) == [_, _, _, 7] }
test fixed_expression { assert eval(delayed, 2 + 3, 1us + 1us) == [_, _, 5] }
test helper_shadow { assert eval(id, [1]) == [11] }
test production_scope { assert eval(production, [1]) == [1] }
test forward_scope { assert eval(forward, [1]) == [1] }
test outputless { eval(sink, [1, _, 1]) }
fn text_state(ts: str) -> str {
    state saved: str = "initial"
    when { saved = ts
        return saved }
}
fn text_dedup(ts: str) -> str {
    inject out
    when { if !valid(out) || ts != out { return ts } }
}
test text_values { assert eval(text_state, ["a", _, "b"]) == ["a", _, "b"]
    assert eval(text_dedup, ["a", "a", _, "b"]) == ["a", _, _, "b"] }

"#;
