//! Typed var declarations carry no fabricated initializer.
use hgl_source::{Cursor, Stmt, lex};
#[test]
fn typed_var_without_initializer_has_an_explicit_declaration_form() {
    let tokens = lex("{var selected:ref<i64>\nselected = route(value)\nselected}").unwrap();
    let body = Cursor::new(&tokens).block().unwrap();
    assert!(
        matches!(&body[0], Stmt::DeclareVar(name, ty) if name == "selected" && ty == "ref<i64>")
    );
    assert!(matches!(&body[1], Stmt::Assign(..)));
    let tokens = lex("{var value:i64 = 1}").unwrap();
    assert!(
        matches!(&Cursor::new(&tokens).block().unwrap()[0], Stmt::Var(_, Some(ty), _) if ty == "i64")
    );
}
#[test]
fn untyped_var_or_let_still_requires_an_initializer() {
    for source in ["{var value\n}", "{let value:i64\n}", "{let value\n}"] {
        let tokens = lex(source).unwrap();
        assert!(Cursor::new(&tokens).block().is_err(), "{source}");
    }
}
