//! Scalar shared state is checked independently of replay and record operators.
use hgl_program::{compile, emit_rust};

fn check(definition: &str) -> Result<String, String> {
    let source = format!(
        "module shared\nfn tick()->i64 {{ inject alarm\nstart {{ schedule(alarm,0s) }}\nwhen {{ return 1 }} }}\n{definition}\nexport fn main() {{ f(tick()) }}"
    );
    compile(&[("shared.hgl".into(), source)], "main").map(|program| emit_rust(&program))
}

#[test]
fn scalar_get_has_an_ordinary_type_context_in_each_hook() {
    let emitted = check(
        "fn f<T>(value:T)->T { inject global_state\nstart { let count:i64=get(global_state,\"count\")\nset(global_state,\"started\",count) }\nwhen { let item:T=get(global_state,key:\"value\")\nset(global_state,value:item,key:\"copied\")\nreturn item }\nstop { let count:i64=get(global_state,\"count\")\nset(global_state,\"stopped\",count) } }",
    ).unwrap();
    assert!(emitted.contains("uses_global_state: true"));
    assert!(emitted.contains("fn stop("));
    assert_eq!(
        emitted
            .matches("_ctx.global_state().get(self.global")
            .count(),
        3
    );
    assert_eq!(
        emitted
            .matches("_ctx.global_state().set(self.global")
            .count(),
        3
    );
    assert!(!emitted.contains("ReplayInput"));
    assert!(!emitted.contains("Capture"));
}

#[test]
fn scalar_eight_have_exact_get_and_set_types() {
    for (ty, literal, rust) in [
        ("bool", "false", "bool"),
        ("i64", "0", "i64"),
        ("f64", "0.0", "f64"),
        ("str", "\"\"", "String"),
        ("date", "@2026-01-01", "hgl_types::Date"),
        ("time", "@00:00:00", "hgl_types::Time"),
        ("datetime", "@2026-01-01T00:00:00Z", "hgl_types::EngineTime"),
        ("duration", "0us", "hgl_types::EngineDelta"),
    ] {
        let code = check(&format!("fn f(value:i64) {{ inject global_state\nstart {{ set(global_state,\"key\",{literal}) }}\nwhen {{ let stored:{ty}=get(global_state,\"key\")\nset(global_state,\"copy\",stored) }} }}")).unwrap();
        assert!(code.contains(&format!("hgl_store::Global<{rust}>")), "{ty}");
        assert!(code.contains(&format!("ports.global::<{rust}>")), "{ty}");
    }
}

#[test]
fn return_assignment_and_condition_supply_direct_expected_types() {
    for body in [
        "inject global_state\nwhen { return get(global_state,\"key\") }",
        "inject global_state\ncache value:i64=0\nwhen { value=get(global_state,\"key\")\nreturn value }",
        "inject global_state\nwhen { if get(global_state,\"enabled\") { return 1 } }",
    ] {
        let result = check(&format!("fn f(input:i64)->i64 {{ {body} }}"));
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn unconstrained_reads_and_non_scalar_values_are_rejected() {
    for (body, message) in [
        ("let item=get(global_state,\"key\")", "expected type"),
        ("let item:bool=get(global_state,0)", "wrong-type argument"),
        (
            "let item:ref<i64> = get(global_state,\"key\")",
            "scalar expected type",
        ),
        ("let item:bool=1", "initializer type mismatch"),
        ("set(global_state,0,1)", "str key"),
        ("set(global_state,\"key\",global_state)", "cannot escape"),
        ("let escaped=global_state", "cannot escape"),
        (
            "let item:i64=get(state:global_state,key:\"key\")",
            "first positional",
        ),
        (
            "let item:i64=global_state.get(\"key\")",
            "properties cannot be invoked",
        ),
        ("set(global_state,\"key\")", "missing value"),
    ] {
        let error = check(&format!(
            "fn f(input:i64)->i64 {{ inject global_state\nwhen {{ {body} }} }}"
        ))
        .unwrap_err();
        assert!(error.contains(message), "{body}: {error}");
    }
    let error = check(
        "fn f(input:signal) { inject global_state\nwhen { set(global_state,\"key\",input) } }",
    )
    .unwrap_err();
    assert!(error.contains("ordinary scalar value"), "{error}");
}

#[test]
fn stops_do_not_admit_temporal_reads_or_evaluation_only_operations() {
    for body in [
        "let item=input",
        "set(global_state,\"key\",delta_value(input))",
        "return 1",
        "set(global_state,\"key\",global_state)",
    ] {
        let result = check(&format!(
            "fn f(input:i64) {{ inject global_state\nwhen {{}}\nstop {{ {body} }} }}"
        ));
        assert!(result.is_err(), "{body}");
    }
    assert!(
        check("fn f(input:i64) { inject global_state\nstart {}\nwhen {}\nstop {}\nstop {} }")
            .unwrap_err()
            .contains("duplicate stop")
    );
}

#[test]
fn injecting_store_never_uses_a_node_role_or_key_as_the_type() {
    assert!(check("fn f(input:i64)->i64 { inject global_state\nwhen { let item=get(global_state,\"i64\")\nreturn item } }").unwrap_err().contains("expected type"));
    assert!(check("fn f(global_state:i64) { when { set(global_state,\"key\",1) } }").is_err());
    let code = check("fn f(input:i64) { inject global_state\nstop { set(global_state,\"last\",0) }\nwhen {}\nstart { set(global_state,\"first\",0) } }").unwrap();
    assert_eq!(
        code.matches("_ctx.global_state().set(self.global").count(),
        2
    );
}

#[test]
fn const_keys_bind_once_and_known_type_conflicts_fail_checking() {
    let code = check("fn f<T>(input:T,const key:str=\"counter\") { inject global_state\nstart { set(global_state,key,0) }\nwhen { let count:i64=get(global_state,key)\nset(global_state,key,count+1) }\nstop { let count:i64=get(global_state,key) } }").unwrap();
    assert_eq!(code.matches("ports.global::<i64>(\"counter\")").count(), 1);
    assert!(code.contains("global_entries: vec![(\"counter\", hgl_types::ScalarType::I64.into())"));
    for body in [
        "set(global_state,\"key\",1)\nset(global_state,\"key\",false)",
        "set(global_state,\"key\",1)\nlet item:str=get(global_state,\"key\")",
    ] {
        let error = check(&format!(
            "fn f(input:i64) {{ inject global_state\nwhen {{ {body} }} }}"
        ))
        .unwrap_err();
        assert!(error.contains("type conflict for key \"key\""), "{error}");
    }
    for body in [
        "let key=\"key\"\nset(global_state,key,1)",
        "var key=\"key\"\nlet item:i64=get(global_state,key)",
    ] {
        let error = check(&format!(
            "fn f(input:i64) {{ inject global_state\nwhen {{ {body} }} }}"
        ))
        .unwrap_err();
        assert!(error.contains("key must resolve"), "{error}");
    }
}

#[test]
fn global_key_types_are_shared_across_the_assembled_graph() {
    let source = "module shared\nfn tick()->str { inject alarm\nstart { schedule(alarm,0s) }\nwhen { return \"dynamic\" } }\nfn a(input:str) { inject global_state\nwhen { set(global_state,\"shared\",1) } }\nfn b(input:str) { inject global_state\nwhen { set(global_state,\"shared\",false) } }\nexport fn main() { let input=tick()\na(input)\nb(input) }";
    let error = compile(&[("shared.hgl".into(), source.into())], "main").unwrap_err();
    assert!(
        error.contains("type conflict for key \"shared\""),
        "{error}"
    );
    let source = source.replace(
        "set(global_state,\"shared\",1)",
        "set(global_state,input,1)",
    );
    assert!(
        compile(&[("shared.hgl".into(), source)], "main")
            .unwrap_err()
            .contains("key must resolve")
    );
}

#[test]
fn requirements_belong_to_selected_nodes_and_exact_keys() {
    let source = "module shared\nfn tick()->i64 { inject alarm\nstart { schedule(alarm,0s) }\nwhen { return 1 } }\nfn a(value:i64) { inject global_state\nwhen { set(global_state,\"one\",1) } }\nfn b(value:i64) { inject global_state\nwhen { let same:i64=get(global_state,\"one\")\nset(global_state,\"two\",false) } }\nfn unused(value:i64) { inject global_state\nwhen { set(global_state,\"one\",false) } }\nexport fn main() { let value=tick()\na(value)\nb(value) }";
    assert!(compile(&[("shared.hgl".into(), source.into())], "main").is_ok());
    let error =
        check("fn f(value:i64) { inject global_state\nwhen { set(global_state,\"a\"+\"b\",1) } }")
            .unwrap_err();
    assert!(error.contains("general const key expressions"), "{error}");
}

#[test]
fn adding_stop_hooks_does_not_expand_legacy_replay_phases() {
    let source = "module phases\nfn replay()->i64 { inject replay_input,alarm\nstart { schedule(alarm,0s) }\nwhen { return 1 }\nstop { let count=len(replay_input) } }\nexport fn main()->i64 { return replay() }";
    let error = compile(&[("phases.hgl".into(), source.into())], "main").unwrap_err();
    assert!(error.contains("forbidden hook phase"), "{error}");
}
