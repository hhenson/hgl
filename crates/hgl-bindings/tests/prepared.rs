//! Prepared projections retain passive admission and release unused subscriptions.
use hgl_bindings::{Bindings, Kind, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[derive(Default)]
struct Wakes(usize);
impl Wake for Wakes {
    fn wake(&mut self, _: NodeId) {
        self.0 += 1;
    }
}
#[test]
fn passive_future_members_and_unbinding_preserve_subscription_semantics() {
    let mut bindings = Bindings::default();
    let kind = Kind::Dictionary(Box::new(Kind::Ts(ScalarType::I64)));
    let root = bindings.add_output(NodeId(0), kind.clone(), 0).0;
    let first = bindings
        .add_output(NodeId(0), Kind::Ts(ScalarType::I64), 0)
        .0;
    let unused = bindings
        .add_output(NodeId(0), Kind::Ts(ScalarType::I64), 1)
        .0;
    bindings.prepare_collection(root, vec![(1, first), (2, unused)]);
    let input = bindings.add_input(NodeId(1), kind, true);
    bindings
        .bind(input, root)
        .unwrap_or_else(|_| unreachable!());
    bindings.prepare_collection_inputs();
    assert_eq!(bindings.storage_counts()[3], 3);
    bindings.set_active(input, false);
    let mut wakes = Wakes::default();
    let now = EngineTime::from_micros(1);
    bindings
        .insert(root, 1, first, now, &mut wakes)
        .unwrap_or_else(|_| unreachable!());
    bindings.publish(first, now, &mut wakes);
    assert_eq!(wakes.0, 0);
    let child = bindings
        .child_input(input, 1)
        .unwrap_or_else(|| unreachable!());
    assert!(bindings.valid(child));
    bindings.set_active(input, true);
    bindings.publish(first, EngineTime::from_micros(2), &mut wakes);
    assert_eq!(wakes.0, 1);
    bindings.unbind(input);
    assert_eq!(bindings.storage_counts()[3], 0);
    assert!(!bindings.valid(input));
}
