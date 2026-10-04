//! Canonical source spellings preserve scalar identities and composite boundaries.
use hgl_type_shape::Ty;

#[test]
fn all_admitted_scalar_wrappers_normalize_recursively() {
    for scalar in [
        "bool", "i64", "f64", "str", "date", "time", "datetime", "duration",
    ] {
        assert_eq!(Ty::parse(&format!("atomic<{scalar}>")), Ty::parse(scalar));
        assert_eq!(
            Ty::parse(&format!("list<atomic<{scalar}>,2>")),
            Ty::parse(&format!("list<{scalar},2>"))
        );
        assert_eq!(
            Ty::parse(&format!("delta<atomic<{scalar}>>")),
            Ty::parse(scalar)
        );
    }
}
#[test]
fn complete_payload_and_structural_delta_are_different_types() {
    for payload in [
        Ty::List(Box::new(Ty::I64), None),
        Ty::List(Box::new(Ty::I64), Some(0)),
        Ty::Tuple(vec![Ty::I64]),
        Ty::Struct("Empty".into(), vec![]),
        Ty::Struct("Single".into(), vec![("value".into(), Ty::I64)]),
    ] {
        let atomic = payload.clone().atomic();
        assert_ne!(atomic, payload);
        assert_eq!(atomic.delta(), Ok(payload));
    }
    assert!(Ty::Set(Box::new(Ty::I64)).atomic().delta().is_err());
    assert!(Ty::parse("atomic<civil_datetime>").is_none());
}
