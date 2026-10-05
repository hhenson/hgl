//! Enum publication frontend keeps declared identities through concrete wrappers.
use hgl_program::{compile, compile_tests};

const DECLARATIONS: &str = "enum E {first = -7, second = 11}\nenum Other {first = -7, second = 11}\nstruct Box<T> {value:T}\nstruct Default {value:E = E::first}";
fn checked(body: &str) -> Result<(), String> {
    let source = format!(
        "module example\n{DECLARATIONS}\nconst fn id<T>(value:delta<atomic<T>>)->T {{return value}}\ntest values {{{body}}}"
    );
    compile_tests(&[("test.hgl".into(), source)]).map(|_| ())
}

#[test]
fn members_defaults_and_generic_delta_atomic_types_are_exact() {
    for body in [
        "let value:delta<map<E,i64>> = delta<map<E,i64>>(upsert:[E::first:1])",
        "let value:delta<set<E>> = delta<set<E>>(added:[E::first])",
        "let value:E = E::first\nassert value == E::first",
        "let value:atomic<E> = E::first\nlet delta:delta<atomic<E>> = value\nassert delta != E::second",
        "let value:E = id(E::first)",
        "let value:Box<E> = Box(value:E::first)\nassert value.value == E::first",
        "let value:Box<delta<atomic<E>>> = Box<delta<E>>(value:E::first)",
        "let value=Default()\nassert value.value == E::first",
        "let value:list<E> = [E::first, E::second]\nassert value[0] == E::first",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn wrong_identity_and_unsupported_integer_operations_are_rejected() {
    for body in [
        "let value:E = -7",
        "let value:i64 = E::first",
        "let value:Other = E::first",
        "let value:E = \"E::first\"",
        "let value:delta<atomic<Other>> = E::first",
        "let value:Box<Other> = Box(value:E::first)",
        "assert E::first == Other::first",
        "assert E::first == -7",
        "let value = E::first + E::second",
        "let value = -E::first",
        "let value = E::First",
        "let value = E(-7)",
        "let value = E(\"first\")",
        "let value = values(E)",
        "let value = keys(E)",
        "let value = elements(E)",
    ] {
        assert!(checked(body).is_err(), "{body}");
    }
}

#[test]
fn unused_bad_enum_is_a_checking_error_in_both_entrypoints() {
    let source = "module example\nenum E {high=9223372036854775807, overflow}\nfn main() {when {}}\ntest values {assert true}";
    for result in [
        compile(&[("test.hgl".into(), source.into())], "main").map(|_| ()),
        compile_tests(&[("test.hgl".into(), source.into())]).map(|_| ()),
    ] {
        assert!(
            result
                .unwrap_err()
                .contains("enum automatic numbering overflow")
        );
    }
}

#[test]
fn imported_enum_defaults_resolve_in_the_declaration_module() {
    let types =
        "module types\nexport enum E {first=-7}\nexport struct Default {value:E = E::first}";
    let main = "module main\nuse types as t\nenum E {first=20}\ntest values {let value=t::Default()\nassert value.value == t::E::first}";
    assert!(
        compile_tests(&[
            ("main.hgl".into(), main.into()),
            ("types.hgl".into(), types.into())
        ])
        .is_ok()
    );
    let private = types.replace("export enum", "enum");
    let error = compile_tests(&[
        ("main.hgl".into(), main.into()),
        ("types.hgl".into(), private),
    ])
    .unwrap_err();
    assert!(
        error.contains("unexported") || error.contains("not exported"),
        "{error}"
    );
}

#[test]
fn concrete_enum_parameters_and_returns_do_not_coerce() {
    for payload in ["E::first", "Other::first", "-7"] {
        let source = format!(
            "module example\n{DECLARATIONS}\nconst fn identity(value:E)->E {{return value}}\ntest values {{let value:E = identity({payload})}}"
        );
        let result = compile_tests(&[("test.hgl".into(), source)]);
        assert_eq!(
            result.is_ok(),
            payload == "E::first",
            "{payload}: {result:?}"
        );
    }
}

#[test]
fn local_nominal_names_take_precedence_over_imported_enum_or_struct() {
    for (local, imported, expression) in [
        (
            "struct E {value:i64}",
            "export enum E {first}",
            "E(value:1)",
        ),
        ("enum E {first}", "export struct E {value:i64}", "E::first"),
    ] {
        let main = format!(
            "module main\nuse types::{{E}}\n{local}\ntest values {{let value:E = {expression}}}"
        );
        let types = format!("module types\n{imported}");
        let result = compile_tests(&[("main.hgl".into(), main), ("types.hgl".into(), types)]);
        assert!(result.is_ok(), "{result:?}");
    }
}

#[test]
fn defaults_are_checked_after_generic_substitution_even_if_overridden() {
    for supplied in ["", "value:Other::first"] {
        let source = format!(
            "module example\n{DECLARATIONS}\nstruct WithDefault<T> {{value:T = E::first}}\ntest values {{let value=WithDefault<Other>({supplied})}}"
        );
        let error = compile_tests(&[("test.hgl".into(), source)]).unwrap_err();
        assert!(error.contains("default type mismatch"), "{error}");
    }
}

#[test]
fn unsupported_construction_and_enumeration_have_specific_diagnostics() {
    for (expression, diagnostic) in [
        ("E(-7)", "enum construction is unsupported"),
        ("E(\"first\")", "enum construction is unsupported"),
        ("keys(E)", "enum enumeration is unsupported"),
        ("values(E)", "enum enumeration is unsupported"),
        ("elements(E)", "enum enumeration is unsupported"),
    ] {
        let error = checked(&format!("let value = {expression}")).unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
    }
}

#[test]
fn repeated_generic_delta_evidence_requires_one_exact_enum() {
    for second in ["E::second", "Other::second", "11"] {
        let source = format!(
            "module example\n{DECLARATIONS}\nconst fn same<T>(first:delta<T>,second:delta<atomic<T>>)->T {{return first}}\ntest values {{let value:E = same(E::first,{second})}}"
        );
        let result = compile_tests(&[("test.hgl".into(), source)]);
        assert_eq!(
            result.is_ok(),
            second == "E::second",
            "{second}: {result:?}"
        );
    }
}
