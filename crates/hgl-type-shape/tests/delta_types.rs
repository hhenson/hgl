//! Pinned ordinary publication delta type and syntax contracts.
use hgl_type_shape::Ty;
#[test]
fn exact_derived_shapes_and_scalar_reduction() {
    for scalar in [
        "bool", "i64", "f64", "str", "date", "time", "datetime", "duration",
    ] {
        assert_eq!(Ty::parse(&format!("delta<{scalar}>")), Ty::parse(scalar));
    }
    for source in [
        "map<i64,tuple<i64,set<bool>>>",
        "list<map<i64,str>,3>",
        "list<i64>",
        "list<list<str>>",
    ] {
        let ty = Ty::parse(source);
        let ty = ty.unwrap();
        assert!(ty.publication());
        assert_eq!(ty.source_name(), source);
        let delta = ty.clone().delta().unwrap();
        assert_eq!(delta.source_name(), format!("delta<{source}>"));
        assert!(!delta.publication());
    }
    assert_ne!(
        Ty::parse("delta<list<i64,2>>"),
        Ty::parse("delta<list<i64,3>>")
    );
}
#[test]
fn unsupported_origins_fail_formation() {
    for source in [
        "list<ref<i64>>",
        "set<list<str,2>>",
        "map<tuple<bool>,i64>",
        "ref<i64>",
        "list<i64,-1>",
        "delta<set<i64>>",
    ] {
        assert!(Ty::parse(&format!("delta<{source}>")).is_none(), "{source}");
    }
}

#[test]
fn only_one_complete_type_argument_forms_a_delta_marker() {
    for source in [
        "delta<>",
        "delta<i64,str>",
        "delta<i64",
        "delta<i64>>",
        "delta_of(i64)",
        "list<delta_of(i64)>",
    ] {
        assert!(Ty::parse(source).is_none(), "{source}");
    }
    assert_eq!(Ty::parse("list<delta<i64>>"), Ty::parse("list<i64>"));
    let nested = Ty::parse("list<delta<tuple<i64,set<bool>>>>").unwrap();
    assert_eq!(nested.source_name(), "list<delta<tuple<i64,set<bool>>>>");
}
