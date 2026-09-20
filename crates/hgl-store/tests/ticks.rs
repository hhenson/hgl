//! A tick: what `set` stamps, whom it wakes and what an input then reads
//! (specification: Time-series types, "A tick" and "Notification").

use hgl_store::{In, Out, Scalar, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType, ScalarValue};

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

/// A `TS<i64>` output of `PRODUCER` with one input of `CONSUMER` bound to it.
fn bound_pair(active: bool) -> (Store, Out<i64>, In<i64>) {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let input = store.add_input::<i64>(CONSUMER, active);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    (store, output, input)
}

// TS-1; card, "Done when": a tick stamps and wakes.
#[test]
fn ts1_a_tick_stamps_the_time_and_wakes_the_active_watcher() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();

    store.set(output, 7, at(10), PRODUCER, &mut woken);

    assert!(store.valid(input));
    assert!(store.modified(input, at(10)));
    assert_eq!(store.last_modified(input), at(10));
    assert_eq!(store.get(input), 7);
    assert_eq!(woken.0, [CONSUMER]);
}

// TS-1: the value persists; modified is true only in the cycle of the tick.
#[test]
fn ts1_in_a_later_cycle_the_input_is_valid_and_not_modified() {
    let (mut store, output, input) = bound_pair(true);
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());

    assert!(store.valid(input));
    assert!(!store.modified(input, at(11)));
    assert_eq!(store.last_modified(input), at(10));
    assert_eq!(store.get(input), 7);
}

// TS-1: modified exactly when the last modified time *equals* the evaluation
// time; neither side of it counts, for an input or on the erased path.
#[test]
fn ts1_modified_is_equality_with_the_evaluation_time_not_an_ordering() {
    let (mut store, output, input) = bound_pair(true);
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());

    let asked = [at(9), at(10), at(11)];

    assert_eq!(
        asked.map(|now| store.modified(input, now)),
        [false, true, false]
    );
    assert_eq!(
        asked.map(|now| store.output_modified(output.id(), now)),
        [false, true, false]
    );
}

// TS-6; card, "Done when": a second `set` in one cycle overwrites and wakes
// nobody again.
#[test]
fn ts6_a_second_set_in_one_cycle_overwrites_and_wakes_nobody_again() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();

    store.set(output, 1, at(10), PRODUCER, &mut woken);
    store.set(output, 2, at(10), PRODUCER, &mut woken);

    assert_eq!(store.get(input), 2);
    assert!(store.modified(input, at(10)));
    assert_eq!(woken.0, [CONSUMER]);
}

// TS-6: once in a cycle, and again in the next.
#[test]
fn ts6_a_tick_in_the_next_cycle_wakes_again() {
    let (mut store, output, _input) = bound_pair(true);
    let mut woken = Woken::default();

    store.set(output, 1, at(10), PRODUCER, &mut woken);
    store.set(output, 2, at(11), PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER, CONSUMER]);
}

// TS-8, GRF-17; card, "Done when": a passive watcher is not woken and still
// reads the value and `modified`.
#[test]
fn ts8_grf17_a_passive_watcher_is_not_woken_and_still_reads() {
    let (mut store, output, input) = bound_pair(false);
    let mut woken = Woken::default();

    store.set(output, 7, at(10), PRODUCER, &mut woken);

    assert!(woken.0.is_empty());
    assert!(store.valid(input));
    assert!(store.modified(input, at(10)));
    assert_eq!(store.last_modified(input), at(10));
    assert_eq!(store.get(input), 7);
}

// TS-8, NOD-7: making an input active or passive changes what wakes its node
// from then on, and nothing that it reads. That `set_active` itself wakes
// nobody is held by its signature, which has no `Wake`.
#[test]
fn ts8_nod7_set_active_changes_who_is_woken_and_no_reading() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();
    store.set(output, 7, at(10), PRODUCER, &mut woken);
    let readings = |store: &Store| {
        (
            store.valid(input),
            store.modified(input, at(10)),
            store.last_modified(input),
            store.get(input),
        )
    };
    let before = readings(&store);

    store.set_active(input, false);
    assert_eq!(readings(&store), before);
    store.set(output, 8, at(11), PRODUCER, &mut woken);
    assert_eq!(woken.0, [CONSUMER], "passive: the second tick woke nobody");

    store.set_active(input, true);
    assert_eq!(store.get(input), 8);
    assert!(store.modified(input, at(11)));
    store.set(output, 9, at(12), PRODUCER, &mut woken);
    assert_eq!(woken.0, [CONSUMER, CONSUMER]);
}

// TS-6: an output notifies on its first write in a cycle only, so a second
// write wakes nobody, not even a watcher made active in between.
#[test]
fn ts6_a_watcher_made_active_after_the_notification_is_not_woken_in_that_cycle() {
    let (mut store, output, input) = bound_pair(false);
    let mut woken = Woken::default();

    store.set(output, 1, at(10), PRODUCER, &mut woken);
    store.set_active(input, true);
    store.set(output, 2, at(10), PRODUCER, &mut woken);

    assert!(woken.0.is_empty());
    store.set(output, 3, at(11), PRODUCER, &mut woken);
    assert_eq!(woken.0, [CONSUMER]);
}

// TS-6: the same guard. A watcher bound between two writes of one cycle is
// not woken by the second. It reads the new value, and wakes from the next
// cycle on.
#[test]
fn ts6_a_watcher_bound_between_two_writes_of_one_cycle_is_not_woken_by_the_second() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let early = store.add_input::<i64>(NodeId(1), true);
    let late = store.add_input::<i64>(NodeId(2), true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(early.id(), output.id()), Ok(()));
    store.set(output, 1, at(10), PRODUCER, &mut woken);

    assert_eq!(store.bind(late.id(), output.id()), Ok(()));
    store.set(output, 2, at(10), PRODUCER, &mut woken);

    assert_eq!(woken.0, [NodeId(1)]);
    assert_eq!(store.get(late), 2);
    store.set(output, 3, at(11), PRODUCER, &mut woken);
    assert_eq!(woken.0, [NodeId(1), NodeId(1), NodeId(2)]);
}

// TS-6; specification, "Time": the earliest evaluation time is one step after
// `NEVER`, and a first write then is a tick like any other.
#[test]
fn ts6_the_first_write_at_the_earliest_start_time_wakes() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();

    store.set(output, 7, EngineTime::MIN_START, PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER]);
    assert!(store.modified(input, EngineTime::MIN_START));
    assert_eq!(store.get(input), 7);
}

// GRF-16; card, `Wake`: the store wakes once for each active input. A node
// with two inputs that both tick is woken twice; that it is evaluated once is
// the schedule's to keep, which is why `Wake` must be idempotent.
#[test]
fn grf16_a_node_with_two_ticking_inputs_is_woken_once_for_each() {
    let mut store = Store::new();
    let left = store.add_output::<i64>(PRODUCER);
    let right = store.add_output::<i64>(PRODUCER);
    let lhs = store.add_input::<i64>(CONSUMER, true);
    let rhs = store.add_input::<i64>(CONSUMER, true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(lhs.id(), left.id()), Ok(()));
    assert_eq!(store.bind(rhs.id(), right.id()), Ok(()));

    store.set(left, 1, at(10), PRODUCER, &mut woken);
    store.set(right, 2, at(10), PRODUCER, &mut woken);

    assert_eq!(woken.0, [CONSUMER, CONSUMER]);
}

// TS-8: `set_active` reaches the input it is given and no other.
#[test]
fn ts8_set_active_reaches_only_its_own_input() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let first = store.add_input::<i64>(NodeId(1), true);
    let second = store.add_input::<i64>(NodeId(2), true);
    let mut woken = Woken::default();
    assert_eq!(store.bind(first.id(), output.id()), Ok(()));
    assert_eq!(store.bind(second.id(), output.id()), Ok(()));

    store.set_active(second, false);
    store.set(output, 1, at(10), PRODUCER, &mut woken);
    assert_eq!(woken.0, [NodeId(1)]);

    store.set_active(second, true);
    store.set_active(first, false);
    store.set(output, 2, at(11), PRODUCER, &mut woken);
    assert_eq!(woken.0, [NodeId(1), NodeId(2)]);
}

// Specification, "A tick": writing the value a time-series already holds is
// still a tick.
#[test]
fn writing_the_same_value_again_is_still_a_tick() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();

    store.set(output, 7, at(10), PRODUCER, &mut woken);
    store.set(output, 7, at(11), PRODUCER, &mut woken);

    assert!(store.modified(input, at(11)));
    assert_eq!(woken.0, [CONSUMER, CONSUMER]);
}

// TS-3: shows only that each tick stamps its own time. That the time never
// *decreases* is a debug assertion in `set`; `debug_asserts.rs` tests it.
#[test]
fn ts3_last_modified_time_moves_forward_with_each_tick() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();
    assert_eq!(store.last_modified(input), EngineTime::NEVER);

    store.set(output, 1, at(10), PRODUCER, &mut woken);
    let first = store.last_modified(input);
    store.set(output, 2, at(25), PRODUCER, &mut woken);

    assert!(first > EngineTime::NEVER);
    assert!(store.last_modified(input) > first);
}

// TS-21: only `set` changes what an output shows; nothing done to its
// watchers does.
#[test]
fn ts21_only_set_changes_what_an_output_shows() {
    let (mut store, output, input) = bound_pair(true);
    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());
    let late = store.add_input::<i64>(NodeId(2), true);

    store.set_active(input, false);
    store.unbind(input.id());
    assert_eq!(store.bind(late.id(), output.id()), Ok(()));

    assert_eq!(store.output_value(output), Some(7));
    assert!(store.output_modified(output.id(), at(10)));
    assert!(!store.output_modified(output.id(), at(11)));
}

// TS-22: `get` returns a `T: Copy` by value, so its signature is the proof
// that what it gives is a copy, and no test of that can fail. This shows only
// the consequence: a later tick does not reach a value that was kept.
#[test]
fn ts22_a_value_kept_from_get_is_a_copy() {
    let (mut store, output, input) = bound_pair(true);
    let mut woken = Woken::default();
    store.set(output, 7, at(10), PRODUCER, &mut woken);

    let kept = store.get(input);
    store.set(output, 8, at(11), PRODUCER, &mut woken);

    assert_eq!(kept, 7);
    assert_eq!(store.get(input), 8);
}

// Card, `output_value` (INJ-8): `None` until the output has ticked.
#[test]
fn inj8_a_node_reads_its_own_output_only_once_it_has_ticked() {
    let (mut store, output, _input) = bound_pair(true);
    assert_eq!(store.output_value(output), None);
    assert_eq!(store.output_value_erased(output.id()), None);

    store.set(output, 7, at(10), PRODUCER, &mut Woken::default());

    assert_eq!(store.output_value(output), Some(7));
    assert_eq!(
        store.output_value_erased(output.id()),
        Some(ScalarValue::I64(7))
    );
}

// TS-8: a tick wakes the owner of each active watcher, in the order they
// were bound, and of no passive one; all of them read it.
#[test]
fn a_tick_wakes_the_owner_of_each_active_watcher() {
    let mut store = Store::new();
    let output = store.add_output::<i64>(PRODUCER);
    let first = store.add_input::<i64>(NodeId(1), true);
    let passive = store.add_input::<i64>(NodeId(2), false);
    let last = store.add_input::<i64>(NodeId(3), true);
    for input in [first, passive, last] {
        assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    }
    let mut woken = Woken::default();

    store.set(output, 7, at(10), PRODUCER, &mut woken);

    assert_eq!(woken.0, [NodeId(1), NodeId(3)]);
    for input in [first, passive, last] {
        assert!(store.modified(input, at(10)));
        assert_eq!(store.get(input), 7);
    }
}

/// An output of `T` with a bound input, ticked once with `value`.
fn ticked<T: Scalar>(store: &mut Store, value: T) -> (Out<T>, In<T>) {
    let output = store.add_output::<T>(PRODUCER);
    let input = store.add_input::<T>(CONSUMER, true);
    assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    store.set(output, value, at(10), PRODUCER, &mut Woken::default());
    (output, input)
}

// Card, "The layout": one column per scalar type, and a slot per output in
// the column of its type.
#[test]
fn each_scalar_type_has_its_own_column() {
    let mut store = Store::new();
    let (flag, flag_in) = ticked(&mut store, true);
    let (count, count_in) = ticked(&mut store, 42_i64);
    let (price, price_in) = ticked(&mut store, 2.5_f64);
    let (other, other_in) = ticked(&mut store, -1_i64);

    assert!(store.get(flag_in));
    assert_eq!(store.get(count_in), 42);
    assert_eq!(store.get(price_in).into_value(), ScalarValue::F64(2.5));
    assert_eq!(store.get(other_in), -1);

    // `count` and `other` share a column, so an output's id is not its slot.
    assert_eq!(store.output_value(flag), Some(true));
    assert_eq!(store.output_value(count), Some(42));
    assert_eq!(store.output_value(price), Some(2.5));
    assert_eq!(store.output_value(other), Some(-1));

    assert_eq!(
        store.output_value_erased(flag.id()),
        Some(ScalarValue::Bool(true))
    );
    assert_eq!(
        store.output_value_erased(count.id()),
        Some(ScalarValue::I64(42))
    );
    assert_eq!(
        store.output_value_erased(price.id()),
        Some(ScalarValue::F64(2.5))
    );
    assert_eq!(
        store.output_value_erased(other.id()),
        Some(ScalarValue::I64(-1))
    );
}

// Card, "Surface": the builder asks for types by id.
#[test]
fn the_builder_reads_types_by_id() {
    let mut store = Store::new();
    let flag = store.add_output::<bool>(PRODUCER);
    let count = store.add_input::<i64>(CONSUMER, true);
    let price = store.add_output::<f64>(PRODUCER);

    assert_eq!(store.output_type(flag.id()), ScalarType::Bool);
    assert_eq!(store.input_type(count.id()), ScalarType::I64);
    assert_eq!(store.output_type(price.id()), ScalarType::F64);
}

// Card, "The layout": ids are dense indices, in the order things were added.
#[test]
fn ids_are_dense_and_in_order_of_adding() {
    let mut store = Store::new();
    let outputs = [
        store.add_output::<f64>(PRODUCER).id(),
        store.add_output::<bool>(PRODUCER).id(),
    ];
    let inputs = [
        store.add_input::<f64>(CONSUMER, true).id(),
        store.add_input::<bool>(CONSUMER, false).id(),
    ];

    assert_eq!(outputs.map(|id| id.0), [0, 1]);
    assert_eq!(inputs.map(|id| id.0), [0, 1]);
}

/// Compiles only if both handles are `Copy` for any scalar type at all.
fn used_twice<T: Scalar>(output: Out<T>, input: In<T>) -> [(Out<T>, In<T>); 2] {
    [(output, input), (output, input)]
}

// Card, "Surface": `Out` is eight bytes, `In` is four, both are `Copy`.
#[test]
fn handles_are_small_and_copy() {
    let mut store = Store::new();
    let output = store.add_output::<f64>(PRODUCER);
    let input = store.add_input::<f64>(CONSUMER, true);

    let [first, second] = used_twice(output, input);

    assert_eq!(first.0.id(), second.0.id());
    assert_eq!(first.1.id(), second.1.id());
    assert_eq!(size_of::<Out<f64>>(), 8);
    assert_eq!(size_of::<In<bool>>(), 4);
}

// Card, `Scalar`: a value carries its type to the erased form and back, and
// does not come back as another type.
#[test]
fn a_scalar_goes_to_the_erased_form_and_back() {
    assert_eq!(bool::TYPE, ScalarType::Bool);
    assert_eq!(i64::TYPE, ScalarType::I64);
    assert_eq!(f64::TYPE, ScalarType::F64);

    assert_eq!(true.into_value(), ScalarValue::Bool(true));
    assert_eq!(42_i64.into_value(), ScalarValue::I64(42));
    assert_eq!(2.5_f64.into_value(), ScalarValue::F64(2.5));

    assert_eq!(bool::from_value(ScalarValue::Bool(true)), Some(true));
    assert_eq!(i64::from_value(ScalarValue::I64(42)), Some(42));
    assert_eq!(f64::from_value(ScalarValue::F64(2.5)), Some(2.5));

    assert_eq!(bool::from_value(ScalarValue::I64(1)), None);
    assert_eq!(i64::from_value(ScalarValue::F64(1.0)), None);
    assert_eq!(f64::from_value(ScalarValue::Bool(true)), None);
}
