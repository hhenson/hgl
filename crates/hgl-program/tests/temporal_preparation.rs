//! Execute provider validation and lexical preparation through generated HGL tests.
use hgl_program::{compile_tests, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn source(body: &str) -> Vec<(String, String)> {
    lifted_defaults(vec![
        (
            "preparation.hgl".into(),
            format!(
                r#"module preparation
native const fn start_marker(value:i64)->i64 throws
native const fn start_marker(value:i64)->i64 throws {{}}
fn target(value:timezone,const ignored:timezone)->timezone {{
    start {{ start_marker(1) }}
    when {{return delta_value(value)}}
}}
fn invalid_default(value:timezone,const ignored:timezone = @[Missing/Default])->timezone {{
    start {{ start_marker(1) }}
    when {{return delta_value(value)}}
}}
fn valid_default(value:timezone,const zone:timezone = @[US/Eastern])->timezone {{
    start {{ start_marker(1) }}
    when {{return zone}}
}}
fn alias_composition(value:timezone,const zone:timezone)->timezone {{
    let alias=zone
    return valid_default(value,zone:alias)
}}
const fn choose(flag:bool)->timezone {{
    if flag {{return @[UTC]}}
    return @[Missing/SkippedHelper]
}}
fn clock_target(value:zoned_time,const ignored:zoned_time = @09:30[UTC])->zoned_time {{
    start {{ start_marker(1) }}
    when {{ return delta_value(value) }}
}}
fn clock_default(value:zoned_time,const ignored:zoned_time = @09:30[Missing/Default])->zoned_time {{
    start {{ start_marker(1) }}
    when {{ return delta_value(value) }}
}}
fn hook_key(value:i64)->map<str,i64> {{
    start {{ start_marker(1) }}
    when {{let key="retained"
    let alias=key
    return delta<map<str,i64>>(upsert:[alias:value])}}
}}
fn configured_key(value:i64,const key:str)->map<str,i64> {{
    start {{ start_marker(1) }}
    when {{let alias=key
    return delta<map<str,i64>>(upsert:[alias:value])}}
}}
fn nested_hook_key(value:i64)->map<str,map<str,i64>> {{
    start {{ start_marker(1) }}
    when {{let outer="outer"
    let inner="inner"
    return delta<map<str,map<str,i64>>>(upsert:[outer:delta<map<str,i64>>(upsert:[inner:value])])}}
}}
fn key_target(value:map<timezone,i64>)->map<timezone,i64> {{
    start {{ start_marker(1) }}
    when {{ return delta_value(value) }}
}}
fn set_target(value:set<zoned_time>)->set<zoned_time> {{
    start {{ start_marker(1) }}
    when {{ return delta_value(value) }}
}}
fn nested_key_target(value:map<timezone,set<zoned_time>>)->map<timezone,set<zoned_time>> {{
    start {{ start_marker(1) }}
    when {{ return delta_value(value) }}
}}
test ordered {{ {body} }}
"#
            ),
        ),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ])
}

fn lifted_defaults(mut sources: Vec<(String, String)>) -> Vec<(String, String)> {
    sources[0].1.push_str(r"
use default_scope::{scoped_default}
const fn seed()->timezone => @[Missing/Caller]
const fn lifted_default(value:i64,zone:timezone = @[UTC])->timezone => zone
const fn lifted_unused(value:i64,zone:timezone = @[Missing/Default])->i64 {
    return start_marker(value)
}
const fn lifted_order(value:i64,first:timezone = @[Missing/FirstDefault],second:timezone = @[Missing/SecondDefault])->i64 => value
");
    sources.push((
            "default_scope.hgl".into(),
            "module default_scope\nconst fn seed()->i64 => 7\nexport const fn scoped_default(value:i64,zone:timezone=@[UTC],offset:i64=seed())->i64 => value+offset".into(),
        ));
    sources
}
const CASES: [(&str, &str, bool, &str, usize); 36] = [
    (
        "lifted_default",
        "assert eval(const(lifted_default),value:[1,2]) == [@[UTC],@[UTC]]",
        true,
        "0 failures",
        0,
    ),
    (
        "lifted_unused",
        "eval(const(lifted_unused),value:[1,2])",
        false,
        "Missing/Default",
        0,
    ),
    (
        "lifted_supplied",
        "assert eval(const(lifted_unused),value:[1,2],zone:@[UTC]) == [1,2]",
        true,
        "0 failures",
        2,
    ),
    (
        "lifted_default_order",
        "eval(const(lifted_order),value:[1])",
        false,
        "Missing/FirstDefault",
        0,
    ),
    (
        "lifted_written_order",
        "eval(const(lifted_order),value:[1],second:@[Missing/Written])",
        false,
        "Missing/Written",
        0,
    ),
    (
        "lifted_declaring_scope",
        "assert eval(const(scoped_default),value:[1,2]) == [8,9]",
        true,
        "0 failures",
        0,
    ),
    (
        "hook_key",
        r#"assert eval(hook_key,value:[1,2]) == [delta<map<str,i64>>(upsert:["retained":1]),delta<map<str,i64>>(upsert:["retained":2])]"#,
        true,
        "0 failures",
        1,
    ),
    (
        "configured_key",
        r#"assert eval(configured_key,value:[1,2],key:"configured") == [delta<map<str,i64>>(upsert:["configured":1]),delta<map<str,i64>>(upsert:["configured":2])]"#,
        true,
        "0 failures",
        1,
    ),
    (
        "nested_hook_key",
        r#"assert eval(nested_hook_key,value:[1,2]) == [delta<map<str,map<str,i64>>>(upsert:["outer":delta<map<str,i64>>(upsert:["inner":1])]),delta<map<str,map<str,i64>>>(upsert:["outer":delta<map<str,i64>>(upsert:["inner":2])])]"#,
        true,
        "0 failures",
        1,
    ),
    (
        "fresh_key_recipes",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[UTC]:1])])
       eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[UTC]:2])])",
        true,
        "0 failures",
        2,
    ),
    (
        "retained_alias",
        r"let key=@[UTC]
       let alias=key
       assert eval(key_target,value:[delta<map<timezone,i64>>(upsert:[alias:1])]) == [delta<map<timezone,i64>>(upsert:[key:1])]
       assert eval(key_target,value:[delta<map<timezone,i64>>(upsert:[key:2])]) == [delta<map<timezone,i64>>(upsert:[alias:2])]",
        true,
        "0 failures",
        2,
    ),
    (
        "alias_duplicate",
        r"let key=@[UTC]
       let alias=key
       eval(key_target,value:[delta<map<timezone,i64>>(upsert:[key:1,alias:2])])",
        false,
        "duplicate",
        0,
    ),
    (
        "alias_overlap",
        r"let key=@[UTC]
       eval(key_target,value:[delta<map<timezone,i64>>(upsert:[key:1],remove:[key])])",
        false,
        "overlap",
        0,
    ),
    (
        "unused_alias",
        r"let key=@[Missing/Unused]
       let alias=key
       eval(key_target,value:[delta<map<timezone,i64>>(upsert:[alias:1])])",
        false,
        "Missing/Unused",
        0,
    ),
    (
        "nested_key_domains",
        r"assert eval(nested_key_target,value:[delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[US/Eastern]])]),delta<map<timezone,set<zoned_time>>>(remove:[@[UTC]]),delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[America/New_York]])])]) == [delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[US/Eastern]])]),delta<map<timezone,set<zoned_time>>>(remove:[@[UTC]]),delta<map<timezone,set<zoned_time>>>(upsert:[@[UTC]:delta<set<zoned_time>>(added:[@09:30[America/New_York]])])]",
        true,
        "",
        1,
    ),
    (
        "key_missing",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[Missing/Key]:1])])",
        false,
        "Missing/Key",
        0,
    ),
    (
        "key_case",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[america/new_york]:1])])",
        false,
        "america/new_york",
        0,
    ),
    (
        "key_duplicate",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[UTC]:1,@[UTC]:2])])",
        false,
        "duplicate",
        0,
    ),
    (
        "key_overlap",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[UTC]:1],remove:[@[UTC]])])",
        false,
        "overlap",
        0,
    ),
    (
        "set_duplicate",
        r"eval(set_target,value:[delta<set<zoned_time>>(added:[@09:30[UTC],@09:30[UTC]])])",
        false,
        "duplicate",
        0,
    ),
    (
        "key_aliases",
        r"eval(key_target,value:[delta<map<timezone,i64>>(upsert:[@[US/Eastern]:1,@[America/New_York]:2])])",
        true,
        "",
        1,
    ),
    (
        "clock_dense",
        r"eval(clock_target,value:[@09:30[america/new_york]])",
        false,
        "america/new_york",
        0,
    ),
    (
        "clock_const",
        r"eval(clock_target,value:[@09:30[UTC]],ignored:@09:30[Missing/Const])",
        false,
        "Missing/Const",
        0,
    ),
    (
        "clock_default",
        r"eval(clock_default,value:[@09:30[UTC]])",
        false,
        "Missing/Default",
        0,
    ),
    (
        "clock_expected",
        r"assert eval(clock_target,value:[@09:30[UTC]]) == [@09:30[Missing/Expected]]",
        false,
        "Missing/Expected",
        1,
    ),
    (
        "clock_valid",
        r"let clock=@09:30:00.123456[US/Eastern]
        assert eval(clock_target,value:[clock,clock,_,@09:30:00.123457[America/New_York]]) == [clock,clock,_,@09:30:00.123457[America/New_York]]",
        true,
        "0 failures",
        1,
    ),
    (
        "unused_default",
        r"eval(invalid_default,value:[@[UTC]])",
        false,
        "Missing/Default",
        0,
    ),
    (
        "captured_default",
        r"assert eval(valid_default,value:[@[UTC],_,@[Etc/UTC]]) == [@[US/Eastern],_,@[US/Eastern]]",
        true,
        "0 failures",
        1,
    ),
    (
        "composition_alias",
        r"let zone=@[Etc/UTC]
            assert eval(alias_composition,value:[@[UTC]],zone:zone) == [zone]",
        true,
        "0 failures",
        1,
    ),
    (
        "dense_first",
        r"eval(target,value:[@[Missing/Dense]],ignored:@[Missing/Const])",
        false,
        "Missing/Dense",
        0,
    ),
    (
        "const_first",
        r"eval(target,ignored:@[Missing/Const],value:[@[Missing/Dense]])",
        false,
        "Missing/Const",
        0,
    ),
    (
        "unused",
        r"let unused=@[Missing/Unused]
            eval(target,value:[@[UTC]],ignored:@[UTC])",
        false,
        "Missing/Unused",
        0,
    ),
    (
        "expected",
        r"assert eval(target,value:[@[UTC]],ignored:@[UTC]) == [@[Missing/Expected]]",
        false,
        "Missing/Expected",
        1,
    ),
    (
        "skipped",
        r"if false {let unused=@[Missing/Skipped]}
            assert eval(target,value:[choose(true)],ignored:@[UTC]) == [@[UTC]]",
        true,
        "0 failures",
        1,
    ),
    (
        "civil_order",
        r"assert @2024-02-29T12:30 < @2024-02-29T12:31
            assert @2024-02-29T12:31 >= @2024-02-29T12:31",
        true,
        "0 failures",
        0,
    ),
    (
        "fresh",
        r"var zone=@[UTC]
            assert eval(target,value:[zone],ignored:zone) == [zone]
            zone=@[US/Eastern]
            assert eval(target,value:[zone],ignored:zone) == [zone]",
        true,
        "0 failures",
        2,
    ),
];
#[test]
fn generated_tests_validate_before_start_and_expectations_after_run()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = scratch_dir("hgl-temporal-preparation")?;
    fs::create_dir_all(dir.join("src"))?;
    let mut modules = String::new();
    let mut calls = String::new();
    for (name, body, _, _, _) in CASES {
        let suite = compile_tests(&source(body))?;
        let generated = emit_tests(&suite);
        assert!(generated.contains("self.0.materialize(recipe)"));
        fs::write(
            dir.join(format!("src/{name}.rs")),
            generated.replace("fn main()", "pub fn main()").replace(
                "self.0.materialize(recipe)",
                "{println!(\"MATERIALIZED\");self.0.materialize(recipe)}",
            ),
        )?;
        writeln!(modules, "mod {name};")?;
        writeln!(calls, "Some({name:?})=>{name}::main(),")?;
    }
    fs::write(
        dir.join("src/main.rs"),
        format!(
            "{modules}\nstruct Provider;\nmod native {{pub fn start_marker_i64(value:i64)->hgl_types::NodeResult<i64> {{println!(\"TARGET_STARTED\");Ok(value)}}}}\nfn main() {{match std::env::args().nth(1).as_deref() {{{calls}_=>panic!(\"unknown test\")}}}}"
        ),
    )?;
    manifest(&root, &dir)?;
    // The gate runs the suite in both profiles; each build follows the profile of this test binary.
    let release = !cfg!(debug_assertions);
    let mut command = Command::new(env!("CARGO"));
    command
        .args(["build", "--offline", "--quiet"])
        // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
        .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
        .env("CARGO_INCREMENTAL", "0")
        .current_dir(&dir);
    if release {
        command.arg("--release");
    }
    let built = command.output()?;
    assert!(
        built.status.success(),
        "{}",
        String::from_utf8_lossy(&built.stderr)
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
        "temporal-preparation-{suffix}{}",
        std::env::consts::EXE_SUFFIX
    ));
    for (name, _, success, diagnostic, starts) in CASES {
        let output = Command::new(&binary).arg(name).output()?;
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.status.success(), success, "{name}: {text}");
        assert!(text.contains(diagnostic), "{name}: {text}");
        materializations(name, &text);
        assert_eq!(
            text.matches("TARGET_STARTED").count(),
            starts,
            "{name}: {text}"
        );
    }

    fs::remove_dir_all(dir)?;
    Ok(())
}

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"temporal-preparation\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(dir.join("Cargo.toml"), unique_package(&manifest, dir))?;
    Ok(())
}

#[test]
fn contextual_composition_construction_requires_run_preparation() {
    for (call, diagnostic) in [
        (
            "valid_default(value)",
            "contextual temporal defaults in composition",
        ),
        (
            "valid_default(value,zone:@[UTC])",
            "requires run preparation",
        ),
    ] {
        let mut sources = source("eval(unprepared,value:[@[UTC]])");
        writeln!(
            sources[0].1,
            "fn unprepared(value:timezone)->timezone => {call}"
        )
        .unwrap();
        let error = compile_tests(&sources).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
}

fn materializations(name: &str, text: &str) {
    if name.starts_with("lifted_") {
        let expected = if name == "lifted_default" {
            3 // One retained default, then the two expected values after execution.
        } else {
            1 // Failures stop preparation; supplied configuration is retained once.
        };
        assert_eq!(
            text.matches("MATERIALIZED").count(),
            expected,
            "{name}: {text}"
        );
    }
    if [
        "retained_alias",
        "alias_duplicate",
        "alias_overlap",
        "unused_alias",
        "fresh_key_recipes",
    ]
    .contains(&name)
    {
        assert_eq!(
            text.matches("MATERIALIZED").count(),
            if name == "fresh_key_recipes" { 2 } else { 1 },
            "{name}: {text}"
        );
    }
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}

/// A directory of this test's own under the temp root; see `NEXT_DIR`.
fn scratch_dir(prefix: &str) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let tick = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let serial = NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(std::env::temp_dir().join(format!("{prefix}-{}-{tick}-{serial}", std::process::id())))
}
