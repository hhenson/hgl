//! Type-level sizes use checked scalar expression rules and canonical identities.
use hgl_type_sizes::{literal, normalize};
#[test]
fn canonical_sizes_and_errors() {
    assert_eq!(
        normalize(
            "delta<map<i64,tuple<list<i64,2*3-1>,list<str,unbounded>>>>",
            &mut literal
        )
        .unwrap(),
        "delta<map<i64,tuple<list<i64,5>,list<str>>>>"
    );
    for (source, fragment) in [
        ("list<i64,1.0>", "constant i64"),
        ("list<i64,(1<2)>", "constant i64"),
        ("list<i64,1-2>", "nonnegative"),
        ("list<i64,9223372036854775807+1>", "overflow"),
        ("list<i64,0%0>", "zero"),
    ] {
        let error = normalize(source, &mut literal).unwrap_err();
        assert!(error.contains(fragment), "{source}: {error}");
    }
}

#[test]
fn schema_bound_collection_does_not_fabricate_or_evaluate_values() {
    assert_eq!(
        hgl_type_sizes::expressions("tuple<rolling<str,width(),0us>,list<i64,size(1)>>"),
        vec!["width()", "0us", "size(1)"]
    );
    assert_eq!(
        hgl_type_sizes::expressions("list<rolling<i64,2>,unbounded>"),
        vec!["2"]
    );
    assert_eq!(
        hgl_type_sizes::expressions("rolling<i64,-1us>"),
        vec!["-1us"]
    );
}
