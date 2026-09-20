//! The evaluation cycle in a graph wide enough that the schedule's set of
//! ready ranks is three levels deep and the ready nodes are far apart
//! (specification: Graph, "The evaluation cycle").

use std::cell::RefCell;
use std::rc::Rc;

use hgl_kernel::{Ctx, Graph, Node, NodeResult, NodeSlot};
use hgl_store::{Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

const NODES: u32 = 5_000;

/// The rank of every node evaluated, in the order it happened. Shared, which
/// the tick path bans and a test of ordering across nodes needs.
type Log = Rc<RefCell<Vec<u32>>>;

/// Logs its rank when evaluated, and ticks its output if it has one. Asks to
/// be evaluated `first` after the start time, if it has one.
struct Probe {
    rank: u32,
    log: Log,
    first: Option<i64>,
    out: Option<Out<i64>>,
}

impl Node for Probe {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        match self.first {
            Some(first) => ctx.schedule_in(EngineDelta::from_micros(first)),
            None => Ok(()),
        }
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.log.borrow_mut().push(self.rank);
        if let Some(out) = self.out {
            ctx.set(out, 1);
        }
        Ok(())
    }
}

fn slot(node: Probe) -> NodeSlot {
    let node_type = NodeType {
        name: "probe",
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler: true,
        schedule_on_start: false,
    };
    NodeSlot {
        node: Box::new(node),
        node_type,
        label: "probe".to_owned(),
        required: Vec::new(),
    }
}

// GRF-11, GRF-15, GRF-16: rank order, each node once, where the ready nodes
// lie in different words and different branches of the ready set, and were
// woken in another order. The second cycle starts below where the first
// ended, with a node due in the word the first ended in.
#[test]
fn grf11_grf16_a_pass_is_in_rank_order_across_a_wide_graph() {
    let first_at = |rank: u32| match rank {
        0 => Some(0),
        10 | 4_995 => Some(1),
        _ => None,
    };
    let woken_by_source = [4_999, 64, 4_096, 1, 4_097, 63, 128, 65, 4_095, 127];
    let woken_by_both = 4_998;
    let mut store = Store::new();
    let source = store.add_output::<i64>(NodeId(0));
    let halfway = store.add_output::<i64>(NodeId(128));
    for rank in woken_by_source {
        let input = store.add_input::<i64>(NodeId(rank), true);
        assert_eq!(store.bind(input.id(), source.id()), Ok(()));
    }
    for output in [source, halfway] {
        let input = store.add_input::<i64>(NodeId(woken_by_both), true);
        assert_eq!(store.bind(input.id(), output.id()), Ok(()));
    }
    let log = Log::default();
    let slots = (0..NODES).map(|rank| {
        let out = match rank {
            0 => Some(source),
            128 => Some(halfway),
            _ => None,
        };
        slot(Probe {
            rank,
            log: Rc::clone(&log),
            first: first_at(rank),
            out,
        })
    });
    let mut graph = Graph::new("wide".to_owned(), slots.collect());
    let start = EngineTime::from_micros(1);
    assert_eq!(graph.start(&mut store, start), Ok(()));

    assert_eq!(graph.evaluate(&mut store, start), Ok(()));
    let first_cycle = [
        0, 1, 63, 64, 65, 127, 128, 4_095, 4_096, 4_097, 4_998, 4_999,
    ];
    assert_eq!(*log.borrow(), first_cycle);

    log.borrow_mut().clear();
    assert_eq!(graph.next_scheduled_time(), EngineTime::from_micros(2));
    assert_eq!(
        graph.evaluate(&mut store, EngineTime::from_micros(2)),
        Ok(())
    );
    assert_eq!(*log.borrow(), [10, 4_995]);
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER);
}
