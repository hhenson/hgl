//! TS-7: notification belongs to a valid-to-invalid transition.
use hgl_store::{BindError, Store, Wake};
use hgl_types::{EngineTime, NodeId};
#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
fn at(t: i64) -> EngineTime {
    EngineTime::from_micros(t + 1)
}
#[test]
fn repeated_child_invalidation_preserves_parent_time_and_has_no_delta() -> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let dictionary = store.add_dictionary::<i64>(NodeId(0));
    let input = store.add_dictionary_input::<i64>(NodeId(1), true);
    store.bind(input.id(), dictionary.id())?;
    let child = store.get_or_create(dictionary, 42, at(0), &mut wakes);
    for cycle in 1..=6 {
        store.begin_cycle(at(cycle));
        wakes.0.clear();
        if cycle == 2 || cycle == 5 {
            store.set(child, 7, at(cycle), NodeId(0), &mut wakes);
        } else {
            store.invalidate(child.id(), at(cycle), &mut wakes);
        }
        let changed = cycle != 1 && cycle != 4;
        assert_eq!(!wakes.0.is_empty(), changed);
        assert_eq!(store.bindings().modified(input.id(), at(cycle)), changed);
        assert_eq!(
            store.bindings().changed_keys(input.id()),
            if changed { &[42][..] } else { &[] }
        );
        let expected_time = match cycle {
            1 => 0,
            4 => 3,
            _ => cycle,
        };
        assert_eq!(
            store.bindings().last_modified(input.id()),
            at(expected_time)
        );
    }
    Ok(())
}
#[test]
fn invalidating_a_never_valid_or_already_invalid_scalar_is_silent() -> Result<(), BindError> {
    let mut store = Store::new();
    let mut wakes = Wakes::default();
    let out = store.add_output::<i64>(NodeId(0));
    let input = store.add_input::<i64>(NodeId(1), true);
    store.bind(input.id(), out.id())?;
    store.invalidate(out.id(), at(0), &mut wakes);
    assert!(wakes.0.is_empty());
    store.set(out, 7, at(1), NodeId(0), &mut wakes);
    wakes.0.clear();
    store.invalidate(out.id(), at(2), &mut wakes);
    assert_eq!(wakes.0, vec![NodeId(1)]);
    assert!(!store.valid(input));
    wakes.0.clear();
    store.invalidate(out.id(), at(3), &mut wakes);
    assert!(wakes.0.is_empty());
    assert_eq!(store.last_modified(input), EngineTime::NEVER);
    Ok(())
}
