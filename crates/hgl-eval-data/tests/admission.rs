//! Recursive publication admission starts fresh and retains membership history.
use hgl_eval_data::validate;
use hgl_rust_ir::{DeltaEntry, Kind, Value};
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
        update(vec![DeltaEntry::Child(
            7,
            patch(&set, vec![DeltaEntry::Add(Literal::Bool(false))]),
        )])
    };
    let remove_member = || {
        update(vec![DeltaEntry::Child(
            7,
            patch(&set, vec![DeltaEntry::Remove(Literal::Bool(false))]),
        )])
    };
    let remove_key = || update(vec![DeltaEntry::Remove(Literal::Int(7))]);
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
fn nested_empty_data_is_not_a_publication_and_silence_stays_silent() {
    let set = Ty::Set(Box::new(Ty::I64));
    let tuple = Ty::Tuple(vec![set.clone(), Ty::Str]);
    let empty_child = patch(&tuple, vec![DeltaEntry::Child(0, patch(&set, vec![]))]);
    assert_eq!(
        validate(&tuple, &[None, Some(empty_child)]).unwrap_err(),
        (1, "empty structural publication".into())
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
