//! ADR0015 nested timed statements and hard reserved words.
use hgl_source::{Cursor, Expr, Literal, Stmt, lex};

#[test]
fn nested_yields_preserve_time_and_payload_positions() {
    let tokens = lex("{ var i=0\nwhile i<2 { if i==0 { yield 1us:10 } else { yield @2026-10-03T00:00:00Z:20 }\ni+=1 }\nwhile { return } }").unwrap();
    let statements = Cursor::new(&tokens).block().unwrap();
    let Stmt::While(Expr::Binary(comparison, _, _), body) = &statements[1] else {
        panic!("expected conditional loop")
    };
    assert_eq!(comparison, "<");
    let Stmt::If(_, yes, no) = &body[0] else {
        panic!("expected nested branches")
    };
    assert!(matches!(
        &yes[0],
        Stmt::TimedYield(
            Expr::Literal(Literal::Duration(1)),
            Expr::Literal(Literal::Int(10))
        )
    ));
    assert!(matches!(
        &no[0],
        Stmt::TimedYield(
            Expr::Literal(Literal::DateTime(_)),
            Expr::Literal(Literal::Int(20))
        )
    ));
    assert!(
        matches!(&statements[2], Stmt::While(Expr::Literal(Literal::Bool(true)), body) if matches!(body[0], Stmt::Exit))
    );
}

#[test]
fn reserved_words_cannot_be_declared_as_names() {
    for source in ["{ let yield=1 }", "{ var while=1 }", "{ let x:yield=1 }"] {
        let tokens = lex(source).unwrap();
        assert!(
            Cursor::new(&tokens)
                .block()
                .unwrap_err()
                .contains("reserved word")
        );
    }
}

#[test]
fn yield_pair_requires_its_separator() {
    let tokens = lex("{ yield 1us 10 }").unwrap();
    assert!(
        Cursor::new(&tokens)
            .block()
            .unwrap_err()
            .contains("expected :")
    );
}
