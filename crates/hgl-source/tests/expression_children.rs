//! Source walkers retain written ordering and operand locations independently of checking.
use hgl_source::{Cursor, Expr, lex};

#[test]
fn named_arguments_keep_operand_origins_and_written_order() {
    let source = "combine(second: later(), first: earlier())";
    let tokens = lex(source).unwrap();
    let expression = Cursor::new(&tokens).expr().unwrap();
    let operands = expression.children();
    assert_eq!(operands.len(), 2);
    assert_eq!(&source[operands[0].span()], "later()");
    assert_eq!(&source[operands[1].span()], "earlier()");
    assert_eq!(&source[operands[0].argument_span()], "second");
}

#[test]
fn sparse_operands_and_absent_cells_keep_distinct_boundaries() {
    let names = |children: Vec<&Expr>| {
        children
            .into_iter()
            .map(|child| {
                let Expr::Name(name) = child.syntax() else {
                    panic!("expected a name");
                };
                name.clone()
            })
            .collect::<Vec<_>>()
    };
    let name = |value: &str| Expr::Name(value.into());
    let sparse = Expr::Sparse(vec![
        (name("key1"), name("value1")),
        (name("key2"), name("value2")),
    ]);
    assert_eq!(
        names(sparse.children()),
        ["key1", "value1", "key2", "value2"]
    );
    let tuple = Expr::Tuple(vec![Some(name("left")), None, Some(name("right"))]);
    assert_eq!(names(tuple.children()), ["left", "right"]);
}
