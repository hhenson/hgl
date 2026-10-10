//! Recursive publication admission starts fresh and retains membership history.
use hgl_semantics::eval_data::validate;
use hgl_semantics::ir::{DeltaEntry, Kind, Value};
use hgl_source::{Literal, Ty};

fn patch(shape: &Ty, entries: Vec<DeltaEntry>) -> Value {
    Value::new(Ty::Delta(Box::new(shape.clone())), Kind::Delta(entries))
}
#[test]
fn recursive_memberships_survive_sparse_gaps_and_reset_on_reinsertion() {
    let set = Ty::Set(Box::new(Ty::Bool));
    let map = Ty::Map(Box::new(Ty::I64), Box::new(set.clone()));
    let fixed = Ty::List(Box::new(map.clone()), Some(2));
    let update = |entries| patch(&fixed, vec![DeltaEntry::Child(1, patch(&map, entries))]);
    let add = || {
        update(vec![DeltaEntry::Keyed(
            Value::new(Ty::I64, Kind::Literal(Literal::Int(7))),
            patch(
                &set,
                vec![DeltaEntry::Add(Value::new(
                    Ty::Bool,
                    Kind::Literal(Literal::Bool(false)),
                ))],
            ),
        )])
    };
    let remove_member = || {
        update(vec![DeltaEntry::Keyed(
            Value::new(Ty::I64, Kind::Literal(Literal::Int(7))),
            patch(
                &set,
                vec![DeltaEntry::Remove(Value::new(
                    Ty::Bool,
                    Kind::Literal(Literal::Bool(false)),
                ))],
            ),
        )])
    };
    let remove_key = || {
        update(vec![DeltaEntry::Remove(Value::new(
            Ty::I64,
            Kind::Literal(Literal::Int(7)),
        ))])
    };
    assert!(
        validate(
            &fixed,
            &[
                Some(add()),
                None,
                Some(remove_member()),
                Some(remove_key()),
                Some(add())
            ]
        )
        .is_ok()
    );
    let redundant = validate(&fixed, &[Some(add()), None, Some(add())]).unwrap_err();
    assert_eq!(redundant, (2, "set addition is already present".into()));
    let stale_child = validate(
        &fixed,
        &[Some(add()), Some(remove_key()), Some(remove_member())],
    )
    .unwrap_err();
    assert_eq!(stale_child, (2, "set removal is absent".into()));
}
#[test]
fn nested_empty_data_is_admitted_without_skipping_canonical_checks() {
    let set = Ty::Set(Box::new(Ty::I64));
    let tuple = Ty::Tuple(vec![set.clone(), Ty::Str]);
    let empty_child = patch(&tuple, vec![DeltaEntry::Child(0, patch(&set, vec![]))]);
    assert!(
        validate(
            &tuple,
            &[None, Some(empty_child.clone()), Some(empty_child)]
        )
        .is_ok()
    );
    assert!(validate(&tuple, &[None, None]).is_ok());
    let equal = patch(
        &tuple,
        vec![DeltaEntry::Child(
            1,
            Value::new(Ty::Str, Kind::Literal(Literal::Str(String::new()))),
        )],
    );
    assert!(validate(&tuple, &[Some(equal.clone()), Some(equal)]).is_ok());
}

#[test]
fn composite_membership_rejects_redundant_additions_and_reinserts_exact_keys() {
    let key = |flag: bool| {
        Value::new(
            Ty::Tuple(vec![Ty::I64, Ty::Bool]),
            Kind::Construct(vec![
                (0, Value::new(Ty::I64, Kind::Literal(Literal::Int(1)))),
                (1, Value::new(Ty::Bool, Kind::Literal(Literal::Bool(flag)))),
            ]),
        )
    };
    let shape = Ty::Set(Box::new(key(false).ty));
    let add = |flag| patch(&shape, vec![DeltaEntry::Add(key(flag))]);
    let remove = || patch(&shape, vec![DeltaEntry::Remove(key(false))]);
    assert!(
        validate(
            &shape,
            &[
                Some(add(false)),
                Some(add(true)),
                None,
                Some(remove()),
                Some(add(false))
            ]
        )
        .is_ok()
    );
    assert_eq!(
        validate(&shape, &[Some(add(false)), None, Some(add(false))]).unwrap_err(),
        (2, "set addition is already present".into())
    );
    assert_eq!(
        validate(&shape, &[Some(add(true)), Some(remove())]).unwrap_err(),
        (1, "set removal is absent".into())
    );
}
