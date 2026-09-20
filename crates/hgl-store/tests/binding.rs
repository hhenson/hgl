//! Binding: what an input reads once bound or unbound, and what `bind`
//! refuses (specification: Time-series types, "Binding").

use hgl_store::{BindError, InputId, OutputId, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};

const PRODUCER: NodeId = NodeId(0);
const CONSUMER: NodeId = NodeId(1);

/// Every wake, in order, as the kernel's schedule would receive them.
#[derive(Default)]
struct Woken(Vec<NodeId>);

impl Wake for Woken {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}

fn at(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}

// TS-1; card, "Done when": an unbound input is not valid.
#[test]
fn ts1_an_unbound_input_is_not_valid() {
    let mut store = Store::new();
    let input = store.add_input::<i64>(CONSUMER, true);

    assert!(!store.valid(input));
    assert!(!store.modified(input, at(10)));
    assert_eq!(store.last_modified(input), EngineTime::NEVER);
}

// TS-1: valid is read from the last modified time, not from being bound.
#[test]
fn ts1_an_input_bound_to_an_output_that_never_ticked_is_not_valid() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));

    assert!(!store.valid(input));
    assert!(!store.modified(input, at(10)));
    assert_eq!(store.last_modified(input), EngineTime::NEVER);
}

// NOD-2, TS-1: what the kernel asks by id before `eval` is `valid` — bound,
// and to an output that has ticked — in every state an input passes through.
#[test]
fn nod2_ts1_input_valid_by_id_follows_the_binding_and_the_tick() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let by_id_and_typed = |store: &Store| (store.input_valid(input.id()), store.valid(input));

    assert_eq!(by_id_and_typed(&store), (false, false), "unbound");

    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    assert_eq!(
        by_id_and_typed(&store),
        (false, false),
        "bound, never ticked"
    );

    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());
    assert_eq!(by_id_and_typed(&store), (true, true), "bound and ticked");

    store.unbind(input.id());
    assert_eq!(by_id_and_typed(&store), (false, false), "unbound again");
}

// NOD-2: by id there is no type to know; an input of any scalar type answers.
#[test]
fn nod2_input_valid_needs_no_type() {
    let mut store = Store::new();
    let flag = store.add_output::<bool>(PRODUCER);
    let price = store.add_output::<f64>(PRODUCER);
    let inputs = [
        store.add_input::<bool>(CONSUMER, true).id(),
        store.add_input::<f64>(CONSUMER, false).id(),
    ];
    assert_eq!(store.bind(inputs[0], flag.id()), Ok(()));
    assert_eq!(store.bind(inputs[1], price.id()), Ok(()));

    store.set(price, 2.5, at(10), PRODUCER, &mut Woken::default());

    assert_eq!(inputs.map(|id| store.input_valid(id)), [false, true]);
}

// TS-14: that a plain bind notifies nothing is held by its signature: `bind`
// takes no `Wake`, so no test of it can fail. What is tested is the other
// half: the input shows the output as it is, and one that ticked two cycles
// ago reads valid and not modified.
#[test]
fn ts14_bind_shows_the_output_as_it_is() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());

    assert_eq!(store.bind(input.id(), output.id()), Ok(()));

    assert!(store.valid(input));
    assert!(!store.modified(input, at(12)));
    assert_eq!(store.last_modified(input), at(10));
    assert_eq!(store.get(input), 7);
}

// TS-14: bound to an output that ticked before, the input is a watcher like
// any other from the output's next tick on.
#[test]
fn ts14_the_first_tick_after_a_bind_wakes() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    store.set(output, 7, at(10), PRODUCER, &mut woken);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));

    store.set(output, 8, at(12), PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER]);
    assert!(store.modified(input, at(12)));
}

// TS-1, TS-14: `modified` is the output's, whenever the input was bound. One
// bound after the tick, in the same cycle, reads modified.
#[test]
fn ts1_an_input_bound_after_the_tick_reads_modified_in_that_cycle() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());

    assert_eq!(store.bind(input.id(), output.id()), Ok(()));

    assert!(store.modified(input, at(10)));
    assert_eq!(store.get(input), 7);
}

// TS-1: and one unbound after the tick, in the same cycle, no longer does.
#[test]
fn ts1_an_input_unbound_after_the_tick_reads_not_modified_in_that_cycle() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());
    assert!(store.modified(input, at(10)));

    store.unbind(input.id());

    assert!(!store.modified(input, at(10)));
}

// Specification, "Binding": after unbind the input reads not valid, and
// nothing is notified.
#[test]
fn unbind_leaves_the_input_not_valid_and_out_of_reach_of_ticks() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    store.set(output, 7, at(10), PRODUCER, &mut woken);

    store.unbind(input.id());
    store.set(output, 8, at(11), PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER], "only the tick before the unbind woke");
    assert!(!store.valid(input));
    assert!(!store.modified(input, at(11)));
    assert_eq!(store.last_modified(input), EngineTime::NEVER);
}

// Specification, "Binding": unbinding one input leaves the others bound.
#[test]
fn unbind_leaves_the_other_watchers_bound() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let leaving = store.add_input::<i64>(NodeId(1), true);
    let staying = store.add_input::<i64>(NodeId(2), true);
    assert_eq!(store.bind(leaving.id(), output.id()), Ok(()));
    assert_eq!(store.bind(staying.id(), output.id()), Ok(()));
    let mut woken = Woken::default();

    store.unbind(leaving.id());
    store.unbind(leaving.id());
    store.set(output, 7, at(10), PRODUCER, &mut woken);

    assert_eq!(woken.0, [NodeId(2)]);
    assert_eq!(store.get(staying), 7);
}

// Specification, "Binding": an unbound input may be bound again, and then
// reads its new output.
#[test]
fn an_unbound_input_binds_again_and_reads_its_new_output() {
    let mut store = Store::new();
    let old = store.add_output::<i64>(PRODUCER);
    let new = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    store.set(old, 1, at(10), PRODUCER, &mut woken);
    store.set(new, 2, at(10), PRODUCER, &mut woken);
    assert_eq!(store.bind(input.id(), old.id()), Ok(()));
    assert_eq!(store.get(input), 1);

    store.unbind(input.id());
    assert_eq!(store.bind(input.id(), new.id()), Ok(()));

    assert_eq!(store.get(input), 2);
    store.set(old, 3, at(11), PRODUCER, &mut woken);
    assert!(
        woken.0.is_empty(),
        "the old output no longer reaches the input"
    );
    assert_eq!(store.get(input), 2);
}

// TS-6, GRF-16: once per cycle is kept per output, so an input re-bound
// within a cycle is woken again by its new output's first write. TS-6's "an
// input schedules its node at most once" is therefore the schedule's to keep,
// with GRF-16 (card, `Wake`).
#[test]
fn ts6_grf16_an_input_rebound_in_a_cycle_is_woken_by_its_new_outputs_first_write() {
    let mut store = Store::new();
    let old = store.add_output::<i64>(PRODUCER);
    let new = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(input.id(), old.id()), Ok(()));
    store.set(old, 1, at(10), PRODUCER, &mut woken);

    store.unbind(input.id());
    assert_eq!(store.bind(input.id(), new.id()), Ok(()));
    store.set(new, 2, at(10), PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER, CONSUMER]);
    assert_eq!(store.get(input), 2);
}

// Card, "Done when": `bind` rejects a type mismatch.
#[test]
fn bind_rejects_a_type_mismatch() {
    let mut store = Store::new();
    let output = store.add_output::<f64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);

    let refused = store.bind(input.id(), output.id());

    let expected = BindError::TypeMismatch {
        input: ScalarType::I64,
        output: ScalarType::F64,
    };
    assert_eq!(refused, Err(expected));
}

// Card, `bind`: a refused bind binds nothing.
#[test]
fn a_refused_bind_leaves_the_input_unbound() {
    let mut store = Store::new();
    let output = store.add_output::<f64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert!(store.bind(input.id(), output.id()).is_err());

    store.set(output, 2.5, at(10), PRODUCER, &mut woken);

    assert!(woken.0.is_empty());
    assert!(!store.valid(input));
}

// Card, `bind`: a bind refused because the input is already bound leaves the
// first binding exactly as it was: nothing of the second output reaches it.
#[test]
fn a_bind_refused_as_already_bound_changes_nothing() {
    let mut store = Store::new();
    let first = store.add_output::<i64>(PRODUCER);
    let second = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(input.id(), first.id()), Ok(()));
    store.set(first, 1, at(10), PRODUCER, &mut woken);
    woken.0.clear();

    let refused = store.bind(input.id(), second.id());
    store.set(second, 2, at(11), PRODUCER, &mut woken);

    assert_eq!(refused, Err(BindError::AlreadyBound(input.id())));
    assert!(woken.0.is_empty());
    assert_eq!(store.get(input), 1);
    assert_eq!(store.last_modified(input), at(10));
}

// Card, `bind`: the same when a bound input is refused for its type. `spare`
// puts `first` at slot 1, so a `source_slot` moved to `other`'s slot 0 shows.
#[test]
fn a_bind_refused_as_a_type_mismatch_changes_nothing() {
    let mut store = Store::new();
    let spare = store.add_output::<i64>(PRODUCER);
    let first = store.add_output::<i64>(PRODUCER);
    let other = store.add_output::<f64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(input.id(), first.id()), Ok(()));
    store.set(spare, 99, at(10), PRODUCER, &mut woken);
    store.set(first, 1, at(10), PRODUCER, &mut woken);
    woken.0.clear();

    let refused = store.bind(input.id(), other.id());
    store.set(other, 2.5, at(11), PRODUCER, &mut woken);

    let expected = BindError::TypeMismatch {
        input: ScalarType::I64,
        output: ScalarType::F64,
    };
    assert_eq!(refused, Err(expected));
    assert!(woken.0.is_empty());
    assert_eq!(store.get(input), 1);
    assert_eq!(store.last_modified(input), at(10));
}

// Card, `bind`: either id unknown.
#[test]
fn bind_rejects_an_id_the_store_did_not_issue() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);

    assert_eq!(
        store.bind(InputId(9), output.id()),
        Err(BindError::UnknownInput(InputId(9)))
    );
    assert_eq!(
        store.bind(input.id(), OutputId(9)),
        Err(BindError::UnknownOutput(OutputId(9)))
    );
}

// Card, `bind`: the input already bound.
#[test]
fn bind_rejects_an_input_that_is_already_bound() {
    let mut store = Store::new();
    let first = store.add_output::<i64>(PRODUCER);
    let second = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, true);
    assert_eq!(store.bind(input.id(), first.id()), Ok(()));

    assert_eq!(
        store.bind(input.id(), second.id()),
        Err(BindError::AlreadyBound(input.id()))
    );
    assert_eq!(
        store.bind(input.id(), first.id()),
        Err(BindError::AlreadyBound(input.id()))
    );
}
