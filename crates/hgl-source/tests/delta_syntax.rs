//! Pinned ordinary publication delta type and syntax contracts.
use hgl_source::{Cursor, Expr, lex};
#[test]
fn contextual_types_and_ordered_sparse_entries() {
    let tokens = lex("delta<map<i64, tuple<i64, str>>>").unwrap();
    assert_eq!(
        Cursor::new(&tokens).type_name().unwrap(),
        "delta<map<i64,tuple<i64,str>>>"
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
fn sparse_is_not_general_literal_or_accessor() {
    let tokens = lex("[1: 2]").unwrap();
    assert!(Cursor::new(&tokens).expr().is_err());
    let tokens = lex("delta(2)").unwrap();
    assert!(matches!(Cursor::new(&tokens).expr().unwrap(), Expr::Call(name, _) if name == "delta"));
}

#[test]
fn nested_delta_annotations_share_constructor_marker_syntax() {
    let tokens =
        lex("{let value:list<TimedValue<delta<\nmap<i64,tuple<i64,str>>\n>>> = []}").unwrap();
    let body = Cursor::new(&tokens).block().unwrap();
    assert!(
        matches!(&body[0], hgl_source::Stmt::Let(_, Some(ty), _) if ty == "list<TimedValue<delta<map<i64,tuple<i64,str>>>>>")
    );
    let tokens = lex("{let delta=1\nlet value=delta}").unwrap();
    assert!(Cursor::new(&tokens).block().is_ok());
    let tokens = lex("{let value:delta_of(i64)=1}").unwrap();
    let error = Cursor::new(&tokens).block().unwrap_err();
    assert!(error.message.contains("expected =, found ("), "{error}");
}
