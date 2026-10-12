//! Test-only nominal identities share module test scope without leaking into production.
use hgl_program::{compile, compile_suite, diagnostics, emit_rust};

fn source(body: &str) -> Vec<(String, String)> {
    vec![("contexts.hgl".into(), format!("module contexts\n{body}"))]
}
fn rejected(sources: &[(String, String)], expected: &str) {
    let errors = diagnostics(sources);
    assert!(
        errors
            .iter()
            .any(|error| error.to_string().contains(expected)),
        "{sources:?}: {errors:?}"
    );
    assert!(compile_suite(sources).is_err());
}

#[test]
fn canonical_spec_contexts_keep_fixture_graphs_test_only() -> Result<(), String> {
    let mut sources = vec![(
        "test-contexts.hgl".into(),
        include_str!("../../../external/hgraph_spec/language/examples/test-contexts.hgl").into(),
    )];
    for (name, text) in [
        (
            "replay_record.hgl",
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl"),
        ),
        (
            "replay_record_impl.hgl",
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl"),
        ),
    ] {
        sources.push((name.into(), text.into()));
    }
    let suite = compile_suite(&sources)?;
    assert_eq!(suite.tests.len(), 2);
    assert_eq!(suite.plans.len(), 4);
    sources.push((
        "production.hgl".into(),
        "module examples.test_contexts part production\nexport fn main() {}".into(),
    ));
    let output = emit_rust(&compile(&sources, "main")?);
    assert!(!output.contains("Fixture"));
    assert!(!output.contains("fixture_pair"));
    Ok(())
}

#[test]
fn forward_types_helpers_and_top_level_tests_share_module_parts() -> Result<(), String> {
    let sources = vec![
        ("main.hgl".into(), "module contexts part cases\ntest Fixture {assert make().value==Choice::yes}\ntest {test Choice {assert Fixture(value:Choice::no).value==Choice::no}}".into()),
        ("types.hgl".into(), "module contexts part types\ntest {const fn make()->Fixture => Fixture(value:Choice::yes)\nstruct Fixture {value:Choice}\nenum Choice {no,yes}}\nexport fn main() {}".into()),
    ];
    let suite = compile_suite(&sources)?;
    for test in suite.tests {
        let count = hgl_semantics::harness::execute(
            &test,
            |_| unreachable!("no contextual values"),
            |_, _| unreachable!("no graph eval"),
        )?;
        assert_eq!(count, 1);
    }
    let emitted = emit_rust(&compile(&sources, "main")?);
    assert!(!emitted.contains("Fixture") && !emitted.contains("Choice"));
    Ok(())
}

#[test]
fn production_cannot_name_test_types_in_signatures_bodies_or_defaults() {
    for declaration in [
        "fn hidden(value:Fixture) {}",
        "fn hidden()->Fixture => Fixture(value:1)",
        "const fn hidden<T>(value:T)->i64 {let result=Fixture(value:1)\nreturn result.value}",
        "const fn hidden()->Choice => Choice::yes",
        "const fn hidden()->i64 {let value=Choice::yes\nreturn 1}",
        "struct Production {value:Fixture}",
        "const fn hidden<T>(value:T)->i64 {let f=fn(x:Fixture)->i64 => 1\nreturn 1}",
        "struct Production {value:Choice=Choice::yes}",
        "struct Production<T> {value:T\nfixture:list<Fixture> = []}",
    ] {
        rejected(
            &source(&format!(
                "test {{struct Fixture {{value:i64}}\nenum Choice {{yes}}}}\n{declaration}"
            )),
            "test-only type",
        );
    }
}

#[test]
fn unused_imports_and_qualified_cross_module_access_are_rejected() {
    for caller in [
        "use owner::{Fixture}\ntest t {assert true}",
        "use owner::{Choice}\ntest t {assert true}",
        "use owner as hidden\ntest t {let value=hidden::Fixture(value:1)}",
        "use owner as hidden\ntest t {assert hidden::Choice::yes==hidden::Choice::yes}",
    ] {
        let sources = vec![
            ("caller.hgl".into(), format!("module caller\n{caller}")),
            (
                "owner.hgl".into(),
                "module owner\ntest {struct Fixture {value:i64}\nenum Choice {yes}}".into(),
            ),
        ];
        assert!(compile_suite(&sources).is_err(), "{caller}");
    }
}

#[test]
fn test_type_names_cannot_shadow_production_declarations_or_imports() {
    for other in [
        "struct Fixture {other:bool}",
        "enum Fixture {item}",
        "fn Fixture(value:i64)->i64 => value",
        "test {enum Fixture {item}}",
        "test {const fn Fixture(value:i64)->i64 => value}",
        "use dependency::{Fixture}",
        "use dependency as Fixture",
    ] {
        for reversed in [false, true] {
            let fixture = "test {struct Fixture {value:i64}}";
            let body = if reversed {
                format!("{fixture}\n{other}")
            } else {
                format!("{other}\n{fixture}")
            };
            let mut sources = source(&body);
            sources.push((
                "dependency.hgl".into(),
                "module dependency\nexport struct Fixture {value:i64}".into(),
            ));
            rejected(&sources, "conflicting test-local type");
        }
    }
}

#[test]
fn duplicate_test_types_across_parts_are_rejected_at_the_declaration() {
    let sources = vec![
        (
            "first.hgl".into(),
            "module contexts part first\ntest {struct Fixture {value:i64}}".into(),
        ),
        (
            "second.hgl".into(),
            "module contexts part second\ntest {struct Fixture {value:i64}}".into(),
        ),
    ];
    rejected(&sources, "conflicting test-local type");
    let errors = diagnostics(&sources);
    assert_eq!(errors[0].source, "first.hgl");
    assert_eq!(errors[0].line, 2);
}

#[test]
fn test_types_retain_generic_inheritance_and_ordinary_private_dependencies() -> Result<(), String> {
    let sources = source(
        "const fn seed()->i64 => 7\ntest {abstract struct Base<T> {value:T=fixture_seed()}\nstruct Fixture:Base<i64> {}\nconst fn fixture_seed()->i64 => seed()\ntest values {assert Fixture().value==7}}",
    );
    let suite = compile_suite(&sources)?;
    assert_eq!(
        hgl_semantics::harness::execute(
            &suite.tests[0],
            |_| unreachable!(),
            |_, _| unreachable!()
        )?,
        1
    );
    Ok(())
}

#[test]
fn test_type_admission_keeps_other_context_members_excluded() {
    for member in [
        "export struct Hidden {}",
        "export enum Hidden {item}",
        "native type Hidden",
        "use owner",
        "test {test nested {assert true}}",
        "impl fn hidden() {}",
    ] {
        rejected(&source(&format!("test {{{member}}}")), "test context");
    }
}
