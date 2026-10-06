//! Pinned ordinary publication delta type and syntax contracts.
use hgl_semantics::library::load;
use hgl_semantics::value_types::{resolve, substitute, unify};
use hgl_source::Ty;
use std::collections::{BTreeMap, BTreeSet};
#[test]
fn inverse_delta_matching_inside_empty_recording_shape() {
    let library = load(&[("types.hgl".into(), "module test\nstruct TimedValue<T> {\n time:datetime\n value:T\n}\nstruct Quote<T> {\n bid:T\n}\n".into())]).unwrap();
    let actual = resolve(
        &library,
        "test",
        "list<TimedValue<delta<map<i64,Quote<f64>>>>>",
        &mut BTreeSet::new(),
    )
    .unwrap();
    let mut bindings = BTreeMap::new();
    unify(
        &library,
        "test",
        "list<TimedValue<delta<T>>>",
        &actual,
        &["T".into()],
        &mut bindings,
    )
    .unwrap();
    assert_eq!(bindings["T"].source_name(), "map<i64,test::Quote<f64>>");
    assert_eq!(
        substitute(
            &library,
            "test",
            "delta<T>",
            &bindings,
            &mut BTreeSet::new()
        )
        .unwrap(),
        bindings["T"].clone().delta().unwrap()
    );
    assert!(
        unify(
            &library,
            "test",
            "delta<T>",
            &Ty::I64,
            &["T".into()],
            &mut bindings
        )
        .is_err()
    );
}
#[test]
fn scalar_inverse_is_exact_and_nominal_identity_survives() {
    let library = load(&[(
        "types.hgl".into(),
        "module test\nstruct A<T> {\n x:i64\n}\nstruct B<T> {\n x:i64\n}\n".into(),
    )])
    .unwrap();
    let mut bindings = BTreeMap::new();
    unify(
        &library,
        "test",
        "delta<T>",
        &Ty::Str,
        &["T".into()],
        &mut bindings,
    )
    .unwrap();
    assert_eq!(bindings["T"], Ty::Str);
    let a = resolve(&library, "test", "delta<A<i64>>", &mut BTreeSet::new()).unwrap();
    let b = resolve(&library, "test", "delta<B<i64>>", &mut BTreeSet::new()).unwrap();
    let c = resolve(&library, "test", "delta<A<str>>", &mut BTreeSet::new()).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, c);
}

#[test]
fn legacy_type_marker_is_not_an_alias_inside_generic_arguments() {
    let library = load(&[(
        "types.hgl".into(),
        "module test\nstruct Box<T>{value:T}".into(),
    )])
    .unwrap();
    for source in ["delta_of(i64)", "list<delta_of(i64)>", "Box<delta_of(i64)>"] {
        let error = resolve(&library, "test", source, &mut BTreeSet::new()).unwrap_err();
        assert!(
            error.contains("unresolved ordinary type delta_of(i64)"),
            "{source}: {error}"
        );
    }
}
