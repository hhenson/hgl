//! The nodes the first slice's cases and benchmarks need.
//!
//! They are written by hand the way an emitter would write them, so each one
//! follows the same recipe and nothing is shared between them: a struct of
//! handles, scalars and state; `impl Node`; `impl Buildable`. Two nodes that
//! differ by one line are two nodes.
//!
//! `Pulse`, `AddOne`, `AddConst`, `Sum` (the baseline's `add`) and `Checksum`
//! match `bench/baselines/cpp/scenarios.cpp` node for node. Arithmetic on
//! `i64` is plain `+`, as the C++ is: a signed overflow there is undefined, so
//! no scenario relies on one.

use hgl_describe::{BuildError, Buildable, Ports, Registry};
use hgl_kernel::{Ctx, Node, NodeResult};
use hgl_store::{In, Out};
use hgl_types::{EngineDelta, NodeType, ScalarType, TsType};

const I64: TsType = TsType::Ts(ScalarType::I64);

/// Register every node in this crate.
pub fn register_all(registry: &mut Registry) -> Result<(), BuildError> {
    registry.register::<AddOne>()?;
    registry.register::<Shift>()?;
    registry.register::<AddConst>()?;
    registry.register::<Sum>()?;
    registry.register::<RunningSum>()?;
    registry.register::<Constant41>()?;
    registry.register::<ConfiguredSource>()?;
    registry.register::<Pulse>()?;
    registry.register::<Checksum>()?;
    Ok(())
}

/// `add_one`: `in + 1`.
#[derive(Debug)]
pub struct AddOne {
    input: In<i64>,
    out: Out<i64>,
}

impl Node for AddOne {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.input) + 1);
        Ok(())
    }
}

impl Buildable for AddOne {
    fn node_type() -> NodeType {
        NodeType {
            name: "add_one",
            inputs: vec![("in", I64)],
            output: Some(I64),
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            out: ports.output()?,
        })
    }
}

/// `shift`: `in + delta`.
#[derive(Debug)]
pub struct Shift {
    input: In<i64>,
    out: Out<i64>,
    delta: i64,
}

impl Node for Shift {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.input) + self.delta);
        Ok(())
    }
}

impl Buildable for Shift {
    fn node_type() -> NodeType {
        NodeType {
            name: "shift",
            inputs: vec![("in", I64)],
            output: Some(I64),
            scalars: vec![("delta", ScalarType::I64)],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            out: ports.output()?,
            delta: ports.scalar("delta")?,
        })
    }
}

/// `add_const`: `in + k`.
#[derive(Debug)]
pub struct AddConst {
    input: In<i64>,
    out: Out<i64>,
    k: i64,
}

impl Node for AddConst {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.input) + self.k);
        Ok(())
    }
}

impl Buildable for AddConst {
    fn node_type() -> NodeType {
        NodeType {
            name: "add_const",
            inputs: vec![("in", I64)],
            output: Some(I64),
            scalars: vec![("k", ScalarType::I64)],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            out: ports.output()?,
            k: ports.scalar("k")?,
        })
    }
}

/// `sum`: `lhs + rhs`. The baseline's `add`.
#[derive(Debug)]
pub struct Sum {
    lhs: In<i64>,
    rhs: In<i64>,
    out: Out<i64>,
}

impl Node for Sum {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.lhs) + ctx.get(self.rhs));
        Ok(())
    }
}

impl Buildable for Sum {
    fn node_type() -> NodeType {
        NodeType {
            name: "sum",
            inputs: vec![("lhs", I64), ("rhs", I64)],
            output: Some(I64),
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            lhs: ports.input("lhs")?,
            rhs: ports.input("rhs")?,
            out: ports.output()?,
        })
    }
}

/// `running_sum`: adds each `in` to a total it keeps, and emits the total.
#[derive(Debug)]
pub struct RunningSum {
    input: In<i64>,
    out: Out<i64>,
    total: i64,
}

impl Node for RunningSum {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.total += ctx.get(self.input);
        ctx.set(self.out, self.total);
        Ok(())
    }
}

impl Buildable for RunningSum {
    fn node_type() -> NodeType {
        NodeType {
            name: "running_sum",
            inputs: vec![("in", I64)],
            output: Some(I64),
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            out: ports.output()?,
            total: 0,
        })
    }
}

/// `constant_41`: emits 41 once, at the start time.
#[derive(Debug)]
pub struct Constant41 {
    out: Out<i64>,
}

impl Node for Constant41 {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, 41);
        Ok(())
    }
}

impl Buildable for Constant41 {
    fn node_type() -> NodeType {
        NodeType {
            name: "constant_41",
            output: Some(I64),
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
        })
    }
}

/// `configured_source`: emits its scalar `value` once, at the start time.
#[derive(Debug)]
pub struct ConfiguredSource {
    out: Out<i64>,
    value: i64,
}

impl Node for ConfiguredSource {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, self.value);
        Ok(())
    }
}

impl Buildable for ConfiguredSource {
    fn node_type() -> NodeType {
        NodeType {
            name: "configured_source",
            output: Some(I64),
            scalars: vec![("value", ScalarType::I64)],
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            value: ports.scalar("value")?,
        })
    }
}

/// `pulse`: emits 0, 1, 2, ... one per cycle from the start time, `count`
/// values in all (at least one, as the baseline).
#[derive(Debug)]
pub struct Pulse {
    out: Out<i64>,
    count: i64,
    emitted: i64,
}

impl Node for Pulse {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let n = self.emitted;
        ctx.set(self.out, n);
        self.emitted = n + 1;
        if n + 1 < self.count {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}

impl Buildable for Pulse {
    fn node_type() -> NodeType {
        NodeType {
            name: "pulse",
            output: Some(I64),
            scalars: vec![("count", ScalarType::I64)],
            uses_scheduler: true,
            schedule_on_start: true,
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            out: ports.output()?,
            count: ports.scalar("count")?,
            emitted: 0,
        })
    }
}

/// `checksum`: a sink that adds each `in` to a total, wrapping as the
/// baseline's unsigned total does, and counts its evaluations.
#[derive(Debug)]
pub struct Checksum {
    input: In<i64>,
    total: u64,
    evals: u64,
}

impl Checksum {
    /// For a benchmark to check after the run, reached with `Graph::node`.
    /// The C++ baseline keeps the same two numbers in globals.
    pub fn total(&self) -> u64 {
        self.total
    }

    /// How many times the node was evaluated.
    pub fn evals(&self) -> u64 {
        self.evals
    }
}

impl Node for Checksum {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.total = self.total.wrapping_add(ctx.get(self.input).cast_unsigned());
        self.evals += 1;
        Ok(())
    }
}

impl Buildable for Checksum {
    fn node_type() -> NodeType {
        NodeType {
            name: "checksum",
            inputs: vec![("in", I64)],
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            input: ports.input("in")?,
            total: 0,
            evals: 0,
        })
    }
}
