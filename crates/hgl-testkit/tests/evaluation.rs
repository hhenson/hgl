//! Shape comparison preserves absence without requiring payload trait implementations.
use hgl_testkit::evaluation::{compare_by, observe};
use hgl_types::EngineTime;

struct Sparse(Vec<(i64, i64)>);
#[test]
fn static_comparison_preserves_presence_without_payload_trait_bounds() {
    let observed = observe(
        vec![(EngineTime::MIN_START, Sparse(vec![(1, 9), (0, 0)]))],
        2,
    )
    .unwrap();
    let same = |a: &Sparse, b: &Sparse| {
        a.0.len() == b.0.len() && a.0.iter().all(|entry| b.0.contains(entry))
    };
    assert!(compare_by(&[Some(Sparse(vec![(0, 0), (1, 9)])), None], &observed, same).is_ok());
    assert!(
        compare_by(&[Some(Sparse(vec![(0, 0)])), None], &observed, same)
            .unwrap_err()
            .contains("cycle 0")
    );
    assert!(
        compare_by(&[None, None], &observed, same)
            .unwrap_err()
            .contains("expected presence false, observed presence true")
    );
    assert!(
        compare_by(&[Some(Sparse(vec![(0, 0), (1, 9)]))], &observed, same)
            .unwrap_err()
            .contains("expected length 1, observed length 2")
    );
}
