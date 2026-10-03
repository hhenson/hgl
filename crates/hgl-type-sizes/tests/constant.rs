//! Type-level sizes use checked scalar expression rules and canonical identities.
use hgl_type_sizes::{literal, normalize};
#[test]
fn canonical_sizes_and_errors() {
    assert_eq!(
        normalize(
            "delta_of(map<i64,tuple<list<i64,2*3-1>,list<str,unbounded>>>)",
            &mut literal
        )
        .unwrap(),
        "delta_of(map<i64,tuple<list<i64,5>,list<str>>>)"
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
