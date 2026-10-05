//! Prepared ordinary payloads for whole-value temporal publication.
use hgl_bindings::{Bindings, OutputId, Wake};
use hgl_global_value::{Capacity, GlobalValue, Layouts, ValueColumns, ValueSlot};
use hgl_prepared_value::PreparedValue;
use hgl_shapes::{Atomic, Input, Output};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult, OrdinaryType, TsType};

#[derive(Debug)]
struct Root {
    layout: Vec<usize>,
    installed: bool,
}
/// Ordinary columns and root positions owned by temporal endpoints.
#[derive(Debug, Default)]
pub struct Arena {
    values: ValueColumns,
    roots: Vec<Root>,
    layouts: Layouts,
}
impl Arena {
    /// Install independent finite capacities before evaluation without publication.
    pub fn prepare_output<T: PreparedValue>(
        &mut self,
        bindings: &Bindings,
        output: OutputId,
        bounds: &T::Bounds,
    ) -> NodeResult {
        let slot = T::allocate(&mut self.values, bounds)?;
        let root = &mut self.roots[bindings.output(output).slot as usize];
        if root.installed {
            ValueSlot::<T>::bind(&mut root.layout.as_slice()).release(&mut self.values);
        }
        slot.flatten(&mut root.layout);
        root.installed = true;
        Ok(())
    }
    /// Typed destination after generation validation, without publication.
    /// # Panics
    /// Expired writing tokens are caller errors.
    pub fn destination<T: GlobalValue>(
        &self,
        bindings: &Bindings,
        output: Output<Atomic<T>>,
    ) -> NodeResult<ValueSlot<T>> {
        let endpoint = bindings.output(output.id());
        assert!(
            endpoint.alive && endpoint.generation == output.generation(),
            "expired atomic output"
        );
        let root = &self.roots[endpoint.slot as usize];
        if !root.installed {
            return Err(NodeError::new("atomic destination was not prepared"));
        }
        Ok(ValueSlot::bind(&mut root.layout.as_slice()))
    }
    /// Borrow columns for capacity-checked independent copying.
    pub fn values_mut(&mut self) -> &mut ValueColumns {
        &mut self.values
    }
    /// Prepare root positions once; leaves require an actual successful publication.
    pub fn add_output(
        &mut self,
        bindings: &mut Bindings,
        owner: NodeId,
        ty: OrdinaryType,
    ) -> OutputId {
        let width = width(&ty);
        let prepared = matches!(ty, OrdinaryType::Enum(_));
        let next = u32::try_from(self.roots.len())
            .unwrap_or_else(|_| unreachable!("atomic endpoint capacity exceeded"));
        let (id, fresh) = bindings.add_output(owner, TsType::Atomic(ty), next);
        if fresh {
            self.roots.push(Root {
                layout: if prepared {
                    vec![self.values.append_scalar(0_i64)]
                } else {
                    vec![0; width]
                },
                installed: prepared,
            });
        }
        id
    }
    /// Project typed positions without copying a payload or consulting its schema.
    pub fn borrow<T: GlobalValue>(
        &self,
        bindings: &Bindings,
        input: Input<Atomic<T>>,
    ) -> NodeResult<ValueSlot<T>> {
        if !bindings.valid(input.id()) {
            return Err(NodeError::new("atomic input is invalid"));
        }
        let root = &self.roots[bindings.input(input.id()).slot as usize];
        debug_assert!(
            root.installed,
            "atomic payload must be published before reading"
        );
        Ok(ValueSlot::bind(&mut root.layout.as_slice()))
    }
    /// Read-only columns for ordinary borrowed projections.
    pub fn values(&self) -> &ValueColumns {
        &self.values
    }
    /// Prepare all fallible work before replacing a complete held payload.
    /// # Panics
    /// Expired writing tokens are caller errors.
    pub fn write<T: GlobalValue, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        output: Output<Atomic<T>>,
        value: T::Value,
        now: EngineTime,
        wake: &mut W,
    ) -> NodeResult {
        let endpoint = bindings.output(output.id());
        assert!(
            endpoint.alive && endpoint.generation == output.generation(),
            "expired atomic output"
        );
        debug_assert!(
            now != EngineTime::NEVER && now >= endpoint.modified_at,
            "atomic publication time"
        );
        let slot = endpoint.slot;
        let mut capacity = Capacity::default();
        self.layouts.reset();
        if !T::PREPARED_SCALAR {
            T::prepare(&value, &mut capacity, &mut self.layouts)?;
            self.values.reserve(&capacity)?;
        }
        let root = &mut self.roots[slot as usize];
        if root.installed {
            ValueSlot::<T>::bind(&mut root.layout.as_slice()).commit(
                &mut self.values,
                value,
                &mut self.layouts,
            );
        } else {
            let slots = ValueSlot::<T>::install(&mut self.values, value, &mut self.layouts);
            slots.flatten(&mut root.layout);
            root.installed = true;
        }
        bindings.publish(output.id(), now, wake);
        Ok(())
    }
}
fn width(ty: &OrdinaryType) -> usize {
    match ty {
        OrdinaryType::Enum(_) | OrdinaryType::Scalar(_) | OrdinaryType::List(..) => 1,
        OrdinaryType::Tuple(fields) => fields.iter().map(width).sum(),
        OrdinaryType::Struct(_, fields) => fields.iter().map(|(_, ty)| width(ty)).sum(),
    }
}
