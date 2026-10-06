//! Immutable nominal batch identity, canonical ordering and explicit missing edges.
use hgl_source::nominal_batch::{Batch, Definition};
#[test]
fn definitions_are_canonical_but_do_not_change_identity() -> Result<(), String> {
    let full = Batch::new(
        "Left",
        vec![
            Definition::new("Right", vec![("n".into(), 2)], vec![]),
            Definition::new("Left", vec![("n".into(), 1)], vec![]),
        ],
    )?;
    let edge = Batch::<_, i64>::reference("Left");
    assert_eq!(full, edge);
    assert_eq!(full.cmp(&edge), std::cmp::Ordering::Equal);
    assert_eq!(full.definitions()[0].identity(), &"Left");
    assert!(edge.definition(&"Left").is_err());
    assert!(full.definition(&"Missing").is_err());
    assert_eq!(full.definition(&"Right")?.fields()[0].1, 2);
    Ok(())
}
#[test]
fn duplicate_exact_definitions_and_missing_roots_are_rejected() {
    assert!(
        Batch::<_, i64>::new(
            "Left",
            vec![
                Definition::new("Left", vec![], vec![]),
                Definition::new("Left", vec![], vec![])
            ]
        )
        .is_err()
    );
    assert!(Batch::<_, i64>::new("Left", vec![Definition::new("Right", vec![], vec![])]).is_err());
}
