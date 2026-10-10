//! One balanced application grammar preserves finite-list and nested argument boundaries.
use hgl_source::type_shape::{Ty, application};

#[test]
fn list_bounds_share_complete_nested_application_boundaries() {
    for (source, element, size) in [
        ("list<i64>", "i64", None),
        ("list<i64,unbounded>", "i64", None),
        (
            "list<map<tuple<i64,str>,list<str,2>>,3>",
            "map<tuple<i64,str>,list<str,2>>",
            Some(3),
        ),
        (
            "list<Box<list<i64,(choose(1,2))>>,0>",
            "Box<list<i64,(choose(1,2))>>",
            Some(0),
        ),
    ] {
        assert_eq!(Ty::list_parts(source), Some((element, size)));
    }
    assert_eq!(
        application("list<i64,(choose(1,2))>"),
        Some(("list", vec!["i64", "(choose(1,2))"]))
    );
    for source in [
        "list<i64,-1>",
        "list<i64,1,2>",
        "list<i64>>",
        "list<list<i64>",
        "list<i64,(1>",
    ] {
        assert!(Ty::list_parts(source).is_none(), "{source}");
    }
}
