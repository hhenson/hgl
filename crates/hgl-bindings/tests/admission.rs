//! TS-8 applies to existing and future collection members.
use hgl_bindings::{Bindings, Kind, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
#[derive(Default)]
struct Wakes(Vec<NodeId>);
impl Wake for Wakes {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}
#[test]
fn changing_collection_activity_reaches_existing_and_future_children()
-> Result<(), hgl_bindings::BindError> {
    let mut bindings = Bindings::default();
    let mut wakes = Wakes::default();
    let scalar = Kind::Scalar(ScalarType::I64);
    let dictionary = Kind::Dictionary(ScalarType::I64);
    let (output, _) = bindings.add_output(NodeId(0), dictionary, 0);
    let input = bindings.add_input(NodeId(1), dictionary, true);
    bindings.bind(input, output)?;
    for cycle in 1..=4 {
        let now = EngineTime::from_micros(cycle * 2);
        bindings.begin_cycle(now);
        let child = if cycle <= 2 {
            let (child, _) = bindings.add_output(NodeId(0), scalar, 0);
            bindings.insert(output, cycle, child, now, &mut wakes)?;
            child
        } else {
            bindings
                .child_output(output, 1)
                .ok_or(hgl_bindings::BindError::ShapeMismatch)?
        };
        bindings.set_active(input, cycle == 4);
        wakes.0.clear();
        // Use the next cycle so prior membership notification cannot mask activity.
        let tick = EngineTime::from_micros(cycle * 2 + 1);
        bindings.begin_cycle(tick);
        bindings.publish(child, tick, &mut wakes);
        assert_eq!(wakes.0.is_empty(), cycle != 4);
        assert_eq!(bindings.last_modified(input), tick);
    }
    Ok(())
}

#[test]
fn releasing_notified_children_cancels_their_parent_mailbox_entries()
-> Result<(), hgl_bindings::BindError> {
    let mut bindings = Bindings::default();
    let mut wakes = Wakes::default();
    let scalar = Kind::Scalar(ScalarType::I64);
    let (output, _) = bindings.add_output(NodeId(0), scalar, 0);
    let root = bindings.scope();
    let mut children = Vec::new();
    for _ in 0..3 {
        let scope = bindings.child_scope(NodeId(1));
        bindings.enter_scope(scope);
        let input = bindings.add_input(NodeId(0), scalar, true);
        bindings.bind(input, output)?;
        bindings.reserve_scope(scope, 1);
        bindings.enter_scope(root);
        children.push(scope);
    }
    let now = EngineTime::MIN_START;
    bindings.publish(output, now, &mut wakes);
    for scope in children {
        bindings.release_scope(scope, now, &mut wakes);
    }
    // Queued, dead children must not count against the next child's mailbox.
    let fresh = bindings.child_scope(NodeId(1));
    assert_eq!(bindings.take_child(NodeId(1)), None);
    bindings.release_scope(fresh, now, &mut wakes);
    Ok(())
}
