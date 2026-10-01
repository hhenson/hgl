//! What runs a graph: the node interface, the schedule, the evaluation cycle,
//! node lifecycle and admission, and the simulation engine.
//!
//! A node author writes a struct of handles and implements [`Node`] for it.
//! [`Ctx`] is the only thing a hook can reach. A [`Graph`] is nodes in rank
//! order with a schedule; its owner asks it four things: start, evaluate at a
//! time, its next scheduled time, and stop (specification: Graph, Part 2).
//! [`run_simulation`] is the owner of a root graph.
//!
//! A cycle costs what is ready, never what exists
//! (`docs/explorations/0009-designing-for-speed.md`): nothing on the path from
//! [`Graph::evaluate`] down allocates, searches by name, or visits a node that
//! is not scheduled.

mod ctx;
mod engine;
mod graph;
mod schedule;

use std::any::Any;

pub use ctx::Ctx;
pub use engine::{EngineError, RunConfig, run_simulation};
pub use graph::{Graph, Lifecycle, NodeSlot};

pub use hgl_types::{NodeError, NodeResult, Phase};

/// A node's behaviour. `start` and `stop` default to doing nothing.
/// `Any` is what lets [`Graph::node`] hand a node back as its own type.
///
/// This is the whole of a node. Read it as C++: a struct of three small
/// handles, and its one virtual method.
///
/// ```
/// use hgl_kernel::{Ctx, Node, NodeResult};
/// use hgl_store::{In, Out};
///
/// struct Sum {
///     lhs: In<i64>,
///     rhs: In<i64>,
///     out: Out<i64>,
/// }
///
/// impl Node for Sum {
///     fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
///         ctx.set(self.out, ctx.get(self.lhs) + ctx.get(self.rhs));
///         Ok(())
///     }
/// }
/// ```
pub trait Node: Any {
    /// Called once, in rank order, before any cycle. May schedule the node;
    /// does not write the output (NOD-22).
    fn start(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }

    /// Called when the node is scheduled for this cycle and the inputs it
    /// requires are valid, so it asks neither (NOD-2).
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult;

    /// Called once, in reverse rank order, for every node that started.
    fn stop(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}
