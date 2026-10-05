//! Sparse ordinary formation validates the whole schema and preserves payload order.
use hgl_delta_check::{Part, constructor};
use hgl_rust_ir::{Kind, Value};
fn fixed(value: Literal) -> (Value, Option<Literal>) {
    (
        Value::new(value.ty(), Kind::Literal(value.clone())),
        Some(value),
    )
}
use hgl_source::{Cursor, Expr, Literal, Ty, lex};
type Parsed = (Ty, Vec<(Option<String>, Expr)>);
fn parsed(source: &str) -> Result<Parsed, String> {
    let tokens = lex(source)?;
    let Expr::Applied(name, args) = Cursor::new(&tokens).expr()? else {
        return Err("expected delta application".into());
    };
    let name = name
        .strip_prefix("delta<")
        .and_then(|name| name.strip_suffix('>'))
        .ok_or("expected delta origin")?;
    Ok((Ty::parse(name).ok_or("expected concrete shape")?, args))
}
#[test]
fn all_collection_forms_and_written_payload_order() {
    for source in [
        "delta<set<bool>>(added:[true],removed:[false])",
        "delta<list<str,3>>(items:[2:\"last\",0:\"first\"])",
        "delta<tuple<i64,str>>(items:[1:\"text\",0:1])",
        "delta<map<i64,i64>>(remove:[9],upsert:[7:one(),8:two()])",
        "delta<map<i64,i64>>()",
        "delta<list<i64,0>>(items:[])",
    ] {
        let (ty, args) = parsed(source).unwrap();
        let parts = constructor(&ty, &args, |expr| {
            expr.fixed().map(fixed).ok_or("constant required".into())
        })
        .unwrap();
        if source.contains("one()") {
            assert!(matches!(
                &parts[0],
                Part::Removed(Value {
                    kind: Kind::Literal(Literal::Int(9)),
                    ..
                })
            ));
            assert!(
                matches!(&parts[1],Part::Keyed(Value {kind: Kind::Literal(Literal::Int(7)), ..},Ty::I64,Expr::Call(name,_)) if name=="one")
            );
            assert!(
                matches!(&parts[2],Part::Keyed(Value {kind: Kind::Literal(Literal::Int(8)), ..},Ty::I64,Expr::Call(name,_)) if name=="two")
            );
        }
    }
}
#[test]
fn rejects_malformed_data_before_any_payload_is_evaluated() {
    for (source, error) in [
        ("delta<set<i64>>(added:[1,1])", "duplicate"),
        ("delta<set<i64>>(added:[1],removed:[1])", "overlap"),
        ("delta<set<i64>>(added:[true])", "type mismatch"),
        (
            "delta<map<i64,i64>>(upsert:[1:call()],remove:[1])",
            "overlap",
        ),
        (
            "delta<map<i64,i64>>(upsert:[1:call(),1:call()])",
            "duplicate",
        ),
        ("delta<map<i64,i64>>(remove:[true])", "constant i64"),
        (
            "delta<map<i64,i64>>(upsert:[dynamic():call()])",
            "constant required",
        ),
        ("delta<list<i64,2>>(items:[2:call()])", "out of bounds"),
        ("delta<tuple<i64,str>>(items:[-1:call()])", "out of bounds"),
        ("delta<list<i64,2>>(items:[0:call()],items:[])", "duplicate"),
        ("delta<map<i64,i64>>(wrong:[])", "unknown"),
        ("delta<map<i64,i64>>([])", "must be named"),
        ("delta<map<i64,i64>>(upsert:[1,2])", "sparse"),
        ("delta<i64>()", "structural"),
    ] {
        let (ty, args) = parsed(source).unwrap();
        let actual = constructor(&ty, &args, |expr| {
            expr.fixed().map(fixed).ok_or("constant required".into())
        })
        .unwrap_err();
        assert!(actual.contains(error), "{source}: {actual}");
    }
}

#[test]
fn scalar_keys_are_exact_and_signed_zero_collisions_fail_during_checking() {
    for source in [
        "delta<set<str>>(added:[\"first\"],removed:[\"second\"])",
        "delta<map<bool,i64>>(upsert:[false:1,true:2])",
        "delta<map<f64,i64>>(upsert:[-1.5:1,0.0:2])",
        "delta<map<date,i64>>(upsert:[@2026-01-01: 1])",
    ] {
        let (ty, args) = parsed(source).unwrap();
        assert!(
            constructor(&ty, &args, |expr| expr
                .fixed()
                .map(fixed)
                .ok_or("constant required".into()))
            .is_ok(),
            "{source}"
        );
    }
    for (source, diagnostic) in [
        ("delta<set<f64>>(added:[0.0,-0.0])", "duplicate"),
        ("delta<set<f64>>(added:[0.0],removed:[-0.0])", "overlap"),
        ("delta<map<f64,i64>>(upsert:[0.0:1,-0.0:2])", "duplicate"),
        (
            "delta<map<f64,i64>>(upsert:[-0.0:1],remove:[0.0])",
            "overlap",
        ),
        ("delta<map<f64,i64>>(upsert:[1:1])", "type mismatch"),
        ("delta<map<i64,i64>>(upsert:[1.0:1])", "type mismatch"),
        ("delta<list<i64,2>>(items:[0.0:1])", "constant i64"),
    ] {
        let (ty, args) = parsed(source).unwrap();
        let error = constructor(&ty, &args, |expr| {
            expr.fixed().map(fixed).ok_or("constant required".into())
        })
        .unwrap_err();
        assert!(error.contains(diagnostic), "{source}: {error}");
    }
}

#[test]
fn provider_dependent_identities_are_retained_without_guessing_duplicate_names() {
    let (ty, args) =
        parsed("delta<set<timezone>>(added:[@[UTC],@[UTC]],removed:[@[UTC]])").unwrap();
    let parts = constructor(&ty, &args, |expr| {
        let Expr::TemporalLiteral(recipe) = expr else {
            return Err("expected contextual recipe".into());
        };
        Ok((
            Value::new(recipe.ty(), Kind::TemporalLiteral(recipe.clone())),
            None,
        ))
    })
    .unwrap();
    assert_eq!(parts.len(), 3);
    assert!(matches!(
        &parts[0],
        Part::Added(Value {
            kind: Kind::TemporalLiteral(_),
            ..
        })
    ));
}
