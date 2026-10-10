//! Byte retention, complete constructor preflight and independent prepared copies.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::shapes::{Atomic, Input, Output, Shape};
use hgl_store::{GlobalValue, Key, Keys, List, PreparedValue, Store, ValueColumns, Wake};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
fn at(n: i64) -> EngineTime {
    EngineTime::from_micros(n)
}

#[test]
fn bytes_retain_source_and_keys_by_content() -> NodeResult {
    let mut octets = vec![0, 128, 255];
    let bytes = hgl_types::bytes(&octets)?;
    octets.clear();
    assert_eq!(bytes, [0, 128, 255]);
    let mut keys = Keys::default();
    <Vec<u8> as Key>::prepare(&mut keys, &bytes)?;
    <Vec<u8> as Key>::prepare(&mut keys, &bytes.clone())?;
    <Vec<u8> as Key>::prepare(&mut keys, &Vec::new())?;
    assert_eq!(<Vec<u8> as Key>::ids(&keys).len(), 2);
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..100 {
            assert_eq!(<Vec<u8> as Key>::id(&keys, &bytes)?, 0);
            <Vec<u8> as Key>::with_value(&keys, 0, |retained| assert_eq!(retained, &bytes))?;
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(
        hgl_types::bytes(&[-1]).unwrap_err().code,
        Some("value.byte_range")
    );
    assert_eq!(
        hgl_types::bytes(&[256]).unwrap_err().code,
        Some("value.byte_range")
    );
    Ok(())
}

#[test]
fn prepared_bytes_copy_independently_and_failed_preflight_preserves_contents() -> NodeResult {
    let mut columns = ValueColumns::default();
    let source = Vec::<u8>::allocate(&mut columns, &3)?;
    let saved = Vec::<u8>::allocate(&mut columns, &3)?;
    let value = vec![0, 128, 255];
    let replacement = vec![255];
    let (result, allocations) = count_in(|| -> NodeResult {
        Vec::<u8>::check_native(&columns, source, &value)?;
        Vec::<u8>::copy_native(&mut columns, source, &value);
        Vec::<u8>::check_slots(&columns, source, &columns, saved)?;
        Vec::<u8>::copy_within(&mut columns, source, saved);
        Vec::<u8>::check_native(&columns, source, &replacement)?;
        Vec::<u8>::copy_native(&mut columns, source, &replacement);
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(saved.read(&columns)?, value);
    assert_eq!(source.read(&columns)?, replacement);
    assert!(Vec::<u8>::check_native(&columns, source, &vec![0; 32]).is_err());
    assert_eq!(source.read(&columns)?, replacement);
    assert_eq!(
        Vec::<u8>::schema(),
        hgl_types::OrdinaryType::Scalar(hgl_types::ScalarType::Bytes)
    );
    Ok(())
}

#[test]
fn constructor_failure_changes_neither_held_output_nor_publication_stamp() -> NodeResult {
    let mut store = Store::new();
    let list = store.add_atomic_output::<List<i64>>(NodeId(0));
    let list_output = Output::<Atomic<List<i64>>>::bind(store.bindings(), list)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<List<i64>>::shape(), true);
    let input = Input::<Atomic<List<i64>>>::bind(store.bindings(), input_id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    store
        .bind(input_id, list)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let output = store.add_output::<Vec<u8>>(NodeId(1));
    let observer = store.add_input::<Vec<u8>>(NodeId(2), true);
    store
        .bind(observer.id(), output.id())
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    store.prepared().prepare_scalar::<Vec<u8>>(output.id(), 2)?;
    let mut wakes = Wakes::default();
    store.set_atomic(list_output, vec![0, 255], at(1), &mut wakes)?;
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(at(1), NodeId(1), &mut wakes)
            .bytes_from_list(input, output)
    });
    result?;
    assert_eq!(allocations, 0);
    assert_eq!(store.get_ref(observer), &[0, 255]);
    let stamp = store.last_modified(observer);
    store.begin_cycle(at(2));
    store.set_atomic(list_output, vec![0, -1], at(2), &mut wakes)?;
    let error = store
        .prepared()
        .tick(at(2), NodeId(1), &mut wakes)
        .bytes_from_list(input, output)
        .unwrap_err();
    assert_eq!(error.code, Some("value.byte_range"));
    assert_eq!(store.get_ref(observer), &[0, 255]);
    assert_eq!(store.last_modified(observer), stamp);
    assert!(!store.modified(observer, at(2)));
    store.begin_cycle(at(3));
    let too_large = vec![0; store.get_ref(observer).capacity() + 1];
    store.set_atomic(list_output, too_large, at(3), &mut wakes)?;
    assert!(
        store
            .prepared()
            .tick(at(3), NodeId(1), &mut wakes)
            .bytes_from_list(input, output)
            .is_err()
    );
    assert_eq!(store.get_ref(observer), &[0, 255]);
    assert_eq!(store.last_modified(observer), stamp);
    Ok(())
}

#[test]
fn rolling_constructor_preflight_preserves_the_previous_arrival() -> NodeResult {
    type Window = hgl_store::Rolling<Vec<u8>, false, 1, 1>;
    let mut store = Store::new();
    let list = store.add_atomic_output::<List<i64>>(NodeId(0));
    let list_output = Output::<Atomic<List<i64>>>::bind(store.bindings(), list)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let input_id = store.add_shaped_input(NodeId(1), Atomic::<List<i64>>::shape(), true);
    store
        .bind(input_id, list)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let input = Input::<Atomic<List<i64>>>::bind(store.bindings(), input_id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let output_id = store.add_shaped_output(NodeId(1), Window::shape());
    let observer_id = store.add_shaped_input(NodeId(2), Window::shape(), true);
    store
        .bind(observer_id, output_id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let output = Output::<Window>::bind(store.bindings(), output_id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let observer = Input::<Window>::bind(store.bindings(), observer_id)
        .map_err(|e| NodeError::new(format!("{e:?}")))?;
    let storage = store.prepared();
    storage
        .rolling
        .prepare_output::<Window>(storage.bindings, output_id, &2, 3)?;
    let mut wakes = Wakes::default();
    store.set_atomic(list_output, vec![0, 255], at(1), &mut wakes)?;
    let (result, allocations) = count_in(|| {
        store
            .prepared()
            .tick(at(1), NodeId(1), &mut wakes)
            .rolling_bytes_from_list(input, output)
    });
    result?;
    assert_eq!(allocations, 0);
    for (time, values, code) in [
        (2, vec![0, -1], Some("value.byte_range")),
        (3, vec![0; 1000], None),
    ] {
        store.begin_cycle(at(time));
        store.set_atomic(list_output, values, at(time), &mut wakes)?;
        let error = store
            .prepared()
            .tick(at(time), NodeId(1), &mut wakes)
            .rolling_bytes_from_list(input, output)
            .unwrap_err();
        assert_eq!(error.code, code);
        assert_eq!(store.bindings().output(output_id).modified_at, at(1));
        assert!(store.rolling.ready(store.bindings(), observer));
        assert_eq!(
            store
                .rolling
                .borrow(store.bindings(), observer)?
                .read(store.rolling.values())?,
            [0, 255]
        );
    }
    Ok(())
}

#[test]
fn ordinary_constructor_owns_storage_and_requires_the_generic_adapter() -> NodeResult {
    let source = [1, 2];
    let (result, allocations) = count_in(|| hgl_types::bytes(&source));
    assert_eq!(result?, [1, 2]);
    assert!(
        allocations > 0,
        "owning fallback must not claim prepared transport"
    );
    Ok(())
}
