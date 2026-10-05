//! Exact ordinary delta source contracts; runtime execution lives in emitted fixtures.
use hgl_program::compile;
fn check(source: &str) -> Result<(), String> {
    compile(
        &[("delta.hgl".into(), format!("module tests\n{source}"))],
        "main",
    )
    .map(|_| ())
}
#[test]
fn data_helpers_empty_retention_and_generic_inverse() {
    check(
        r"
struct TimedValue<T> { time:datetime
value:delta<T> }
operator replay<T>(const values:list<TimedValue<T>>)->T
impl fn replay<T>(const values:list<TimedValue<T>>)->T {
 var index=0
 while index<len(values) { yield values[index].time:values[index].value
 index+=1 }
}
instantiate replay<map<i64,i64>>
const fn values()->list<TimedValue<map<i64,i64>>> {
 var values:list<TimedValue<map<i64,i64>>> = []
 var data:delta<map<i64,i64>> =delta<map<i64,i64>>()
 push(values,TimedValue(time:@1970-01-01T00:00:00.000001Z,value:data))
 data=delta<map<i64,i64>>(upsert:[1:2])
 return values
}
fn main()->map<i64,i64> => replay(values())
",
    )
    .unwrap();
}
#[test]
fn recursive_publications_observation_and_explicit_retention() {
    check(r#"
struct Quote { bid:i64
 ask:str }
struct Held<T> { value:T }
fn source()->map<i64,tuple<list<Quote,2>,set<bool>>> {
 yield 0us:delta<map<i64,tuple<list<Quote,2>,set<bool>>>>(upsert:[1:delta<tuple<list<Quote,2>,set<bool>>>(items:[0:delta<list<Quote,2>>(items:[1:delta<Quote>(bid:1)])])])
}
fn pass<T>(value:T)->T {
 inject global_state, out
 when {
 let observed:delta<T> =delta_value(value)
 let alias=observed
 let owner=Held(value:alias)
 var copy=owner
 copy.value=observed
 set(global_state,"data",copy)
 out=observed
 return alias
 }
}
fn main()->map<i64,tuple<list<Quote,2>,set<bool>>> => pass(source())
"#).unwrap();
}
#[test]
fn observation_authority_and_no_delta_inspection() {
    for (body, error) in [
        (
            "var d:delta<map<i64,i64>> =delta_value(value)",
            "observation cannot initialize writable",
        ),
        (
            "let d=delta_value(value)\nhelper(d)",
            "observation cannot escape",
        ),
        ("let d=delta_value(value)\nlet e=d[0]", "indexing requires"),
        (
            "let d=delta_value(value)\nlet e=d.field",
            "field access requires",
        ),
        ("let d=delta_value(value)\nlet e=d==d", "unsupported binary"),
        (
            "let d=delta<list<i64,2>>(items:[0:1])\nout=d",
            "assignment type mismatch",
        ),
    ] {
        let source = format!(
            r"
const fn helper(value:delta<map<i64,i64>>)->i64 => 1
fn source()->map<i64,i64> {{ yield 0us:delta<map<i64,i64>>(upsert:[1:2]) }}
fn target(value:map<i64,i64>)->map<i64,i64> {{ inject out
when {{ {body} }} }}
fn main()->map<i64,i64> => target(source())
"
        );
        let actual = check(&source).unwrap_err();
        assert!(actual.contains(error), "{body}: {actual}");
    }
}
#[test]
fn malformed_constructors_and_incompatible_origins() {
    for (value, error) in [
        ("delta<list<i64,2>>(items:[2:1])", "out of bounds"),
        ("delta<map<i64,i64>>(upsert:[1:1],remove:[1])", "overlap"),
        ("delta<set<bool>>(added:[true,true])", "duplicate"),
        ("delta<tuple<i64,str>>(items:[1:2])", "child type mismatch"),
        (
            "delta<map<tuple<bool>,i64>>()",
            "unsupported publication shape",
        ),
        ("delta<list<i64>>()", "unsupported publication shape"),
    ] {
        let actual=check(&format!("fn source()->i64 {{ start {{ let d={value} }}\nwhen {{return 1}} }}\nfn main()->i64=>source()")).unwrap_err();
        assert!(actual.contains(error), "{value}: {actual}");
    }
}

#[test]
fn constant_size_expressions_share_exact_type_identity() {
    check(
        r"
const fn size(value:i64)->i64 => value*2+1
struct Quote { value:list<i64,size(1)> }
fn source(const count:i64)->list<i64,count*2+1> {
 let quote:delta<Quote> =delta<Quote>(value:delta<list<i64,1+2>>(items:[0:1]))
 let data:delta<list<i64,size(count)>> =delta<list<i64,count*2+1>>(items:[count*2:1])
 yield 0us:data
}
fn forward(value:list<i64,5>)->list<i64,(2+3)> { when { return delta_value(value) } }
fn main()->list<i64,5> => forward(source(2))
",
    )
    .unwrap();
    for (size, error) in [
        ("1.0", "constant i64"),
        ("(1<2)", "constant i64"),
        ("1-2", "nonnegative"),
        ("9223372036854775807+1", "overflow"),
        ("unknown", "unknown value"),
    ] {
        let source = format!(
            "fn source()->list<i64,{size}> {{yield 0us:delta<list<i64,1>>(items:[0:1])}}\nfn main()->list<i64,1> =>source()"
        );
        let actual = check(&source).unwrap_err();
        assert!(actual.contains(error), "{size}: {actual}");
    }
}

#[test]
fn imported_generic_family_does_not_export_private_application_arguments() {
    let sources=vec![
        ("root.hgl".into(),"module root\nuse family::{Box,Phantom}\nstruct Private { value:i64 }\nfn main() { let data=Box(value:Private(value:1))\nlet phantom=Phantom<Private>(value:2) }".into()),
        ("family.hgl".into(),"module family\nexport struct Box<T> {value:T}\nexport struct Phantom<T> {value:i64}".into())
    ];
    assert!(compile(&sources, "main").is_ok());
    let mut leaking = sources;
    leaking[0].1="module root\nuse family::{Box}\nstruct Private { value:i64 }\nexport struct Public { value:Box<Private> }\nfn main() { let data=Public(value:Box(value:Private(value:1))) }".into();
    let error = compile(&leaking, "main").unwrap_err();
    assert!(
        error.contains("exported struct reaches unexported root::Private"),
        "{error}"
    );
}

#[test]
fn zero_size_list_data_checks_before_runtime_publication_validation() {
    for slots in ["[]", "[_,_]"] {
        let source = format!(
            "fn pass(value:list<i64,0>)->list<i64,0> {{when {{return delta_value(value)}}}}\ntest empty {{assert eval(pass,{slots})=={slots}}}"
        );
        let suite = hgl_program::compile_tests(&with_std(&source)).unwrap();
        assert!(!hgl_program::emit_tests(&suite).contains("empty structural publication"));
    }
    let source = "fn pass(value:list<i64,0>)->list<i64,0> {when {return delta_value(value)}}\ntest empty {eval(pass,[delta<list<i64,0>>()])}";
    // Empty delta data is valid; structural_evaluation checks its rejection before start.
    hgl_program::compile_tests(&with_std(source)).unwrap();
    let error = check("fn main() {let data=delta<list<i64,0>>(items:[0:1])}").unwrap_err();
    assert!(error.contains("out of bounds"), "{error}");
}

#[test]
fn generic_delta_origins_require_exact_arguments_and_sizes() {
    for (first, second) in [
        ("A<i64>", "A<str>"),
        ("A<i64>", "B<i64>"),
        ("list<i64,2>", "list<i64,3>"),
    ] {
        let source = format!(
            "struct A<T> {{value:i64}}\nstruct B<T> {{value:i64}}\nconst fn same<T>(first:delta<T>,second:delta<T>)->i64 => 1\nfn main() {{let value=same(delta<{first}>(),delta<{second}>())}}"
        );
        let error = check(&source).unwrap_err();
        assert!(
            error.contains("expected one matching declaration"),
            "{error}"
        );
    }
}

fn with_std(body: &str) -> Vec<(String, String)> {
    vec![
        ("delta.hgl".into(), format!("module tests\n{body}")),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}

#[test]
fn defaulted_delta_origins_check_defaults_without_filling_sparse_fields() {
    for literal in ["delta<Box>(amount:1)", "delta<Box>()"] {
        let source = format!(
            "struct Box {{ amount:i64=true }}\nfn source()->i64 {{ start {{ let data={literal} }}\nwhen {{ return 1 }} }}\nfn main()->i64=>source()"
        );
        let error = check(&source).unwrap_err();
        assert!(error.contains("default type mismatch"), "{error}");
    }
    check("struct Box { amount:i64=1 }\nfn source()->i64 { start { let empty=delta<Box>() }\nwhen { return 1 } }\nfn main()->i64=>source()").unwrap();
}
