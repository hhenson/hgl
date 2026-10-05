//! Runtime primitive keys retain fixed slot identity across membership changes.
use hgl_member_table::Table;
#[test]
fn runtime_keys_keep_slots_through_collisions_removal_and_clear() {
    let mut table = Table::default();
    table.prepare(&[0, 1, 2, 3]);
    table.pool();
    let keys = [i64::MIN, i64::MAX, 0, 16];
    for (position, key) in keys.into_iter().enumerate() {
        assert_eq!(table.register(key), position);
        table.insert(key, position);
    }
    for (position, key) in keys.into_iter().enumerate() {
        assert_eq!(table.get(key), Some(&position));
    }
    assert_eq!(table.remove(i64::MIN), Some(0));
    assert_eq!(table.register(i64::MIN), 0);
    table.insert(i64::MIN, 10);
    table.clear();
    assert!(table.is_empty());
    for (position, key) in keys.into_iter().enumerate() {
        assert_eq!(table.register(key), position);
        table.insert(key, position);
    }
    assert_eq!(table.len(), 4);
}
