//! Removal and reinsertion preserve dense iteration without exposing prepared absence.
use hgl_store::member_table::Table;
#[test]
fn prepared_occupancy_survives_middle_removal_clear_and_reinsert() {
    let mut table = Table::default();
    table.prepare(&(0..10_000).collect::<Vec<_>>());
    assert_eq!(table.iter().count(), 0);
    for key in [3, 42, 9999] {
        assert_eq!(table.insert(key, key * 2), None);
    }
    assert_eq!(table.remove(42), Some(84));
    assert_eq!(table.insert(9999, 5), Some(19998));
    assert_eq!(table.len(), 2);
    assert_eq!(table.item(0), Some((3, &6)));
    assert_eq!(table.item(1), Some((9999, &5)));
    assert_eq!(table.item(2), None);
    assert_eq!(table.remove(9999), Some(5));
    table.insert(42, 7);
    assert_eq!(table.iter().count(), 2);
    table.clear();
    assert!(table.is_empty());
    table.insert(9999, 8);
    assert_eq!(table.pop_first(), Some((9999, 8)));
    assert_eq!(table.pop_first(), None);
}
