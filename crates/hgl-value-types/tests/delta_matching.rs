//! Pinned ordinary publication delta type and syntax contracts.
use hgl_library::load;
use hgl_source::Ty;
use hgl_value_types::{resolve, substitute, unify};
use std::collections::{BTreeMap, BTreeSet};
#[test]
fn inverse_delta_matching_inside_empty_recording_shape() {
    let library = load(&[("types.hgl".into(), "module test\nstruct TimedValue<T> {\n time:datetime\n value:T\n}\nstruct Quote<T> {\n bid:T\n}\n".into())]).unwrap();
    let actual = resolve(
        &library,
        "test",
        "list<TimedValue<delta_of(map<i64,Quote<f64>>)>>",
        &mut BTreeSet::new(),
    )
    .unwrap();
    let mut bindings = BTreeMap::new();
    unify(
        &library,
        "test",
        "list<TimedValue<delta_of(T)>>",
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
            "delta_of(T)",
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
            "delta_of(T)",
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
        "delta_of(T)",
        &Ty::Str,
        &["T".into()],
        &mut bindings,
    )
    .unwrap();
    assert_eq!(bindings["T"], Ty::Str);
    let a = resolve(&library, "test", "delta_of(A<i64>)", &mut BTreeSet::new()).unwrap();
    let b = resolve(&library, "test", "delta_of(B<i64>)", &mut BTreeSet::new()).unwrap();
    let c = resolve(&library, "test", "delta_of(A<str>)", &mut BTreeSet::new()).unwrap();
    assert_ne!(a, b);
    assert_ne!(a, c);
}
