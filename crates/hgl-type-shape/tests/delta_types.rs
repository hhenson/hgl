//! Pinned ordinary publication delta type and syntax contracts.
use hgl_type_shape::Ty;
#[test]
fn exact_derived_shapes_and_scalar_reduction() {
    for scalar in [
        "bool", "i64", "f64", "str", "date", "time", "datetime", "duration",
    ] {
        assert_eq!(Ty::parse(&format!("delta_of({scalar})")), Ty::parse(scalar));
    }
    for source in ["map<i64,tuple<i64,set<bool>>>", "list<map<i64,str>,3>"] {
        let ty = Ty::parse(source);
        let ty = ty.unwrap();
        assert!(ty.publication());
        assert_eq!(ty.source_name(), source);
        let delta = ty.clone().delta().unwrap();
        assert_eq!(delta.source_name(), format!("delta_of({source})"));
        assert!(!delta.publication());
    }
    assert_ne!(
        Ty::parse("delta_of(list<i64,2>)"),
        Ty::parse("delta_of(list<i64,3>)")
    );
}
#[test]
fn unsupported_origins_fail_formation() {
    for source in [
        "list<i64>",
        "set<str>",
        "map<bool,i64>",
        "ref<i64>",
        "list<i64,-1>",
        "delta_of(set<i64>)",
    ] {
        assert!(
            Ty::parse(&format!("delta_of({source})")).is_none(),
            "{source}"
        );
    }
}
