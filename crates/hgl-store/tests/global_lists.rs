//! LIST-READ/GROW/RETAIN/ERROR: typed global lists and recursive ownership.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{
    Capacity, GlobalValue, Layouts, List, Store, ValueColumns, ValueSlot, list_index,
    list_index_mut, list_len, list_push,
};
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, NodeResult, OrdinaryType, Time};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

struct Envelope<T: GlobalValue>(std::marker::PhantomData<T>);
impl<T: GlobalValue> GlobalValue for Envelope<T> {
    type Value = (String, T::Value);
    type Slots = (ValueSlot<String>, ValueSlot<T>);
    const WIDTH: usize = 1 + T::WIDTH;
    fn schema() -> OrdinaryType {
        OrdinaryType::Struct(
            "test::Envelope",
            vec![("label", String::schema()), ("value", T::schema())],
        )
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (ValueSlot::bind(layout), ValueSlot::bind(layout))
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        Ok((
            <String as GlobalValue>::retain(&value.0)?,
            T::retain(&value.1)?,
        ))
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        <String as GlobalValue>::prepare(&value.0, capacity, layouts)?;
        T::prepare(&value.1, capacity, layouts)
    }
    fn read(columns: &ValueColumns, slots: Self::Slots) -> NodeResult<Self::Value> {
        Ok((slots.0.read(columns)?, slots.1.read(columns)?))
    }
    fn commit(
        columns: &mut ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        slots.0.commit(columns, value.0, layouts);
        slots.1.commit(columns, value.1, layouts);
    }
    fn install(
        columns: &mut ValueColumns,
        value: Self::Value,
        layouts: &mut Layouts,
    ) -> Self::Slots {
        (
            ValueSlot::install(columns, value.0, layouts),
            ValueSlot::install(columns, value.1, layouts),
        )
    }
    fn release(columns: &mut ValueColumns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[..1]);
        slots.1.flatten(&mut layout[1..]);
    }
}
struct Fallible;
impl GlobalValue for Fallible {
    type Value = i64;
    type Slots = usize;
    const WIDTH: usize = 1;
    fn schema() -> OrdinaryType {
        OrdinaryType::Struct("test::Fallible", vec![("value", i64::schema())])
    }
    fn slots(layout: &mut &[usize]) -> usize {
        i64::slots(layout)
    }
    fn retain(value: &i64) -> NodeResult<i64> {
        if *value == -1 {
            Err(NodeError::new("injected retention failure"))
        } else {
            Ok(*value)
        }
    }
    fn prepare(value: &i64, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        if *value == -2 {
            capacity.lists(usize::MAX);
        }
        i64::prepare(value, capacity, layouts)
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<i64> {
        i64::read(columns, slot)
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: i64, layouts: &mut Layouts) {
        i64::commit(columns, slot, value, layouts);
    }
    fn install(columns: &mut ValueColumns, value: i64, layouts: &mut Layouts) -> usize {
        i64::install(columns, value, layouts)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        i64::release(columns, slot);
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        i64::flatten(slot, layout);
    }
}
fn provisioned() -> Store {
    let mut store = Store::new();
    store.provision_global_state();
    store
}

#[test]
fn list_empty_bounds_owned_growth_and_self_source() -> NodeResult {
    let mut values = Vec::new();
    assert_eq!(list_len(&values)?, 0);
    assert!(list_index(&values, 0).is_err());
    list_push::<i64>(&mut values, &10)?;
    list_push::<i64>(&mut values, &20)?;
    let first = *list_index(&values, 0)?;
    list_push::<i64>(&mut values, &first)?;
    assert_eq!(values, [10, 20, 10]);
    assert!(list_index(&values, -1).is_err());
    assert!(list_index(&values, 3).is_err());
    let mut children = vec![vec![1]];
    let retained = <List<i64> as GlobalValue>::retain(list_index(&children, 0)?)?;
    list_push::<List<i64>>(&mut children, &retained)?;
    list_push::<i64>(list_index_mut(&mut children, 0)?, &2)?;
    assert_eq!(children, [vec![1, 2], vec![1]]);
    Ok(())
}

#[test]
fn fixed_identity_seed_validation_and_absence_are_recursive() -> NodeResult {
    let mut store = provisioned();
    let fixed = store.bind_global::<List<i64, 2>>("fixed")?;
    assert!(store.global_state().borrow(fixed).is_err());
    assert!(store.global_set(fixed, &vec![]).is_err());
    assert!(store.global_state().borrow(fixed).is_err());
    store.global_set(fixed, &vec![1, 2])?;
    assert!(store.global_set(fixed, &vec![9]).is_err());
    assert_eq!(store.global_get(fixed)?, [1, 2]);
    assert!(store.bind_global::<List<i64>>("fixed").is_err());
    assert!(store.bind_global::<List<i64, 0>>("fixed").is_err());
    let nested = store.bind_global::<Envelope<List<List<i64, 2>>>>("nested")?;
    let original = ("stable".into(), vec![vec![1, 2]]);
    store.global_set(nested, &original)?;
    assert!(
        store
            .global_set(nested, &("changed".into(), vec![vec![1, 2], vec![3]]))
            .is_err()
    );
    assert_eq!(store.global_get(nested)?, original);
    let zero = store.bind_global::<List<i64, 0>>("zero")?;
    store.global_set(zero, &vec![])?;
    let slot = store.global_state().borrow(zero)?;
    assert_eq!(store.global_state().list_len(slot)?, 0);
    assert!(store.global_state().list_index(slot, 0).is_err());
    Ok(())
}

#[test]
fn nested_lists_and_struct_fields_are_live_without_aliasing_retained_payloads() -> NodeResult {
    type Root = Envelope<List<Envelope<List<i64>>>>;
    let mut store = provisioned();
    let entry = store.bind_global::<Root>("root")?;
    let mut original = ("root".into(), vec![("child".into(), vec![1])]);
    store.global_set(entry, &original)?;
    original.1[0].1.push(99);
    let root = store.global_state().borrow(entry)?;
    let items = root.fields().1;
    let first = store.global_state().list_index(items, 0)?;
    let retained = store.global_state().read(first)?;
    store.global_state().list_push(items, &retained)?;
    store.global_state().list_push(first.fields().1, &2)?;
    assert_eq!(
        store.global_state().read(first)?,
        ("child".into(), vec![1, 2])
    );
    let second = store.global_state().list_index(items, 1)?;
    assert_eq!(
        store.global_state().read(second)?,
        ("child".into(), vec![1])
    );
    let snapshot = store.global_get(entry)?;
    store.global_state().write(root, &("new".into(), vec![]))?;
    assert_eq!(store.global_state().list_len(root.fields().1)?, 0);
    drop(store);
    assert_eq!(
        snapshot.1,
        [("child".into(), vec![1, 2]), ("child".into(), vec![1])]
    );
    Ok(())
}

#[test]
fn failed_retention_and_capacity_reservation_preserve_values_and_slot_counts() -> NodeResult {
    let mut store = provisioned();
    let entry = store.bind_global::<Envelope<List<Envelope<List<Fallible>>>>>("root")?;
    let original = ("outer".into(), vec![("child".into(), vec![1])]);
    store.global_set(entry, &original)?;
    let root = store.global_state().borrow(entry)?;
    let items = root.fields().1;
    let counts = store.global_state().slot_counts();
    for failure in [-1, -2] {
        let failed_item = ("different".into(), vec![2, failure]);
        for _ in 0..32 {
            assert!(store.global_state().list_push(items, &failed_item).is_err());
            assert!(
                store
                    .global_state()
                    .write(root, &("changed".into(), vec![failed_item.clone()]))
                    .is_err()
            );
            assert_eq!(store.global_get(entry)?, original);
            assert_eq!(store.global_state().slot_counts(), counts);
        }
    }
    Ok(())
}

#[test]
fn repeated_whole_replacement_and_push_reuse_all_recursive_slots() -> NodeResult {
    let mut store = provisioned();
    let entry = store.bind_global::<List<Envelope<List<String>>>>("root")?;
    let value = vec![("label".into(), vec!["one".into(), "two".into()])];
    store.global_set(entry, &value)?;
    let root = store.global_state().borrow(entry)?;
    for _ in 0..4 {
        store.global_state().write(root, &value)?;
        store.global_state().list_push(root, &value[0])?;
    }
    let counts = store.global_state().slot_counts();
    for _ in 0..1000 {
        store.global_state().write(root, &value)?;
        store.global_state().list_push(root, &value[0])?;
    }
    assert_eq!(store.global_state().slot_counts(), counts);
    assert_eq!(
        store.global_get(entry)?,
        [value[0].clone(), value[0].clone()]
    );
    Ok(())
}

#[test]
fn list_borrow_length_and_nested_projection_do_not_copy_or_allocate() -> NodeResult {
    let mut store = provisioned();
    let entry = store.bind_global::<List<Envelope<List<String>>>>("root")?;
    store.global_set(
        entry,
        &vec![("large".repeat(4096), vec!["payload".repeat(4096)])],
    )?;
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..10_000 {
            let root = store.global_state().borrow(entry)?;
            assert_eq!(store.global_state().list_len(root)?, 1);
            let first = store.global_state().list_index(root, 0)?;
            assert_eq!(store.global_state().list_len(first.fields().1)?, 1);
            let _text = store.global_state().list_index(first.fields().1, 0)?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}

fn scalar_list<T: hgl_store::Scalar>(item: &T) -> NodeResult {
    let mut store = provisioned();
    let entry = store.bind_global::<List<T>>("value")?;
    store.global_set(entry, &vec![])?;
    let slot = store.global_state().borrow(entry)?;
    store.global_state().list_push(slot, item)?;
    let element = store.global_state().list_index(slot, 0)?;
    assert_eq!(&store.global_state().read(element)?, item);
    Ok(())
}
#[test]
fn all_eight_primitives_use_their_typed_columns_in_lists() -> NodeResult {
    scalar_list(&true)?;
    scalar_list(&4_i64)?;
    scalar_list(&2.5)?;
    scalar_list(&"text".to_owned())?;
    scalar_list(&Date(1))?;
    scalar_list(&Time(2))?;
    scalar_list(&EngineTime::from_micros(3))?;
    scalar_list(&EngineDelta::STEP)
}
