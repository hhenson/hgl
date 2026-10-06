//! Generator continuation, timing and operand evaluation from the pinned spec.
use hgl_program::{compile, compile_tests, emit_rust, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[test]
fn generator_sources_execute_spec_and_scalar_payloads() -> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-generators-{}-{}",
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(dir.join("src"))?;
    let suite = compile_tests(&[
        (
            "generators.hgl".into(),
            include_str!("fixtures/generators.hgl").into(),
        ),
        (
            "pull-sources.hgl".into(),
            include_str!("../../../external/hgraph_spec/language/examples/pull-sources.hgl").into(),
        ),
        (
            "generator-yield-operands.hgl".into(),
            include_str!(
                "../../../external/hgraph_spec/language/examples/generator-yield-operands.hgl"
            )
            .into(),
        ),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ])?;
    fs::write(
        dir.join("src/evaluations.rs"),
        emit_tests(&suite).replace("fn main() {", "pub fn main() {"),
    )?;
    effects(&dir)?;
    manifest(&root, &dir)?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
            .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
            .env("CARGO_INCREMENTAL", "0")
            .args(profile)
            .current_dir(&dir)
            .output()?;
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"generator-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-semantics",
        "hgl-source",
        "hgl-testkit",
        "hgl-stdlib",
    ] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, dir))?;
    Ok(())
}

fn effects(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut runner = String::from(include_str!("fixtures/generator_effects_runner.rs"));
    let mut calls = String::from("fn main() { evaluations::main();\n");
    for (name, scenarios) in effect_cases().into_iter().chain(order_cases()) {
        let plan = compile(
            &[(
                "effects.hgl".into(),
                include_str!("fixtures/generator_effects.hgl").into(),
            )],
            &format!("run_{name}"),
        )?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
        writeln!(
            runner,
            "mod {name}; impl {name}::Native for Provider {{ fn mark_i64(value:i64)->Result<i64,Box<hgl_types::NodeError>> {{mark(value)}} }}"
        )?;
        writeln!(
            calls,
            "let mut registry=Registry::new(); {name}::register(&mut registry).unwrap(); let description={name}::main(&registry).unwrap();"
        )?;
        for (fail, end, trace, count, last, error, held) in scenarios {
            writeln!(
                calls,
                "for _ in 0..2 {{ run(&registry,&description,{fail},{end},{trace},{count},{last:?},{error:?},{held:?}); }}"
            )?;
        }
    }
    runner.push_str("mod evaluations;\n");
    runner.push_str(&calls);
    runner.push_str("}\n");
    fs::write(dir.join("src/main.rs"), runner)?;
    Ok(())
}

type Scenario = (
    i64,
    i64,
    i64,
    i64,
    Option<i64>,
    Option<&'static str>,
    Option<i64>,
);
const NEGATIVE: Option<&str> = Some("generator negative yield duration");
const ORDER: Option<&str> = Some("generator yield times must strictly increase");

fn effect_cases() -> Vec<(&'static str, Vec<Scenario>)> {
    vec![
        (
            "future",
            vec![
                (0, 10, 123, 2, Some(3), None, Some(3)),
                (0, 3, 12, 0, None, None, None),
                (1, 10, 1, 0, None, Some("marker failure"), None),
                (2, 10, 12, 0, None, Some("marker failure"), None),
            ],
        ),
        (
            "past",
            vec![
                (0, 10, 123, 1, Some(3), None, Some(3)),
                (2, 10, 12, 0, None, Some("marker failure"), None),
            ],
        ),
        (
            "negative",
            vec![
                (0, 10, 12, 0, None, NEGATIVE, None),
                (1, 10, 1, 0, None, Some("marker failure"), None),
                (2, 10, 12, 0, None, Some("marker failure"), None),
            ],
        ),
        ("negative_min", vec![(0, 10, 12, 0, None, NEGATIVE, None)]),
        ("zero", vec![(0, 10, 123, 1, Some(-1), None, Some(-1))]),
        (
            "implicit_overflow",
            vec![(
                0,
                10,
                12,
                0,
                None,
                Some("generator target time overflow"),
                None,
            )],
        ),
        (
            "explicit_overflow",
            vec![(0, 10, 1, 0, None, Some("time arithmetic overflow"), None)],
        ),
    ]
}
fn order_cases() -> Vec<(&'static str, Vec<Scenario>)> {
    vec![
        ("duplicate", vec![(0, 10, 1212, 0, None, ORDER, Some(7))]),
        (
            "past_equal",
            vec![
                (0, 10, 1212, 0, None, ORDER, None),
                (121, 10, 121, 0, None, Some("marker failure"), None),
                (1212, 10, 1212, 0, None, Some("marker failure"), None),
            ],
        ),
        ("past_decreasing", vec![(0, 10, 1212, 0, None, ORDER, None)]),
        (
            "past_increasing",
            vec![(0, 10, 12123, 1, Some(3), None, Some(3))],
        ),
        ("future_equal", vec![(0, 10, 1212, 0, None, ORDER, Some(7))]),
        (
            "future_decreasing",
            vec![(0, 10, 1212, 0, None, ORDER, Some(7))],
        ),
        (
            "resumed_negative",
            vec![
                (0, 10, 1212, 0, None, NEGATIVE, Some(7)),
                (1212, 10, 1212, 0, None, Some("marker failure"), Some(7)),
            ],
        ),
        ("resumed_zero", vec![(0, 10, 1212, 0, None, ORDER, Some(7))]),
        (
            "resumed_positive",
            vec![(0, 10, 12123, 2, Some(8), None, Some(8))],
        ),
    ]
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
