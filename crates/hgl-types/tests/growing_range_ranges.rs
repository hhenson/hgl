//! Canonical ranges are independent of written index order.
use hgl_types::growing_range::validate;
#[test]
fn unordered_dense_appends_and_complete_tail_removals() {
    for (length, items, removed) in [
        (0, vec![1, 0], vec![]),
        (2, vec![2, 1], vec![]),
        (3, vec![0], vec![2, 1]),
        (1, vec![], vec![0]),
        (0, vec![0], vec![]),
    ] {
        assert_eq!(
            validate(length, items.iter().copied(), removed.iter().copied()),
            Ok(())
        );
    }
}
#[test]
fn gaps_duplicates_invalid_tails_and_overlap_are_rejected() {
    for (length, items, removed) in [
        (0, vec![1], vec![]),
        (2, vec![3], vec![]),
        (0, vec![0, 0], vec![]),
        (2, vec![], vec![0]),
        (2, vec![], vec![2]),
        (3, vec![], vec![1, 1]),
        (3, vec![1], vec![1, 2]),
        (2, vec![-1], vec![]),
        (2, vec![], vec![-1]),
    ] {
        assert!(validate(length, items.iter().copied(), removed.iter().copied()).is_err());
    }
}
