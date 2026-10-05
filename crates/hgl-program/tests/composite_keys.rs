//! Complete composite sparse keys; generated runtime coverage is integrated separately.
use hgl_program::compile_tests;
fn support(mut sources: Vec<(String, String)>) -> Vec<(String, String)> {
    sources.push((
        "replay.hgl".into(),
        include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
    ));
    sources.push((
        "replay_impl.hgl".into(),
        include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
    ));
    sources
}
fn source(body: &str) -> Vec<(String, String)> {
    support(vec![(
        "keys.hgl".into(),
        format!(
            "module hgraph.std part keys_tests\nstruct Key {{number:i64\nflag:bool=null}}\nstruct Other {{number:i64\nflag:bool=null}}\ntest {{ fn pass<T>(value:T)->T {{when {{return delta_value(value)}}}}\ntest sample {{{body}}}\n}}"
        ),
    )])
}
#[test]
fn all_shared_composite_cases_typecheck() -> Result<(), String> {
    let shared = include_str!("fixtures/composite_key_values.hgl");
    let sources = vec![
        ("composite.hgl".into(), shared.into()),
        (
            "pass.hgl".into(),
            "module hgraph.std\nfn pass_through<T>(value:T)->T {when {return delta_value(value)}}"
                .into(),
        ),
    ];
    compile_tests(&support(sources))?;
    Ok(())
}
#[test]
fn complete_key_constants_preserve_aliases_presence_and_nested_tuples() -> Result<(), String> {
    compile_tests(&source(
        r#"
        let key: Key = Key(number:1)
        let delta = delta<map<Key,i64>>(upsert:[key:1,Key(number:1,flag:false):2])
        assert eval(pass, [delta,delta<map<Key,i64>>(remove:[Key(number:1,flag:null)])]) == [delta,delta<map<Key,i64>>(remove:[key])]
        assert eval(pass,[delta<set<tuple<i64,tuple<str,bool>>>>(added:[(1,("a",false))])]) == [delta<set<tuple<i64,tuple<str,bool>>>>(added:[(1,("a",false))])]
    "#,
    ))?;
    Ok(())
}
#[test]
fn duplicate_overlap_wrong_nominal_and_nonconstant_keys_fail() {
    for expression in [
        "delta<set<Key>>(added:[Key(number:1),Key(number:1,flag:null)])",
        "delta<map<Key,i64>>(upsert:[Key(number:1):1],remove:[Key(number:1)])",
        "delta<set<tuple<f64,str>>>(added:[(0.0,\"a\"),(-0.0,\"a\")])",
        "delta<set<Key>>(added:[Other(number:1)])",
        "delta<set<tuple<f64,str>>>(added:[(1,\"a\")])",
        "delta<set<tuple<list<i64>>>>(added:[([1],)])",
    ] {
        let result = compile_tests(&source(&format!("let value = {expression}")));
        assert!(result.is_err(), "admitted {expression}");
    }
    assert!(
        compile_tests(&source(
            "var key:Key=Key(number:1)\nlet value=delta<set<Key>>(added:[key])"
        ))
        .is_err()
    );
}

#[test]
fn compound_provider_keys_keep_cold_alias_provenance() -> Result<(), String> {
    compile_tests(&source(
        r#"
        let zone = @[UTC]
        let key: tuple<timezone,str> = (zone,"x")
        let value = delta<set<tuple<timezone,str>>>(added:[key,(@[UTC],"x")])
    "#,
    ))?;
    Ok(())
}
