//! ADR 0016 / VAL-17 ordinary struct retention and lexical prepared projections.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{Capacity, GlobalValue, Layouts, Store, ValueColumns as Columns, ValueSlot};
use hgl_types::{NodeError, OrdinaryType};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
type Result<T> = std::result::Result<T, Box<NodeError>>;

struct Inner;
impl GlobalValue for Inner {
    type Value = (i64, String);
    type Slots = (ValueSlot<i64>, ValueSlot<String>);
    const WIDTH: usize = 2;
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> Result<()> {
        <i64 as GlobalValue>::prepare(&value.0, capacity, layouts)?;
        <String as GlobalValue>::prepare(&value.1, capacity, layouts)?;
        Ok(())
    }
    fn install(columns: &mut Columns, value: Self::Value, layouts: &mut Layouts) -> Self::Slots {
        (
            ValueSlot::<i64>::install(columns, value.0, layouts),
            ValueSlot::<String>::install(columns, value.1, layouts),
        )
    }
    fn release(columns: &mut Columns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[0..1]);
        slots.1.flatten(&mut layout[1..2]);
    }
    fn schema() -> OrdinaryType {
        OrdinaryType::Struct(
            "test::Inner",
            vec![("amount", i64::schema()), ("text", String::schema())],
        )
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (ValueSlot::bind(layout), ValueSlot::bind(layout))
    }
    fn retain(value: &Self::Value) -> Result<Self::Value> {
        let amount = i64::retain(&value.0)?;
        let text = <String as GlobalValue>::retain(&value.1)?;
        if text == "inject retention failure" {
            return Err(NodeError::new("retention failed"));
        }
        Ok((amount, text))
    }
    fn read(columns: &Columns, slots: Self::Slots) -> Result<Self::Value> {
        Ok((slots.0.read(columns)?, slots.1.read(columns)?))
    }
    fn commit(
        columns: &mut Columns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        slots.0.commit(columns, value.0, layouts);
        slots.1.commit(columns, value.1, layouts);
    }
}
struct Outer;
impl GlobalValue for Outer {
    type Value = (String, InnerValue, bool);
    type Slots = (ValueSlot<String>, ValueSlot<Inner>, ValueSlot<bool>);
    const WIDTH: usize = 4;
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> Result<()> {
        <String as GlobalValue>::prepare(&value.0, capacity, layouts)?;
        <Inner as GlobalValue>::prepare(&value.1, capacity, layouts)?;
        <bool as GlobalValue>::prepare(&value.2, capacity, layouts)?;
        Ok(())
    }
    fn install(columns: &mut Columns, value: Self::Value, layouts: &mut Layouts) -> Self::Slots {
        (
            ValueSlot::<String>::install(columns, value.0, layouts),
            ValueSlot::<Inner>::install(columns, value.1, layouts),
            ValueSlot::<bool>::install(columns, value.2, layouts),
        )
    }
    fn release(columns: &mut Columns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
        slots.2.release(columns);
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[0..1]);
        slots.1.flatten(&mut layout[1..3]);
        slots.2.flatten(&mut layout[3..4]);
    }
    fn schema() -> OrdinaryType {
        OrdinaryType::Struct(
            "test::Outer",
            vec![
                ("label", String::schema()),
                ("inner", Inner::schema()),
                ("flag", bool::schema()),
            ],
        )
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (
            ValueSlot::bind(layout),
            ValueSlot::bind(layout),
            ValueSlot::bind(layout),
        )
    }
    fn retain(value: &Self::Value) -> Result<Self::Value> {
        Ok((
            <String as GlobalValue>::retain(&value.0)?,
            Inner::retain(&value.1)?,
            bool::retain(&value.2)?,
        ))
    }
    fn read(columns: &Columns, slots: Self::Slots) -> Result<Self::Value> {
        Ok((
            slots.0.read(columns)?,
            slots.1.read(columns)?,
            slots.2.read(columns)?,
        ))
    }
    fn commit(
        columns: &mut Columns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        slots.0.commit(columns, value.0, layouts);
        slots.1.commit(columns, value.1, layouts);
        slots.2.commit(columns, value.2, layouts);
    }
}
type InnerValue = <Inner as GlobalValue>::Value;
fn original() -> <Outer as GlobalValue>::Value {
    ("label".into(), (7, "nested".into()), false)
}

#[test]
fn required_presence_nested_write_through_and_whole_replacement() -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<Outer>("entry")?;
    assert!(
        store
            .global_borrow(entry)
            .unwrap_err()
            .message
            .contains("missing value")
    );
    assert!(store.global_get(entry).is_err());
    store.global_set(entry, &original())?;
    let root = store.global_borrow(entry)?;
    let nested = root.fields().1;
    store.global_write(nested.fields().0, &9)?;
    assert_eq!(store.global_get(entry)?.1.0, 9);
    assert_eq!(store.global_read(root)?.0, "label");
    store.global_write(root, &("replacement".into(), (11, "new".into()), true))?;
    assert_eq!(store.global_read(nested)?.0, 11);
    assert_eq!(store.global_read(root)?.0, "replacement");
    let alias = store.bind_global::<Outer>("entry")?;
    assert_eq!(store.global_get(alias)?, store.global_get(entry)?);
    Ok(())
}

#[test]
fn retained_nested_values_are_independent_and_survive_run_disposal() -> Result<()> {
    let retained = {
        let mut store = Store::new();
        store.provision_global_state();
        let entry = store.bind_global::<Outer>("entry")?;
        let mut source = original();
        store.global_set(entry, &source)?;
        source.0.push('!');
        source.1.1.push('!');
        assert_eq!(store.global_get(entry)?, original());
        let retained = store.global_get(entry)?;
        let destination = store.bind_global::<Outer>("copy")?;
        store.global_set(destination, &store.global_get(entry)?)?;
        let root = store.global_borrow(entry)?;
        store.global_write(root.fields().1.fields().1, &"changed".into())?;
        assert_eq!(store.global_get(destination)?, original());
        retained
    };
    assert_eq!(retained, original());
    Ok(())
}

#[test]
fn failed_retention_preserves_all_fields_and_absence() -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<Outer>("entry")?;
    let failing = (
        "changed first field".into(),
        (99, "inject retention failure".into()),
        true,
    );
    assert!(store.global_set(entry, &failing).is_err());
    assert!(store.global_borrow(entry).is_err());
    store.global_set(entry, &original())?;
    assert!(store.global_set(entry, &failing).is_err());
    assert_eq!(store.global_get(entry)?, original());
    let root = store.global_borrow(entry)?;
    assert!(store.global_write(root, &failing).is_err());
    assert_eq!(store.global_get(entry)?, original());
    assert!(store.global_write(root.fields().1, &failing.1).is_err());
    assert_eq!(store.global_get(entry)?, original());
    Ok(())
}

#[test]
fn exact_nominal_identity_field_names_and_nested_types_bind_before_start() -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<Outer>("entry")?;
    store.global_set(entry, &original())?;
    let OrdinaryType::Struct(_, fields) = Outer::schema() else {
        unreachable!()
    };
    assert!(
        store
            .prepare_global("entry", OrdinaryType::Struct("Other", fields.clone()))
            .is_err()
    );
    let mut renamed = fields.clone();
    renamed[0].0 = "other_label";
    assert!(
        store
            .prepare_global("entry", OrdinaryType::Struct("test::Outer", renamed))
            .is_err()
    );
    let mut changed = fields;
    changed[1].1 = Inner::schema();
    if let OrdinaryType::Struct(_, fields) = &mut changed[1].1 {
        fields[0].1 = bool::schema();
    }
    assert!(
        store
            .prepare_global("entry", OrdinaryType::Struct("test::Outer", changed))
            .is_err()
    );
    assert!(store.bind_global::<i64>("entry").is_err());
    assert_eq!(store.global_get(entry)?, original());
    let mut another_run = Store::new();
    another_run.provision_global_state();
    let other = another_run.bind_global::<Inner>("entry")?;
    assert!(another_run.global_get(other).is_err());
    Ok(())
}

#[test]
fn borrowing_large_text_aggregate_and_mutating_primitive_leaf_allocates_nothing() -> Result<()> {
    let mut store = Store::new();
    store.provision_global_state();
    let entry = store.bind_global::<Outer>("entry")?;
    store.global_set(entry, &("a".repeat(8192), (0, "b".repeat(8192)), false))?;
    let (result, allocations) = count_in(|| -> Result<()> {
        for _ in 0..10_000 {
            let root = store.global_borrow(entry)?;
            let amount = root.fields().1.fields().0;
            store.global_write(amount, &(store.global_read(amount)? + 1))?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(store.global_get(entry)?.1.0, 10_000);
    Ok(())
}
