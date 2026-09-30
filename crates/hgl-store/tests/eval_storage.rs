//! Runtime storage used by the generated standard-library nodes.
use hgl_store::{BindError, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType, TsType};
#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
#[test]
fn string_read_borrows_and_equal_publications_still_tick() -> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let output = store.add_output::<String>(NodeId(0));
    let input = store.add_input::<String>(NodeId(1), true);
    store.bind(input.id(), output.id())?;
    for cycle in 1..=2 {
        let value = "payload".to_owned();
        let address = value.as_ptr();
        let now = EngineTime::from_micros(cycle);
        store.set(output, value, now, NodeId(0), &mut wakes);
        assert_eq!(store.get_ref(input).as_ptr(), address);
        assert_eq!(store.get_ref(input), "payload");
        assert!(store.modified(input, now));
    }
    assert_eq!(wakes.0, vec![NodeId(1), NodeId(1)]);
    Ok(())
}
#[test]
fn capturing_a_reference_does_not_subscribe_to_values_or_revive_retired_members()
-> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dict = store.add_dictionary::<i64>(NodeId(0));
    let first = store.get_or_create(dict, 7, EngineTime::from_micros(1), &mut wakes);
    let capture = store.add_shaped_input(
        NodeId(1),
        TsType::Reference(Box::new(TsType::Ts(ScalarType::I64))),
        true,
    );
    store.bind_designation(capture, first.id())?;
    assert!(
        store.input_valid(capture),
        "identity is valid before a value exists"
    );
    let saved = store.bindings().input_reference(capture);
    store.set(first, 42, EngineTime::from_micros(1), NodeId(0), &mut wakes);
    assert!(wakes.0.is_empty());
    assert_eq!(store.bindings().input_reference(capture), saved);
    store.remove(dict, 7, EngineTime::from_micros(2), &mut wakes);
    assert_eq!(store.bindings().input_reference(capture), saved);
    store.begin_cycle(EngineTime::from_micros(3));
    assert!(!store.input_valid(capture));
    assert_eq!(
        store.bindings().input_reference(capture),
        Reference::default()
    );
    let replacement = store.get_or_create(dict, 7, EngineTime::from_micros(4), &mut wakes);
    assert_eq!(replacement.id(), first.id(), "exercise generation reuse");
    assert!(!store.input_valid(capture));
    assert_eq!(
        store.bindings().input_reference(capture),
        Reference::default()
    );
    Ok(())
}
