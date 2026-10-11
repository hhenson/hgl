//! Selection must omit unrelated generated graphs and preserve nested references.
use hgl_program::{compile_tests, emit_tests};

#[test]
fn selection_emits_only_reachable_graphs() -> Result<(), String> {
    let definitions = "module selected\nfn id(value:i64)->i64 {when {return value}}\nfn fail()->i64 {yield -1us:1}\n";
    let selected = "test retained {let captured=eval(id,[2])\nassert len(captured)==1\nif true {assert eval(id,[3])==[3]} else {eval(id,[4])}\nassert raises(\"yield.negative_duration\") {eval(fail)}}";
    let all = format!(
        "{definitions}test discarded {{eval(id,[1])}}\n{selected}\ntest last {{eval(id,[5])}}"
    );
    let source = |body: String| {
        vec![
            ("selected.hgl".into(), body),
            (
                "replay.hgl".into(),
                include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
            ),
            (
                "replay_impl.hgl".into(),
                include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl")
                    .into(),
            ),
        ]
    };
    let mut suite = compile_tests(&source(all))?;
    let original = emit_tests(&suite);
    assert!(suite.select(&["unknown"]).is_err());
    assert_eq!(emit_tests(&suite), original);
    suite.select(&[])?;
    assert_eq!(emit_tests(&suite), original);
    suite.select(&["retained"])?;
    assert_eq!(suite.tests.len(), 1);
    assert_eq!(
        suite.plans.len(),
        4,
        "unselected graph plans must be omitted"
    );
    let independent = compile_tests(&source(format!("{definitions}{selected}")))?;
    assert_eq!(emit_tests(&suite), emit_tests(&independent));
    suite.select(&["selected::retained"])?;
    assert_eq!(emit_tests(&suite), emit_tests(&independent));

    let scalar = "test scalar {assert true}";
    let mut suite = compile_tests(&source(format!(
        "{definitions}test discarded {{eval(id,[1])}}\n{scalar}"
    )))?;
    suite.select(&["scalar"])?;
    assert!(suite.plans.is_empty());
    assert_eq!(
        emit_tests(&suite),
        emit_tests(&compile_tests(&source(format!("{definitions}{scalar}")))?)
    );
    Ok(())
}
