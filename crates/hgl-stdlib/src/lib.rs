//! Fresh-run standard-library ports, written as the future emitter would.
//! These concrete implementation keys are not HGL operator identities.
//! Deduplication history is not checkpointed; recovery requires further work.
use hgl_describe::{BuildError, Buildable, Ports, Registry};
use hgl_kernel::{Ctx, Node, NodeResult};
use hgl_store::{In, Out};
use hgl_types::{NodeType, ScalarType, TsType};

const I64: TsType = TsType::Ts(ScalarType::I64);

/// Register the three concrete fresh-run specimens.
pub fn register_all(registry: &mut Registry) -> Result<(), BuildError> {
    registry.register::<BitAndI64>()?;
    registry.register::<SampleI64>()?;
    registry.register::<DedupI64>()?;
    Ok(())
}

/// The i64 specialization of `hgraph.operators.bit_and`.
#[derive(Debug)]
pub struct BitAndI64 {
    lhs: In<i64>,
    rhs: In<i64>,
    out: Out<i64>,
}

impl Node for BitAndI64 {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(
            self.out,
            native::bit_and_i64(ctx.get(self.lhs), ctx.get(self.rhs)),
        );
        Ok(())
    }
}

impl Buildable for BitAndI64 {
    fn node_type() -> NodeType {
        NodeType {
            name: "hgraph.operators.bit_and.i64",
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

/// `hgraph.std.sample<i64>` with an i64 payload-erased signal input.
#[derive(Debug)]
pub struct SampleI64 {
    signal: In<i64>,
    ts: In<i64>,
    out: Out<i64>,
}

impl Node for SampleI64 {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if ctx.modified(self.signal) {
            ctx.set(self.out, ctx.get(self.ts));
        }
        Ok(())
    }
}

impl Buildable for SampleI64 {
    fn node_type() -> NodeType {
        NodeType {
            name: "hgraph.std.sample.i64",
            inputs: vec![("signal", I64), ("ts", I64)],
            output: Some(I64),
            active_inputs: Some(vec![0]),
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            signal: ports.input("signal")?,
            ts: ports.input("ts")?,
            out: ports.output()?,
        })
    }
}

/// Fresh-run `hgraph.std.dedup<i64>`; history is node-local but not checkpointed.
#[derive(Debug)]
pub struct DedupI64 {
    ts: In<i64>,
    out: Out<i64>,
    last: Option<i64>,
}

impl Node for DedupI64 {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let value = ctx.get(self.ts);
        if self.last != Some(value) {
            ctx.set(self.out, value);
        }
        self.last = Some(value);
        Ok(())
    }
}

impl Buildable for DedupI64 {
    fn node_type() -> NodeType {
        NodeType {
            name: "hgraph.std.dedup.i64",
            inputs: vec![("ts", I64)],
            output: Some(I64),
            ..NodeType::default()
        }
    }
    fn build(ports: &mut Ports<'_>) -> Result<Self, BuildError> {
        Ok(Self {
            ts: ports.input("ts")?,
            out: ports.output()?,
            last: None,
        })
    }
}

pub mod native;
pub mod std_native;
