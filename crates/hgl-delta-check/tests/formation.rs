//! Sparse ordinary formation validates the whole schema and preserves payload order.
use hgl_delta_check::{Part, constructor};
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
            expr.fixed().ok_or("constant required".into())
        })
        .unwrap();
        if source.contains("one()") {
            assert!(matches!(&parts[0], Part::Removed(Literal::Int(9))));
            assert!(matches!(&parts[1],Part::Child(7,Ty::I64,Expr::Call(name,_)) if name=="one"));
            assert!(matches!(&parts[2],Part::Child(8,Ty::I64,Expr::Call(name,_)) if name=="two"));
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
            expr.fixed().ok_or("constant required".into())
        })
        .unwrap_err();
        assert!(actual.contains(error), "{source}: {actual}");
    }
}
