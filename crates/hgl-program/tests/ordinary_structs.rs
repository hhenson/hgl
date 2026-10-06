//! Ordinary struct ownership and recursive access authority in source hooks.
use hgl_program::{compile, compile_tests, emit_rust, emit_tests};
use std::{fmt::Write as _, fs, path::Path, process::Command, time::SystemTime};

const TYPES: &str =
    "struct Box { amount: i64 }\nstruct Other { amount: i64 }\nstruct Envelope { inner: Box }\n";
fn checked(types: &str, body: &str) -> Result<String, String> {
    let source = format!(
        "module structs\n{types}\nfn source()->i64 {{ inject alarm\nstart {{ schedule(alarm,0us) }}\nwhen {{ return 1 }} }}\nfn f(input:i64) {{ inject global_state\nwhen {{ {body} }} }}\nexport fn main() {{ f(source()) }}"
    );
    compile(&[("structs.hgl".into(), source)], "main").map(|p| emit_rust(&p))
}

#[test]
fn value_mutability_rejects_readonly_and_incompatible_struct_writes() {
    for (body, error) in [
        ("let box=Box(amount:1)\nbox.amount=2", "writable"),
        ("let box=Box(amount:1)\nbox=Box(amount:2)", "writable"),
        (
            "let outer=Envelope(inner:Box(amount:1))\nouter.inner.amount=2",
            "writable",
        ),
        (
            "var box=Box(amount:1)\nlet frozen=box\nfrozen.amount+=2",
            "writable",
        ),
        ("input=2", "writable"),
        ("input.amount=2", "ordinary struct"),
        (
            "var box=Box(amount:1)\nbox=Other(amount:2)",
            "type mismatch",
        ),
        (
            "var outer=Envelope(inner:Box(amount:1))\nouter.inner=Other(amount:2)",
            "type mismatch",
        ),
        ("let box:Other=Box(amount:1)", "type mismatch"),
        ("let box=Box()", "missing or wrong-type"),
        ("let box=Box(amount:true)", "missing or wrong-type"),
        ("let box=Box(amount:1,amount:2)", "duplicate"),
        ("let box=Box(1)", "named fields"),
        ("let box=Box(other:1)", "unknown argument"),
        (
            "let box=Box(amount:1)\nlet x=box.missing",
            "unknown struct field",
        ),
    ] {
        let actual = checked(TYPES, body).unwrap_err();
        assert!(actual.contains(error), "{body}: {actual}");
    }
}

#[test]
fn required_structs_admit_global_storage_and_typed_borrow_bindings() {
    for body in [
        "set(global_state,\"box\",Box(amount:1))",
        "var box:Box=get(global_state,\"box\")",
        "let box:Box=get(global_state,\"box\")",
    ] {
        let result = checked(TYPES, body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn unsupported_struct_schemas_are_diagnosed() {
    for (types, body, error) in [
        (
            "struct Box<T> { amount: T }",
            "let box:Box=Box(amount:1)",
            "complete type arguments",
        ),
        (
            "struct Box { amount:i64=null }",
            "let box=Box(amount:1)\nlet amount=box.amount",
            "optional field access",
        ),
        (
            "struct Box { child:Box }",
            "let box:Box=get(global_state,\"x\")",
            "recursive",
        ),
        (
            "struct Box { amount:i64\namount:i64 }",
            "let box=Box(amount:1)",
            "duplicate struct field",
        ),
        (
            "struct Box { amount:i64 }\nstruct Box { amount:i64 }",
            "let box=Box(amount:1)",
            "duplicate struct",
        ),
        (
            "struct Box { child:ref<i64> }",
            "let box:Box=get(global_state,\"x\")",
            "fields require",
        ),
    ] {
        let actual = checked(types, body).unwrap_err();
        assert!(actual.contains(error), "{types}: {actual}");
    }
}

#[test]
fn fixed_struct_defaults_are_checked_even_when_fields_are_supplied() {
    for (types, body, error) in [
        (
            "struct Box { amount:i64=true }",
            "let box=Box(amount:1)",
            "default type mismatch",
        ),
        (
            "struct Box { amount:i64=9223372036854775807+1 }",
            "let box=Box()",
            "fixed scalar",
        ),
        (
            "struct Box { amount:i64=unknown() }",
            "let box=Box()",
            "fixed scalar",
        ),
        (
            "struct Box<T> { amount:T=1 }",
            "let box=Box<bool>(amount:true)",
            "default type mismatch",
        ),
        (
            "struct Box<T> { amount:T=1 }",
            "let box=Box()",
            "unresolved struct type parameter",
        ),
        (
            "struct Box { amount:i64=1\nrequired:bool }",
            "let box=Box()",
            "missing or wrong-type",
        ),
    ] {
        let actual = checked(types, body).unwrap_err();
        assert!(actual.contains(error), "{types}: {actual}");
    }
    for body in ["let box=Box(amount:1)", "let box=Box()"] {
        assert!(checked("struct Box { amount:i64=1 }", body).is_ok());
    }
}

#[test]
fn imported_structs_keep_nominal_identity() {
    let sources = [
        ("main.hgl".into(), "module root\nuse boxes::{Box}\nuse boxes as b\nuse others as o\nfn source()->i64 { inject alarm\nstart { schedule(alarm,0us) }\nwhen { return 1 } }\nfn f(value:i64) { when { var box:Box=Box(amount:value)\nbox=b::Box(amount:2)\nlet copy:b::Box=box } }\nexport fn main() { f(source()) }".into()),
        ("boxes.hgl".into(), "module boxes\nexport struct Box { amount:i64 }".into()),
        ("others.hgl".into(), "module others\nexport struct Box { amount:i64 }".into()),
    ];
    let accepted = compile(&sources, "main");
    assert!(accepted.is_ok(), "{accepted:?}");
    let mut wrong = sources;
    wrong[0].1 = wrong[0].1.replace("box=b::Box", "box=o::Box");
    assert!(
        compile(&wrong, "main")
            .unwrap_err()
            .contains("type mismatch")
    );
}

#[test]
fn struct_resolution_preserves_declaring_scope_and_exports() {
    let source = "module root\nuse hidden::{Box}\nuse hidden as h\nfn source()->i64 { inject alarm\nstart { schedule(alarm,0us) }\nwhen { return 1 } }\nfn f<T>(value:T) { when { let x=Box(amount:value) } }\nexport fn main() { f(source()) }";
    let mut sources = vec![
        ("root.hgl".into(), source.into()),
        (
            "hidden.hgl".into(),
            "module hidden\nstruct Box { amount:i64 }".into(),
        ),
    ];
    assert!(
        compile(&sources, "main")
            .unwrap_err()
            .contains("not exported")
    );
    sources[1].1 = "module hidden\nexport struct Box { amount:T }".into();
    assert!(
        compile(&sources, "main")
            .unwrap_err()
            .contains("unresolved ordinary type T")
    );
    sources[1].1 =
        "module hidden\nstruct Inner { amount:i64 }\nexport struct Box { amount:Inner }".into();
    assert!(
        compile(&sources, "main")
            .unwrap_err()
            .contains("exported struct reaches unexported")
    );
    sources[1].1 = "module hidden\nexport struct Box { text:str }".into();
    sources[0].1 = source.replace("fn source", "struct Box { amount:i64 }\nfn source");
    assert!(compile(&sources, "main").is_ok());
}

#[test]
fn ordinary_struct_binding_can_shadow_clock() {
    let code = checked(
        TYPES,
        "var clock=Box(amount:1)\nclock.amount+=2\nlet result=clock.amount",
    );
    assert!(code.is_ok(), "{code:?}");
}

#[test]
fn struct_fields_supply_global_get_expected_context() {
    let code = checked(
        TYPES,
        "set(global_state,\"amount\",1)\nlet box=Box(amount:get(global_state,\"amount\"))\nlet outer=Envelope(inner:Box(amount:get(global_state,\"amount\")))",
    );
    assert!(code.is_ok(), "{code:?}");
    let wrong = checked(
        TYPES,
        "set(global_state,\"amount\",true)\nlet box=Box(amount:get(global_state,\"amount\"))",
    )
    .unwrap_err();
    assert!(wrong.contains("type conflict"), "{wrong}");
}

#[test]
fn signal_cannot_enter_struct_fields_through_values_or_aliases() {
    for body in [
        "let box=Box(amount:input)",
        "let alias=input\nlet box=Box(amount:alias)",
        "let box=Box(amount:input+1)",
        "let box=Box(amount:-input)",
    ] {
        let source = format!(
            "module signals\nstruct Box {{ amount:i64 }}\nfn source()->i64 {{ inject alarm\nstart {{ schedule(alarm,0us) }}\nwhen {{ return 1 }} }}\nfn f(input:signal) {{ when {{ {body} }} }}\nexport fn main() {{ f(source()) }}"
        );
        let error = compile(&[("signal.hgl".into(), source)], "main").unwrap_err();
        assert!(
            error.contains("signal has no ordinary scalar value"),
            "{body}: {error}"
        );
    }
}

#[test]
fn val17_owned_structs_execute_independent_nested_copies_and_mutations()
-> Result<(), Box<dyn std::error::Error>> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let dir = std::env::temp_dir().join(format!(
        "hgl-structs-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir_all(dir.join("src"))?;
    let plan = compile(
        &[(
            "ordinary_structs.hgl".into(),
            include_str!("fixtures/ordinary_structs.hgl").into(),
        )],
        "main",
    )?;
    let emitted = emit_rust(&plan);
    assert!(emitted.contains("Scalar::try_clone"));
    fs::write(dir.join("src/graph.rs"), emitted)?;
    for entry in ["success", "failure"] {
        let plan = compile(
            &[(
                "constructor_order.hgl".into(),
                include_str!("fixtures/constructor_order.hgl").into(),
            )],
            entry,
        )?;
        fs::write(dir.join(format!("src/order_{entry}.rs")), emit_rust(&plan))?;
    }

    let suite = compile_tests(&[
        (
            "ordinary_structs.hgl".into(),
            include_str!("fixtures/ordinary_structs.hgl").into(),
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

    fs::write(
        dir.join("src/main.rs"),
        include_str!("fixtures/ordinary_structs_runner.rs"),
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

fn manifest(root: &Path, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut manifest = String::from(
        "[package]\nname=\"ordinary-struct-test\"\nversion=\"0.0.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\n",
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
    fs::write(dir.join("Cargo.toml"), manifest)?;
    Ok(())
}
