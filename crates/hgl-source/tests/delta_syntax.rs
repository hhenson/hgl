//! Pinned ordinary publication delta type and syntax contracts.
use hgl_source::{Cursor, Expr, lex};
#[test]
fn contextual_types_and_ordered_sparse_entries() {
    let tokens = lex("delta_of(map<i64, tuple<i64, str>>)").unwrap();
    assert_eq!(
        Cursor::new(&tokens).type_name().unwrap(),
        "delta_of(map<i64,tuple<i64,str>>)"
    );
    let tokens =
        lex("delta<map<i64, i64>>(remove: [9], upsert: [7: first(), 8: second()])").unwrap();
    let Expr::Applied(name, args) = Cursor::new(&tokens).expr().unwrap() else {
        panic!("applied delta");
    };
    assert_eq!(name, "delta<map<i64,i64>>");
    assert_eq!(args[0].0.as_deref(), Some("remove"));
    let Expr::Sparse(entries) = &args[1].1 else {
        panic!("sparse entries");
    };
    assert_eq!(entries.len(), 2);
    assert!(matches!(&entries[0].1, Expr::Call(name, _) if name == "first"));
    assert!(matches!(&entries[1].1, Expr::Call(name, _) if name == "second"));
}
#[test]
fn sparse_is_not_general_literal_or_value_level_delta_of() {
    let tokens = lex("[1: 2]").unwrap();
    assert!(Cursor::new(&tokens).expr().is_err());
    let tokens = lex("delta_of(2)").unwrap();
    assert!(
        matches!(Cursor::new(&tokens).expr().unwrap(), Expr::Call(name, _) if name == "delta_of")
    );
}
