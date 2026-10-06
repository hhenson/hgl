//! Canonical required-field generic structs: inference, identity and retained storage.
use hgl_program::{compile, emit_rust};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

// Two tests can start within the same clock tick, and on Windows a second test
// in the same directory then fights the first for its executable.
static NEXT_DIR: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

const TYPES: &str = "struct TimedValue<T> { time:datetime\nvalue:T }\nstruct Pair<A,B> { first:A\nsecond:B }\nstruct Same<T> { first:T\nsecond:T }\nstruct Phantom<T> { amount:i64 }\nstruct Wrap<T> { inner:TimedValue<T> }\nstruct Numeric<T> requires T in {i64,f64} { value:T }\nstruct ConstrainedWrapper<T> { inner:Numeric<T> }";
const FIXTURE: &str = include_str!("fixtures/generic_structs.hgl");

fn source(types: &str, body: &str) -> String {
    format!(
        "module generic_structs\n{types}\nfn source()->i64 {{when {{return 1}}}}\nfn exercise(input:i64) {{inject global_state\nwhen {{{body}}}}}\nexport fn main() {{exercise(source())}}"
    )
}
fn checked(body: &str) -> Result<(), String> {
    compile(
        &[("generic_structs.hgl".into(), source(TYPES, body))],
        "main",
    )
    .map(|_| ())
}

#[test]
fn explicit_inferred_and_contextual_arguments_form_complete_specializations() {
    for body in [
        "let item=TimedValue<i64>(time:@2026-10-03T00:00:00Z,value:1)",
        "let item=TimedValue(time:@2026-10-03T00:00:00Z,value:1)",
        "let item:TimedValue<i64> =TimedValue(time:@2026-10-03T00:00:00Z,value:1)",
        "let item:TimedValue<list<i64>> =TimedValue(time:@2026-10-03T00:00:00Z,value:[])",
        "let item=Pair(first:1,second:\"two\")\nlet exact:Pair<i64,str> =item",
        "let item=Same(first:1,second:2)\nlet exact:Same<i64> =item",
        "let value=Same(first:Phantom(amount:1),second:Phantom<i64>(amount:2))\nlet exact:Same<Phantom<i64>> =value",
        "let value=Same(second:Phantom<i64>(amount:2),first:Phantom(amount:1))\nlet exact:Same<Phantom<i64>> =value",
        "let item=Wrap(inner:TimedValue(time:@2026-10-03T00:00:00Z,value:2))\nlet exact:Wrap<i64> =item",
        "let item=Phantom<i64>(amount:1)\nlet other:Phantom<str> =Phantom(amount:2)",
        "let item=Pair<TimedValue<i64>,list<str>>(first:TimedValue(time:@2026-10-03T00:00:00Z,value:1),second:[])",
        "let item=Numeric(value:2)\nlet other:Numeric<f64> =Numeric(value:2.5)",
        "let item:Pair<Phantom<i64>,Phantom<str>> =Pair(first:Phantom(amount:1),second:Phantom(amount:2))",
        "let item=ConstrainedWrapper(inner:Numeric(value:2))\nlet exact:ConstrainedWrapper<i64> =item",
        "var item:TimedValue<i64> =TimedValue(time:@2026-10-03T00:00:00Z,value:1)\nitem=TimedValue(time:item.time,value:2)",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn contextual_global_get_uses_sibling_constructor_evidence() {
    for body in [
        "let item=Same(first:get(global_state,\"entry\"),second:1)\nlet exact:Same<i64> =item",
        "let item=Same(second:1,first:get(global_state,\"entry\"))\nlet exact:Same<i64> =item",
        "let item=Same(first:Same(first:get(global_state,\"entry\"),second:1),second:Same(first:2,second:3))\nlet exact:Same<Same<i64>> =item",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    let error =
        checked("let item=Same(first:get(global_state,\"a\"),second:get(global_state,\"b\"))")
            .expect_err("contextual reads alone do not supply a type");
    assert!(
        error.contains("requires a concrete scalar expected type"),
        "{error}"
    );
}

#[test]
fn incompatible_or_incomplete_generic_arguments_have_meaningful_diagnostics() {
    let mut failures = Vec::new();
    for (body, expected) in [
        (
            "let item:TimedValue=TimedValue(time:@2026-10-03T00:00:00Z,value:1)",
            "argument",
        ),
        (
            "let item=TimedValue<i64,str>(time:@2026-10-03T00:00:00Z,value:1)",
            "argument",
        ),
        ("let item:Pair<i64> =Pair(first:1,second:2)", "argument"),
        (
            "let item:Pair<_,str> =Pair(first:1,second:\"two\")",
            "placeholders are not admitted",
        ),
        (
            "let item=Phantom(amount:1)",
            "unresolved struct type parameter T",
        ),
        ("let item=Same(first:1,second:2.0)", "conflict"),
        (
            "var item=Pair(first:1,second:\"two\")\nitem=Pair(first:\"one\",second:2)",
            "conflicting struct inference",
        ),
        (
            "let item:TimedValue<i64> =TimedValue(time:@2026-10-03T00:00:00Z,value:1.5)",
            "conflicting struct inference for T",
        ),
        (
            "let item=Numeric<str>(value:\"text\")",
            "requires T in {i64, f64}",
        ),
        (
            "let item=Numeric(value:\"text\")",
            "requires T in {i64, f64}",
        ),
        (
            "let item:ConstrainedWrapper<str> =get(global_state,\"bad\")",
            "requires T in {i64, f64}",
        ),
        (
            "var item=Phantom<list<i64,0>>(amount:1)\nitem=Phantom<list<i64>>(amount:2)",
            "conflicting struct inference",
        ),
        (
            "var item=Phantom<i64>(amount:1)\nitem=Phantom<str>(amount:2)",
            "conflicting struct inference",
        ),
        (
            "set(global_state,\"same\",Phantom<i64>(amount:1))\nset(global_state,\"same\",Phantom<str>(amount:2))",
            "type conflict",
        ),
        (
            "let item:TimedValue<ref<i64>> =get(global_state,\"bad\")",
            "ordinary",
        ),
        (
            "let item:TimedValue<atomic<set<i64>>> =get(global_state,\"bad\")",
            "ordinary",
        ),
        (
            "let item:TimedValue<rolling<i64,2>> =get(global_state,\"bad\")",
            "ordinary",
        ),
    ] {
        let error = checked(body).expect_err(body);
        if !error.contains(expected) {
            failures.push(format!("{body}: expected {expected:?}, got {error:?}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn generic_borrows_keep_existing_lexical_permissions_and_exact_specializations() {
    for body in [
        "let item:TimedValue<i64> =get(global_state,\"a\")\nlet alias=item\nlet first=alias.value",
        "var item:TimedValue<list<i64>> =get(global_state,\"a\")\npush(item.value,1)",
        "var copy:TimedValue<i64> =TimedValue(time:@2026-10-03T00:00:00Z,value:0)\nlet borrowed:TimedValue<i64> =get(global_state,\"a\")\ncopy=borrowed\ncopy.value=2",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    for (body, expected) in [
        (
            "let item:TimedValue<i64> =get(global_state,\"a\")\nitem.value=2",
            "assignment requires writable",
        ),
        (
            "var item:TimedValue<i64> =get(global_state,\"a\")\nlet alias=item",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "let item:TimedValue<i64> =get(global_state,\"a\")\nvar alias=item",
            "cannot upgrade a read-only global borrow",
        ),
        (
            "var item:TimedValue<list<i64>> =get(global_state,\"a\")\nlet alias=item.value",
            "cannot alias an exclusive writable global borrow",
        ),
        (
            "let item:Phantom<i64> =get(global_state,\"a\")\nlet wrong:Phantom<str> =get(global_state,\"a\")",
            "type conflict",
        ),
    ] {
        let error = checked(body).expect_err(body);
        assert!(error.contains(expected), "{body}: {error}");
    }
}

#[test]
fn aliases_preserve_origin_and_different_origins_remain_nominal() {
    let families = vec![
        ("family.hgl".into(), "module family\nexport struct Box<T> { value:T }\nexport struct Phantom<T> { amount:i64 }".into()),
        ("other.hgl".into(), "module other\nexport struct Box<T> { value:T }".into()),
    ];
    let base = "module root\nuse family::{Box,Phantom}\nuse family as f\nuse other as o\n";
    for body in [
        "let first:Box<i64> =f::Box<i64>(value:1)\nlet same:f::Box<i64> =first",
        "let item:Phantom<str> =f::Phantom(amount:1)",
    ] {
        let mut sources = families.clone();
        sources.insert(
            0,
            (
                "root.hgl".into(),
                format!("{base}export fn main() {{{body}}}"),
            ),
        );
        let result = compile(&sources, "main");
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    let mut sources = families;
    sources.insert(0,("root.hgl".into(),format!("{base}export fn main() {{var first=f::Box<i64>(value:1)\nfirst=o::Box<i64>(value:2)}}")));
    let error = compile(&sources, "main").expect_err("distinct origins must not unify");
    assert!(
        error.contains("constructor expected nominal type mismatch"),
        "{error}"
    );
}

#[test]
fn optional_generic_fields_preserve_their_concrete_specialization() {
    let types = "struct Box<T> { value:T =null }";
    for body in [
        "let item:Box<i64> =Box<i64>(value:1)",
        "let item:Box<i64> =Box<i64>()",
        "let item:Box<i64> =Box(value:null)",
    ] {
        let result = compile(&[("optional.hgl".into(), source(types, body))], "main");
        assert!(result.is_ok(), "{body}: {result:?}");
    }
    let error = compile(
        &[(
            "optional.hgl".into(),
            source(types, "let item:Box<i64> =Box(value:true)"),
        )],
        "main",
    )
    .expect_err("presence does not erase the field type");
    assert!(error.contains("conflicting struct inference"), "{error}");
}

#[test]
fn unsupported_generic_field_forms_are_explicitly_rejected() {
    for (types, body, expected) in [
        (
            "struct Base<T> { value:T }\nstruct Child<T>:Base<T> { extra:i64 }",
            "let item=Child<i64>(value:1,extra:2)",
            "inherit",
        ),
        (
            "struct Vector<T,const size:i64> { values:list<T,size> }",
            "let item=Vector<i64,0>(values:[])",
            "const",
        ),
    ] {
        let error =
            compile(&[("unsupported.hgl".into(), source(types, body))], "main").expect_err(body);
        assert!(
            error.contains(expected),
            "{types}: expected {expected:?}, got {error:?}"
        );
    }
}

#[test]
fn generic_field_defaults_follow_explicit_and_contextual_specialization() {
    for body in [
        "let item=Box<i64>(value:2)",
        "let item=Box<i64>()",
        "let item:Box<i64> =Box()",
        "let item=Box(value:2)",
    ] {
        let result = compile(
            &[(
                "defaults.hgl".into(),
                source("struct Box<T> { value:T =1 }", body),
            )],
            "main",
        );
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn generic_struct_values_configurations_and_globals_execute_in_both_profiles()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let directory = std::env::temp_dir().join(format!(
        "hgl-generic-structs-{}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos(),
        NEXT_DIR.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    fs::create_dir_all(directory.join("src"))?;
    let program = compile(&[("generic_structs.hgl".into(), FIXTURE.into())], "main")?;
    fs::write(directory.join("src/graph.rs"), emit_rust(&program))?;
    for (module, fields) in [
        (
            "get_first",
            "first:get(global_state,\"missing\"),second:mark(1)",
        ),
        (
            "get_last",
            "second:mark(1),first:get(global_state,\"missing\")",
        ),
    ] {
        let types = format!(
            "{TYPES}\nnative const fn mark(value:i64)->i64 throws\nnative const fn mark(value:i64)->i64 throws {{}}"
        );
        let code = format!(
            "module generic_structs\n{types}\nfn exercise() {{inject global_state,alarm\nstart {{schedule(alarm,0us)}}\nwhen {{let item=Same({fields})}}}}\nexport fn main() {{exercise()}}"
        );
        let program = compile(&[("contextual_get.hgl".into(), code)], "main")?;
        fs::write(
            directory.join(format!("src/{module}.rs")),
            emit_rust(&program),
        )?;
    }
    fs::write(
        directory.join("src/main.rs"),
        include_str!("fixtures/generic_structs_runner.rs"),
    )?;
    let mut manifest = String::from(
        "[package]\nname=\"generic-struct-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
    );
    for name in ["hgl-types", "hgl-store", "hgl-kernel", "hgl-describe"] {
        let path = root
            .join("crates")
            .join(name)
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        writeln!(manifest, "{name}={{path=\"{path}\"}}")?;
    }
    fs::write(
        directory.join("Cargo.toml"),
        unique_package(&manifest, &directory),
    )?;
    for profile in [vec![], vec!["--release"]] {
        let output = Command::new(env!("CARGO"))
            .args(["run", "--offline", "--quiet"])
            // One build cache for every generated program; a fresh one per test rebuilt the runtime each time.
            .env("CARGO_TARGET_DIR", concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/source-tests"))
            .env("CARGO_INCREMENTAL", "0")
            .args(profile)
            .current_dir(&directory)
            .output()?;
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    fs::remove_dir_all(directory)?;
    Ok(())
}

#[test]
fn explicit_type_application_is_not_generic_function_call_syntax() {
    let types = format!("{TYPES}\nconst fn identity<T>(value:T)->T {{return value}}");
    let error = compile(
        &[(
            "functions.hgl".into(),
            source(&types, "let item=identity<i64>(value:1)"),
        )],
        "main",
    )
    .expect_err("explicit generic calls are admitted for struct constructors only");
    assert!(error.contains("struct"), "{error}");
}

#[test]
fn configured_cross_node_specialization_conflicts_fail_before_hooks() {
    let error = compile(
        &[(
            "generic_struct_conflict.hgl".into(),
            include_str!("fixtures/generic_struct_conflict.hgl").into(),
        )],
        "main",
    )
    .expect_err("a shared key must have one complete nominal specialization");
    assert!(
        error.contains("type conflict") && error.contains("shared"),
        "{error}"
    );
}

#[test]
fn generic_helper_parameters_preserve_the_aggregate_borrow_escape_rule() {
    let types = format!(
        "{TYPES}\nconst fn payload<T>(value:TimedValue<T>)->T {{return value.value}}\nconst fn first<T>(values:list<TimedValue<T>>)->T {{return values[0].value}}"
    );
    for body in [
        "let borrowed:TimedValue<i64> =get(global_state,\"entry\")\nlet escaped=payload(borrowed)",
        "let borrowed:list<TimedValue<i64>> =get(global_state,\"entry\")\nlet escaped=payload(borrowed[0])",
        "let borrowed:list<TimedValue<i64>> =get(global_state,\"entry\")\nlet escaped=first(borrowed)",
    ] {
        let error = compile(&[("escape.hgl".into(), source(&types, body))], "main")
            .expect_err("generic parameters cannot carry lexical aggregate borrows");
        assert!(
            error.contains(
                "borrowed global aggregate cannot escape through an ordinary helper call"
            ),
            "{body}: {error}"
        );
    }
}

#[test]
fn multiline_explicit_nested_types_remain_distinct_from_comparisons() {
    let result = checked(
        "let item=Pair<\nTimedValue<\ni64\n>,\nlist<\nstr\n>\n>(first:TimedValue<\ni64\n>(time:@2026-10-03T00:00:00Z,value:1),second:[])\nlet smaller=input < 2\nlet larger=input > 0\nlet bounded=(input < 2) && (input > 0)\nlet exact:Pair<TimedValue<i64>,list<str>> =item",
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn rolling_delta_fields_preserve_exact_window_specialization() {
    let types = "struct Arrival<T>{value:delta<T>}";
    let check = |body| compile(&[("rolling.hgl".into(), source(types, body))], "main");
    let result = check(
        "let item:Arrival<rolling<i64,2>> = get(global_state,\"window\")\nlet arrival:i64 = item.value",
    );
    assert!(result.is_ok(), "{result:?}");
    let result = check(
        "let a:Arrival<rolling<i64,2>> = get(global_state,\"window\")\nlet b:Arrival<rolling<i64,3>> = get(global_state,\"window\")",
    );
    assert!(result.is_err(), "distinct rolling identities merged");
}

/// Parallel tests share one build cache, so each generated package needs a name
/// of its own: `cargo run` would otherwise execute a sibling's binary.
fn unique_package(manifest: &str, dir: &Path) -> String {
    let suffix = dir
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    manifest.replacen("\"\n", &format!("-{suffix}\"\n"), 1)
}
