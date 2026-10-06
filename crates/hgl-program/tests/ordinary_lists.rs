//! Ordinary list contracts exercised through source checking and emitted execution.
use hgl_program::{compile, compile_tests, emit_rust, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const FIXTURE: &str = include_str!("fixtures/ordinary_lists.hgl");
const PHASES: &str = include_str!("fixtures/ordinary_list_phases.hgl");
const TYPES: &str = "struct Sample { amount:i64 }\nstruct Bucket { values:list<i64> }";

fn checked(body: &str) -> Result<(), String> {
    let source = format!(
        "module lists\n{TYPES}\nfn source()->i64 {{when {{return 1}}}}\nfn f(input:i64) {{inject global_state\nwhen {{{body}}}}}\nexport fn main() {{f(source())}}"
    );
    compile(&[("lists.hgl".into(), source)], "main").map(|_| ())
}

#[test]
fn list_context_shape_and_owning_authority_are_checked() {
    for body in [
        "var xs:list<i64> =[]\npush(xs,1)\nlet n=len(xs)\nlet first=xs[0]",
        "let fixed:list<i64,0> =[]\nlet n=len(fixed)",
        "let fixed:list<i64,2> =[1,2]\nlet n=fixed[1]",
        "var xs:list<i64,unbounded> =[]\nvar same:list<i64> =xs\npush(same,1)",
        "var xs:list<i64> =[1,2]\nxs=[]",
        "let xs:list<i64> =[]\nvar independent=xs\npush(independent,1)",
        "var bucket=Bucket(values:[])\npush(bucket.values,1)",
        "var samples:list<Sample> =[]\npush(samples,Sample(amount:1))\nsamples[0].amount=2",
        "var child:list<i64> =[1]\nvar outer:list<list<i64>> =[]\npush(outer,child)\npush(outer,outer[0])\npush(outer[0],2)",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    for (body, expected) in [
        ("var xs=[]", "expected concrete list type"),
        ("var xs:list<i64,2> =[]", "fixed list size mismatch"),
        ("let xs:list<i64,2> =[1]", "fixed list size mismatch"),
        (
            "var xs:list<i64> =[]\npush(xs,1.0)",
            "push ordinary list element type mismatch",
        ),
        (
            "var xs:list<i64> =[1]\nlet n=xs[1.0]",
            "ordinary list index requires i64",
        ),
        (
            "let xs:list<i64> =[]\npush(xs,1)",
            "push requires writable ordinary list access",
        ),
        (
            "var xs:list<i64,0> =[]\npush(xs,1)",
            "push requires an unbounded ordinary list",
        ),
        (
            "var xs:list<i64,2> =[1,2]\npush(xs,3)",
            "push requires an unbounded ordinary list",
        ),
        (
            "var xs:list<i64> =[]\nlet result=push(xs,1)",
            "statement operation has no initializer value",
        ),
        (
            "var xs:list<i64> =[1]\nxs[0]=2",
            "indexed replacement is not admitted",
        ),
        (
            "var xs:list<i64> =[1]\nxs[0]+=2",
            "indexed replacement is not admitted",
        ),
        (
            "var xs:list<i64> =[]\nlet fixed:list<i64,0> =[]\nxs=fixed",
            "assignment type mismatch",
        ),
        (
            "let xs:list<i64> =[input]",
            "ordinary nonempty list literals require constant elements",
        ),
        (
            "let bucket=Bucket(values:[])\npush(bucket.values,1)",
            "push requires writable ordinary list access",
        ),
        (
            "var original:list<Sample> =[]\npush(original,Sample(amount:1))\nlet samples=original\nsamples[0].amount=2",
            "assignment requires writable var",
        ),
        (
            "let outer:list<list<i64>> =[]\npush(outer[0],1)",
            "push requires writable ordinary list access",
        ),
    ] {
        let error = checked(body).expect_err(body);
        assert!(
            error.contains(expected),
            "{body}: expected {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn list_borrows_preserve_alias_authority_and_lexical_conflicts() {
    for body in [
        "var xs:list<i64> =get(global_state,\"a\")\nlet n=len(xs)\nlet first=xs[0]\npush(xs,first)\npush(xs,xs[0])",
        "let xs:list<list<i64>> =get(global_state,\"a\")\nlet child=xs[0]\nlet alias=child\nlet n=len(alias)",
        "var xs:list<list<i64>> =get(global_state,\"a\")\npush(xs,xs[0])\npush(xs[0],1)",
        "var owned:list<i64> =[]\nlet xs:list<list<i64>> =get(global_state,\"a\")\nowned=xs[0]\npush(owned,1)",
        "if true {var xs:list<i64> =get(global_state,\"a\")\npush(xs,1)}\nlet xs:list<i64> =get(global_state,\"a\")",
        "var a:list<i64> =get(global_state,\"a\")\nvar b:list<i64> =get(global_state,\"b\")\npush(a,1)\npush(b,2)",
        "let fixed:list<i64,2> =[1,2]\nset(global_state,\"a\",fixed)\nlet borrowed:list<i64,2> =get(global_state,\"a\")\nlet n=len(borrowed)",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    for (body, expected) in [
        (
            "let xs:list<i64> =get(global_state,\"a\")\npush(xs,1)",
            "push requires writable ordinary list access",
        ),
        (
            "let xs:list<list<i64>> =get(global_state,\"a\")\nlet child=xs[0]\npush(child,1)",
            "push requires writable ordinary list access",
        ),
        (
            "let xs:list<list<i64>> =get(global_state,\"a\")\nvar child=xs[0]",
            "cannot upgrade a read-only global borrow",
        ),
        (
            "var xs:list<list<i64>> =get(global_state,\"a\")\nlet child=xs[0]",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var xs:list<list<i64>> =get(global_state,\"a\")\nvar child=xs[0]",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var xs:list<Sample> =get(global_state,\"a\")\nlet child=xs[0]",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "var xs:list<i64> =get(global_state,\"a\")\nlet again:list<i64> =get(global_state,\"a\")",
            "conflicting access overlaps a lexical aggregate borrow",
        ),
        (
            "var xs:list<i64> =get(global_state,\"a\")\nset(global_state,\"a\",xs)",
            "conflicting access overlaps a lexical aggregate borrow",
        ),
        (
            "let xs:list<i64> =get(global_state,\"a\")\nset(global_state,\"a\",xs)",
            "conflicting access overlaps a lexical aggregate borrow",
        ),
        (
            "let fixed:list<i64,2> =[1,2]\nset(global_state,\"a\",fixed)\nlet wrong:list<i64> =get(global_state,\"a\")",
            "global_state: type conflict for key",
        ),
        (
            "let bucket:Bucket=get(global_state,\"a\")\npush(bucket.values,1)",
            "push requires writable ordinary list access",
        ),
    ] {
        let error = checked(body).expect_err(body);
        assert!(
            error.contains(expected),
            "{body}: expected {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn ordinary_parameters_and_configuration_are_recursively_readonly() {
    for (function, call) in [
        (
            "const fn mutate(values:list<i64>)->i64 {push(values,1)\nreturn len(values)}",
            "mutate([])",
        ),
        (
            "const fn mutate(values:list<list<i64>>)->i64 {push(values[0],1)\nreturn len(values)}",
            "mutate([])",
        ),
    ] {
        let source = format!("module readonly\n{function}\nexport fn main() {{let n={call}}}");
        let result = compile(&[("readonly.hgl".into(), source)], "main");
        let error = result.expect_err(function);
        assert!(
            error.contains("push requires writable ordinary list access"),
            "{function}: {error}"
        );
    }
    let source = "module readonly\nfn f(const values:list<i64>) {when {push(values,1)}}\nexport fn main() {f([])}";
    let error = compile(&[("readonly.hgl".into(), source.into())], "main")
        .expect_err("const configuration must be read-only");
    assert!(
        error.contains("push requires writable ordinary list access"),
        "{error}"
    );
}

#[test]
fn ordinary_lists_execute_source_values_borrows_and_failures()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-ordinary-lists-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(dir.join("src"))?;
    for (entry, name) in [
        ("main", "graph"),
        ("fail_append", "fail_append"),
        ("fail_replace", "fail_replace"),
    ] {
        let plan = compile(&[("ordinary_lists.hgl".into(), FIXTURE.into())], entry)?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
    }
    let phases = compile(
        &[("ordinary_list_phases.hgl".into(), PHASES.into())],
        "main",
    )?;
    fs::write(dir.join("src/phases.rs"), emit_rust(&phases))?;
    evaluations(&dir)?;
    let failures = bounds_and_presence(&dir)?;
    fs::write(
        dir.join("src/main.rs"),
        format!(
            "{}\n{failures}",
            include_str!("fixtures/ordinary_lists_runner.rs")
        ),
    )?;
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
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(dir)?;
    Ok(())
}

fn evaluations(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let suite = compile_tests(&[
        (
            "spec_lists.hgl".into(),
            include_str!(
                "../../../external/hgraph_spec/language/examples/ordinary-list-values.hgl"
            )
            .into(),
        ),
        ("ordinary_lists.hgl".into(), FIXTURE.into()),
        ("ordinary_list_phases.hgl".into(), PHASES.into()),
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

fn bounds_and_presence(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let mut modules = String::new();
    let mut calls = String::new();
    for (name, index, body) in [
        ("negative", -1, "let xs:list<i64> =[1]\nlet bad=xs[input]"),
        ("past_end", 1, "let xs:list<i64> =[1]\nlet bad=xs[input]"),
        ("empty", 0, "let xs:list<i64> =[]\nlet bad=xs[input]"),
        (
            "missing",
            0,
            "let xs:list<i64> =get(global_state,\"values\")",
        ),
    ] {
        let source = format!(
            "module bounds\nfn source()->i64 {{inject alarm\nstart {{schedule(alarm,0us)}}\nwhen {{return {index}}}}}\nfn f(input:i64) {{inject global_state\nwhen {{{body}}}}}\nexport fn main() {{f(source())}}"
        );
        let plan = compile(&[("bounds.hgl".into(), source)], "main")?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
        writeln!(modules, "mod {name};")?;
        writeln!(
            calls,
            "let mut registry=Registry::new();\n{name}::register(&mut registry).unwrap();\nlet description={name}::main(&registry).unwrap();\nlet mut store=Store::new();\nstore.global_state().provision();\nlet mut built=instantiate_complete(&description,&registry,&mut store).unwrap();\nassert!(run_simulation(&mut built.graph,&mut store,&config()).is_err(),\"{name}\");"
        )?;
    }
    for (name, body) in [
        ("wiring_bounds", "let value=fail()"),
        ("returned_bounds", "return fail()"),
    ] {
        let source = format!(
            "module {name}\nconst fn fail()->i64 {{let values:list<i64> =[]\nreturn values[0]}}\nexport fn main() {{{body}}}"
        );
        let plan = compile(&[(format!("{name}.hgl"), source)], "main")?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
        writeln!(modules, "mod {name};")?;
        writeln!(
            calls,
            "let mut registry=Registry::new(); {name}::register(&mut registry).unwrap(); let error={name}::main(&registry).unwrap_err(); assert!(format!(\"{{error:?}}\").contains(\"ordinary list index out of bounds\"));"
        )?;
    }
    for (name, body) in [
        ("wiring_mod_zero", "return remainder(1.0,0.0)"),
        ("runtime_mod_zero", "node()"),
    ] {
        let source = format!(
            "module {name}\nconst fn remainder(lhs:f64,rhs:f64)->f64 => lhs % rhs\nfn node() {{inject alarm\nstart {{schedule(alarm,0us)}}\nwhen {{let failed=remainder(1.0,0.0)}}}}\nexport fn main() {{{body}}}"
        );
        let plan = compile(&[(format!("{name}.hgl"), source)], "main")?;
        fs::write(dir.join(format!("src/{name}.rs")), emit_rust(&plan))?;
        writeln!(modules, "mod {name};")?;
        if name == "wiring_mod_zero" {
            writeln!(
                calls,
                "let mut registry=Registry::new(); {name}::register(&mut registry).unwrap(); let error={name}::main(&registry).unwrap_err(); assert!(format!(\"{{error:?}}\").contains(\"division by zero\"));"
            )?;
        } else {
            writeln!(
                calls,
                "let mut registry=Registry::new(); {name}::register(&mut registry).unwrap(); let description={name}::main(&registry).unwrap(); let mut store=Store::new(); let mut built=instantiate_complete(&description,&registry,&mut store).unwrap(); let error=run_simulation(&mut built.graph,&mut store,&config()).unwrap_err(); assert!(format!(\"{{error:?}}\").contains(\"division by zero\"));"
            )?;
        }
    }
    Ok(format!("{modules}\nfn bounds_and_presence() {{ {calls} }}"))
}

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"ordinary-list-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
