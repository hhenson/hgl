//! The two nodes the harness is made of.
//!
//! They are ordinary nodes, registered and instantiated like any other, so a
//! case is driven through the runtime's own front door and nothing about the
//! harness is privileged.
//!
//! A description is plain data (GRF-1), so neither a sequence to replay nor
//! the values a run produced can travel in one. Both are handed over through
//! the node itself: [`Replay::load`] after instantiation and before the run,
//! [`Record::seen`] after it.

use hgl_describe::{BuildError, Buildable, Ports};
use hgl_kernel::{Ctx, Node, NodeError, NodeResult};
use hgl_store::{In, Out};
use hgl_types::{EngineDelta, EngineTime, NodeType, ScalarType, ScalarValue, TsType};

const I64: TsType = TsType::Ts(ScalarType::I64);

/// The name [`Replay`] is registered under.
pub(crate) const REPLAY: &str = "testkit.replay";

/// The name [`Record`] is registered under.
pub(crate) const RECORD: &str = "testkit.record";

/// Which of the case's inputs a replay stands for.
pub(crate) const SLOT: &str = "slot";

/// How many cycles a record may have to note.
pub(crate) const CYCLES: &str = "cycles";

/// Cycle `i` runs at the start time plus `i` steps, which is the alignment
/// hgraph's `eval_node` uses and the case tables are written in.
fn cycle_at(start: EngineTime, now: EngineTime) -> usize {
    let steps = (now.micros() - start.micros()) / EngineDelta::STEP.micros();
    usize::try_from(steps).unwrap_or(0)
}

/// `testkit.replay`: a pull source that emits a fixed sequence, one element
/// per cycle from the start time, emitting nothing where the sequence has no
/// tick, and rescheduling itself while elements remain.
#[derive(Debug)]
pub struct Replay {
    out: Out<i64>,
    ticks: Vec<Option<ScalarValue>>,
    next: usize,
}

impl Replay {
    /// The sequence to emit, one element per cycle; `None` is *no tick*.
    ///
    /// Reached with `Graph::node_mut` after `instantiate` and before the run.
    /// The vector keeps its capacity, so a run allocates here and not in a
    /// cycle.
    pub fn load(&mut self, ticks: &[Option<ScalarValue>]) {
        self.ticks.clear();
        self.ticks.extend_from_slice(ticks);
        self.next = 0;
    }
}

impl Node for Replay {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let tick = self.ticks.get(self.next).cloned().flatten();
        self.next += 1;
        match tick {
            None => {}
            Some(ScalarValue::I64(value)) => ctx.set(self.out, value),
            Some(
                ScalarValue::Bool(_)
                | ScalarValue::F64(_)
                | ScalarValue::Text(_)
                | ScalarValue::Date(_)
                | ScalarValue::Time(_)
                | ScalarValue::DateTime(_)
                | ScalarValue::Duration(_),
            ) => {
                return Err(NodeError::new("testkit.replay emits TS[int] only"));
            }
        }
        if self.next < self.ticks.len() {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}

impl Buildable for Replay {
    fn node_type() -> NodeType {
        NodeType {
            name: REPLAY,
            output: Some(I64),
            scalars: vec![(SLOT, ScalarType::I64)],
            uses_scheduler: true,
            schedule_on_start: true,
            ..NodeType::default()
        }
    }

    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            ticks: Vec::new(),
            next: 0,
        })
    }
}

/// `testkit.record`: a sink that notes, per cycle, what its input showed.
#[derive(Debug)]
pub struct Record {
    input: In<i64>,
    seen: Vec<Option<ScalarValue>>,
    /// How many cycles this record may note. A tick after them is a failure of
    /// the case: without the bound, `seen` is sized from engine time, which
    /// runs to 10^16 steps.
    span: usize,
    start: EngineTime,
}

impl Record {
    /// One element per cycle from the start time; `None` is a cycle the input
    /// did not tick in. Cycles after the last tick are left off the end.
    ///
    /// Reached with `Graph::node` after the run.
    pub fn seen(&self) -> &[Option<ScalarValue>] {
        &self.seen
    }
}

impl Node for Record {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.start = ctx.evaluation_time();
        Ok(())
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let cycle = cycle_at(self.start, ctx.evaluation_time());
        debug_assert!(
            cycle >= self.seen.len(),
            "ENG-2: each cycle is later than the one before"
        );
        if cycle >= self.span {
            return Err(NodeError::new(
                "testkit.record: a tick after the last cycle",
            ));
        }
        // A cycle this node was not evaluated in is a cycle its input did not
        // tick in: the value it kept showing is not a tick (TS-1).
        self.seen.resize(cycle, None);
        self.seen.push(Some(ScalarValue::I64(ctx.get(self.input))));
        Ok(())
    }
}

impl Buildable for Record {
    fn node_type() -> NodeType {
        NodeType {
            name: RECORD,
            inputs: vec![("in", I64)],
            scalars: vec![(CYCLES, ScalarType::I64)],
            ..NodeType::default()
        }
    }

    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        let cycles: i64 = ports.scalar(CYCLES)?;
        let span = usize::try_from(cycles).unwrap_or(0);
        Ok(Self {
            input: ports.input("in")?,
            seen: Vec::with_capacity(span),
            span,
            start: EngineTime::NEVER,
        })
    }
}
