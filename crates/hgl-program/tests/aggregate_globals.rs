//! Spec-derived lexical aggregate borrowing and retained owning values.
use hgl_program::{compile, compile_tests, emit_rust, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

const TYPES: &str = "struct Box { amount:i64\ntext:str }\nstruct Other { amount:i64\ntext:str }\nstruct Envelope { inner:Box }";
const FIXTURE: &str = include_str!("fixtures/aggregate_globals.hgl");

fn checked(body: &str) -> Result<(), String> {
    let source = format!(
        "module borrows\n{TYPES}\nfn source()->i64 {{when {{return 1}}}}\nfn f(input:i64) {{ inject global_state\nwhen {{ {body} }} }}\nexport fn main() {{f(source())}}"
    );
    compile(&[("borrows.hgl".into(), source)], "main").map(|_| ())
}

#[test]
fn lexical_borrows_admit_readonly_aliases_and_disjoint_access() {
    for body in [
        "let a:Box=get(global_state,\"a\")\nlet b:Box=get(global_state,\"a\")\nlet alias=a\nlet n=alias.amount",
        "let a:Envelope=get(global_state,\"a\")\nlet child=a.inner\nlet alias=child\nlet n=alias.amount",
        "var a:Box=get(global_state,\"a\")\nvar b:Box=get(global_state,\"b\")\na.amount+=1\nb.amount+=2",
        "if true {var a:Box=get(global_state,\"a\")\na.amount+=1}\nset(global_state,\"a\",Box(amount:2,text:\"x\"))",
        "if input==1 {var a:Box=get(global_state,\"a\")} else {var b:Box=get(global_state,\"a\")}\nvar later:Box=get(global_state,\"a\")",
        "var a:Box=get(global_state,\"a\")\nlet n=a.amount\nvar copy=n\ncopy+=1\na.amount=copy",
        "var a:Box=get(global_state,\"a\")\na=Box(amount:a.amount+1,text:a.text)\na.amount+=1",
        "var own=Box(amount:0,text:\"own\")\nlet a:Box=get(global_state,\"a\")\nown=a\nown.amount+=1",
        "let a:Box=get(global_state,\"a\")\nset(global_state,\"b\",a)\nvar owned=Envelope(inner:a)\nowned.inner.amount+=1",
        "var a:Box=get(global_state,\"a\")\nset(global_state,\"b\",a)\nlet owned=Envelope(inner:a)\na.amount+=1",
        "var a:i64=get(global_state,\"a\")\nvar b:i64=get(global_state,\"a\")\na+=1\nb+=2\nset(global_state,\"a\",b)",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn borrowed_aggregates_cannot_escape_through_helper_calls() {
    for access in ["let", "var"] {
        for (ty, argument) in [("Box", "borrowed"), ("Envelope", "borrowed.inner")] {
            let source = format!(
                "module escape\n{TYPES}\nconst fn helper(value:Box)->i64 {{return value.amount}}\nfn source()->i64 {{when {{return 1}}}}\nfn f(input:i64) {{inject global_state\nwhen {{{access} borrowed:{ty}=get(global_state,\"a\")\nlet result=helper({argument})}}}}\nexport fn main() {{f(source())}}"
            );
            let error = compile(&[("escape.hgl".into(), source)], "main").unwrap_err();
            assert!(
                error.contains(
                    "borrowed global aggregate cannot escape through an ordinary helper call"
                ),
                "{error}"
            );
        }
    }
}

#[test]
fn borrowed_access_cannot_gain_authority_or_alias_exclusive_access() {
    for (body, expected) in [
        (
            "let a:Box=get(global_state,\"a\")\na.amount=2",
            "assignment requires writable var",
        ),
        (
            "let a:Box=get(global_state,\"a\")\na=Box(amount:2,text:\"x\")",
            "assignment requires writable var",
        ),
        (
            "let a:Envelope=get(global_state,\"a\")\na.inner.amount+=1",
            "increment requires a cache variable or writable var",
        ),
        (
            "let a:Box=get(global_state,\"a\")\nvar upgrade=a",
            "cannot upgrade a read-only global borrow",
        ),
        (
            "let a:Envelope=get(global_state,\"a\")\nvar upgrade=a.inner",
            "cannot upgrade a read-only global borrow",
        ),
        (
            "let a:Envelope=get(global_state,\"a\")\nlet child=a.inner\nvar upgrade=child",
            "cannot upgrade a read-only global borrow",
        ),
        (
            "var a:Box=get(global_state,\"a\")\nlet alias=a",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var a:Box=get(global_state,\"a\")\nvar alias=a",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var a:Envelope=get(global_state,\"a\")\nlet child=a.inner",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var a:Envelope=get(global_state,\"a\")\nvar child=a.inner",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var a:Box=get(global_state,\"a\")\na=Other(amount:1,text:\"x\")",
            "assignment type mismatch",
        ),
        (
            "var a:Envelope=get(global_state,\"a\")\na.inner=Other(amount:1,text:\"x\")",
            "assignment type mismatch",
        ),
    ] {
        let error = checked(body).unwrap_err();
        assert!(error.contains(expected), "{body}: {error}");
    }
}

#[test]
fn live_borrows_reject_conflicting_gets_and_replacement() {
    for body in [
        "var a:Box=get(global_state,\"a\")\nlet b:Box=get(global_state,\"a\")",
        "var a:Box=get(global_state,\"a\")\nvar b:Box=get(global_state,\"a\")",
        "let a:Box=get(global_state,\"a\")\nvar b:Box=get(global_state,\"a\")",
        "var a:Box=get(global_state,\"a\")\nset(global_state,\"a\",Box(amount:1,text:\"x\"))",
        "let a:Box=get(global_state,\"a\")\nset(global_state,\"a\",Box(amount:1,text:\"x\"))",
        "let a:Box=get(global_state,\"a\")\nlet alias=a\nset(global_state,\"a\",alias)",
        "var a:Box=get(global_state,\"a\")\nset(global_state,\"a\",a)",
        "let a:Box=get(global_state,\"a\")\nif true {var b:Box=get(global_state,\"a\")}",
        "var a:Box=get(global_state,\"a\")\nif true {let a=Box(amount:0,text:\"shadow\")\nset(global_state,\"a\",a)}",
    ] {
        let error = checked(body).unwrap_err();
        assert!(
            error.contains("conflicting access overlaps a lexical aggregate borrow"),
            "{body}: {error}"
        );
    }
}

#[test]
fn configured_key_aliases_and_nominal_type_conflicts_fail_before_start() {
    for right in ["left", "right"] {
        let source = format!(
            "module configured\n{TYPES}\nfn source()->i64 {{when {{return 1}}}}\nfn f(input:i64,const left:str,const right:str) {{inject global_state\nwhen {{var a:Box=get(global_state,left)\nset(global_state,right,Box(amount:1,text:\"x\"))}}}}\nexport fn main() {{f(source(),\"left\",\"{right}\")}}"
        );
        let result = compile(&[("configured.hgl".into(), source)], "main");
        if right == "right" {
            assert!(result.is_ok(), "{result:?}");
        } else {
            assert!(
                result
                    .unwrap_err()
                    .contains("conflicting access overlaps a lexical aggregate borrow")
            );
        }
    }
    for body in [
        "set(global_state,\"a\",Box(amount:1,text:\"x\"))\nset(global_state,\"a\",Other(amount:1,text:\"x\"))",
        "set(global_state,\"a\",1)\nlet a:Box=get(global_state,\"a\")",
        "let a:Box=get(global_state,\"a\")\nlet b:Other=get(global_state,\"a\")",
    ] {
        let error = checked(body).unwrap_err();
        assert!(error.contains("type conflict for key"), "{body}: {error}");
    }
}

#[test]
fn aggregate_global_source_runs_retention_borrows_failure_and_fresh_runs()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-aggregate-globals-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    for (entry, name) in [
        ("main", "graph"),
        ("fail_assign", "fail_assign"),
        ("fail_set", "fail_set"),
    ] {
        let plan = compile(&[("aggregate_globals.hgl".into(), FIXTURE.into())], entry)?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
    }
    evaluations(&dir)?;
    let missing = missing_values(&dir)?;
    fs::write(
        dir.join("src/main.rs"),
        format!(
            "{}\n{missing}",
            include_str!("fixtures/aggregate_globals_runner.rs")
        ),
    )?;
    manifest(&root, &dir)?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            .args(profile)
            .current_dir(&dir)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn evaluations(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let suite = compile_tests(&[
        ("aggregate_globals.hgl".into(), FIXTURE.into()),
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
    Ok(())
}

fn missing_values(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut modules = String::new();
    let mut calls = String::new();
    for phase in ["start", "when", "stop"] {
        for binding in ["let", "var"] {
            let name = format!("missing_{phase}_{binding}");
            let hook = if phase == "when" { "" } else { "when {}" };
            let source = format!(
                "module missing\n{TYPES}\nfn source()->i64 {{inject alarm\nstart {{schedule(alarm,0us)}}\nwhen {{return 1}}}}\nfn f(input:i64) {{inject global_state\n{phase} {{{binding} absent:Box=get(global_state,\"box\")}}\n{hook}}}\nexport fn main() {{f(source())}}"
            );
            let plan = compile(&[("missing.hgl".into(), source)], "main")?;
            fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
            writeln!(modules, "mod {name};")?;
            writeln!(
                calls,
                "let mut registry=Registry::new();\n{name}::register(&mut registry).unwrap();\nlet description={name}::main(&registry).unwrap();\nlet mut store=Store::new();\nstore.global_state().provision();\nlet mut built=instantiate_complete(&description,&registry,&mut store).unwrap();\nlet error=run_simulation(&mut built.graph,&mut store,&config()).unwrap_err();\nassert!(format!(\"{{error:?}}\").contains(\"box\"));"
            )?;
        }
    }
    Ok(format!("{modules}\nfn missing_values() {{ {calls} }}"))
}

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"aggregate-global-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in [
        "hgl-types",
        "hgl-store",
        "hgl-kernel",
        "hgl-describe",
        "hgl-harness",
        "hgl-harness-ir",
        "hgl-rust-ir",
        "hgl-source",
        "hgl-value-eval",
        "hgl-time-context",
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
    fs::write(dir.join("Cargo.toml"), manifest)?;
    Ok(())
}
