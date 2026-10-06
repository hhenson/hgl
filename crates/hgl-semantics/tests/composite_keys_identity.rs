//! Exact complete key identity and malformed-data admission.
use hgl_semantics::composite_keys::{key, known};
use hgl_semantics::ir::{Kind, Value};
use hgl_source::{Literal, Ty};
fn literal(value: Literal) -> Value {
    Value::new(value.ty(), Kind::Literal(value))
}
fn structure(name: &str, fields: Vec<(usize, Value)>) -> Value {
    Value::new(
        Ty::Struct(
            name.into(),
            vec![("number".into(), Ty::F64), ("flag".into(), Ty::Bool)],
            vec![1],
        ),
        Kind::Construct(fields),
    )
}
#[test]
fn exact_nominal_presence_positions_and_zero_identity() -> Result<(), String> {
    let zero = || literal(Literal::Float(0.0));
    let a = structure("a::Key", vec![(0, zero())]);
    let same = structure("a::Key", vec![(0, literal(Literal::Float(-0.0)))]);
    assert_eq!(key(&a)?, key(&same)?);
    assert_ne!(key(&a)?, key(&structure("b::Key", vec![(0, zero())]))?);
    let flag = || literal(Literal::Bool(false));
    let present = structure("a::Key", vec![(0, zero()), (1, flag())]);
    let reordered = structure("a::Key", vec![(1, flag()), (0, zero())]);
    assert_ne!(key(&a)?, key(&present)?);
    assert_eq!(key(&present)?, key(&reordered)?);
    let tuple = |a, b| {
        Value::new(
            Ty::Tuple(vec![Ty::I64, Ty::I64]),
            Kind::Construct(vec![
                (0, literal(Literal::Int(a))),
                (1, literal(Literal::Int(b))),
            ]),
        )
    };
    assert_ne!(key(&tuple(1, 2))?, key(&tuple(2, 1))?);
    Ok(())
}
#[test]
fn incomplete_malformed_and_unsupported_components_are_rejected() {
    for value in [
        structure("Key", vec![]),
        structure("Key", vec![(0, literal(Literal::Float(f64::NAN)))]),
        structure("Key", vec![(0, literal(Literal::Int(1)))]),
        structure("Key", vec![(2, literal(Literal::Bool(true)))]),
        structure(
            "Key",
            vec![
                (0, literal(Literal::Float(1.0))),
                (0, literal(Literal::Float(2.0))),
            ],
        ),
    ] {
        assert!(key(&value).is_err(), "{value:?}");
    }
    for ty in [
        Ty::List(Box::new(Ty::I64), Some(1)),
        Ty::Set(Box::new(Ty::I64)),
        Ty::Ref(Box::new(Ty::I64)),
        Ty::Atomic(Box::new(Ty::I64)),
    ] {
        assert!(!ty.collection_key());
        assert!(!Ty::Tuple(vec![ty]).collection_key());
    }
}
#[test]
fn nested_provider_leaves_have_no_guessed_identity() -> Result<(), String> {
    let tokens = hgl_source::lex("@[UTC]")?;
    let hgl_source::Expr::TemporalLiteral(recipe) = hgl_source::Cursor::new(&tokens).expr()? else {
        return Err("provider recipe required".into());
    };
    let value = Value::new(
        Ty::Tuple(vec![recipe.ty(), Ty::I64]),
        Kind::Construct(vec![
            (0, Value::new(recipe.ty(), Kind::TemporalLiteral(recipe))),
            (1, literal(Literal::Int(2))),
        ]),
    );
    assert!(known(&value)?.is_none());
    assert!(key(&value).is_err());
    Ok(())
}
