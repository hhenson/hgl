//! One ordinary outcome and canonical diagnostics retain independent typed descendants.
use crate::{Capacity, GlobalValue, Layouts, Optional, PreparedValue, ValueColumns, ValueSlot};
use hgl_types::{EngineTime, ErrorContext, ErrorValue, NodeError, NodeResult, OrdinaryType};
use std::marker::PhantomData;
/// Typed storage for a complete `ResultValue` payload.
#[derive(Debug)]
pub struct ResultValue<T: GlobalValue, E: GlobalValue>(PhantomData<(T, E)>);
impl<T: GlobalValue, E: GlobalValue> GlobalValue for ResultValue<T, E> {
    type Value = Result<T::Value, E::Value>;
    type Slots = (
        ValueSlot<bool>,
        ValueSlot<Optional<T>>,
        ValueSlot<Optional<E>>,
    );
    const WIDTH: usize = 3;
    fn schema() -> OrdinaryType {
        OrdinaryType::Result(Box::new(T::schema()), Box::new(E::schema()))
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (
            ValueSlot::<bool>::bind(layout),
            ValueSlot::<Optional<T>>::bind(layout),
            ValueSlot::<Optional<E>>::bind(layout),
        )
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[0..1]);
        slots.1.flatten(&mut layout[1..2]);
        slots.2.flatten(&mut layout[2..3]);
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        let tag = value.is_ok();
        let fields = (&tag, value.as_ref().ok(), value.as_ref().err());
        bool::prepare(fields.0, capacity, layouts)?;
        Optional::<T>::prepare_ref(fields.1, capacity, layouts)?;
        Optional::<E>::prepare_ref(fields.2, capacity, layouts)?;
        Ok(())
    }
    fn install(
        columns: &mut ValueColumns,
        value: Self::Value,
        layouts: &mut Layouts,
    ) -> Self::Slots {
        let fields = match value {
            Ok(value) => (true, Some(value), None),
            Err(value) => (false, None, Some(value)),
        };
        (
            ValueSlot::<bool>::install(columns, fields.0, layouts),
            ValueSlot::<Optional<T>>::install(columns, fields.1, layouts),
            ValueSlot::<Optional<E>>::install(columns, fields.2, layouts),
        )
    }
    fn release(columns: &mut ValueColumns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
        slots.2.release(columns);
    }
    fn read(columns: &ValueColumns, slots: Self::Slots) -> NodeResult<Self::Value> {
        let fields = (
            slots.0.read(columns)?,
            slots.1.read(columns)?,
            slots.2.read(columns)?,
        );
        if fields.0 {
            Ok(Ok(fields.1.ok_or_else(|| {
                NodeError::new("missing success payload")
            })?))
        } else {
            Ok(Err(fields
                .2
                .ok_or_else(|| NodeError::new("missing error payload"))?))
        }
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        let tag = value.is_ok();
        let fields = (&tag, value.as_ref().ok(), value.as_ref().err());
        let fields = (
            bool::retain(fields.0)?,
            fields.1.map(T::retain).transpose()?,
            fields.2.map(E::retain).transpose()?,
        );
        if fields.0 {
            Ok(Ok(fields.1.ok_or_else(|| {
                NodeError::new("missing success payload")
            })?))
        } else {
            Ok(Err(fields
                .2
                .ok_or_else(|| NodeError::new("missing error payload"))?))
        }
    }
    fn commit(
        columns: &mut ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        let fields = match value {
            Ok(value) => (true, Some(value), None),
            Err(value) => (false, None, Some(value)),
        };
        slots.0.commit(columns, fields.0, layouts);
        slots.1.commit(columns, fields.1, layouts);
        slots.2.commit(columns, fields.2, layouts);
    }
}
/// Cold field maxima provision each independently owned branch.
pub struct ResultBounds<T: PreparedValue, E: PreparedValue> {
    /// Bounds of field 0.
    pub field0: <bool as PreparedValue>::Bounds,
    /// Bounds of field 1.
    pub field1: <Optional<T> as PreparedValue>::Bounds,
    /// Bounds of field 2.
    pub field2: <Optional<E> as PreparedValue>::Bounds,
}
impl<T: PreparedValue, E: PreparedValue> Default for ResultBounds<T, E> {
    fn default() -> Self {
        Self {
            field0: Default::default(),
            field1: None,
            field2: None,
        }
    }
}
impl<T: PreparedValue, E: PreparedValue> PreparedValue for ResultValue<T, E> {
    type Bounds = ResultBounds<T, E>;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        let tag = value.is_ok();
        let fields = (&tag, value.as_ref().ok(), value.as_ref().err());
        bool::include(&mut bounds.field0, fields.0);
        Optional::<T>::include_ref(&mut bounds.field1, fields.1);
        Optional::<E>::include_ref(&mut bounds.field2, fields.2);
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        Ok(ValueSlot::from_fields((
            bool::allocate(columns, &bounds.field0)?,
            Optional::<T>::allocate(columns, &bounds.field1)?,
            Optional::<E>::allocate(columns, &bounds.field2)?,
        )))
    }
    fn check_native(
        columns: &ValueColumns,
        destination: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        let tag = value.is_ok();
        let fields = (&tag, value.as_ref().ok(), value.as_ref().err());
        bool::check_native(columns, destination.fields().0, fields.0)?;
        Optional::<T>::check_ref(columns, destination.fields().1, fields.1)?;
        Optional::<E>::check_ref(columns, destination.fields().2, fields.2)?;
        Ok(())
    }
    fn copy_native(columns: &mut ValueColumns, destination: ValueSlot<Self>, value: &Self::Value) {
        let tag = value.is_ok();
        let fields = (&tag, value.as_ref().ok(), value.as_ref().err());
        bool::copy_native(columns, destination.fields().0, fields.0);
        Optional::<T>::copy_ref(columns, destination.fields().1, fields.1);
        Optional::<E>::copy_ref(columns, destination.fields().2, fields.2);
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        bool::check_slots(source, from.fields().0, destination, to.fields().0)?;
        Optional::<T>::check_slots(source, from.fields().1, destination, to.fields().1)?;
        Optional::<E>::check_slots(source, from.fields().2, destination, to.fields().2)?;
        Ok(())
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        bool::copy_between(source, from.fields().0, destination, to.fields().0);
        Optional::<T>::copy_between(source, from.fields().1, destination, to.fields().1);
        Optional::<E>::copy_between(source, from.fields().2, destination, to.fields().2);
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        bool::copy_within(columns, from.fields().0, to.fields().0);
        Optional::<T>::copy_within(columns, from.fields().1, to.fields().1);
        Optional::<E>::copy_within(columns, from.fields().2, to.fields().2);
    }
}
/// Typed storage for a complete Context payload.
#[derive(Debug)]
pub struct Context;
impl GlobalValue for Context {
    type Value = ErrorContext;
    type Slots = (ValueSlot<String>, ValueSlot<String>, ValueSlot<EngineTime>);
    const WIDTH: usize = 3;
    fn schema() -> OrdinaryType {
        OrdinaryType::Tuple(vec![
            String::schema(),
            String::schema(),
            EngineTime::schema(),
        ])
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (
            ValueSlot::<String>::bind(layout),
            ValueSlot::<String>::bind(layout),
            ValueSlot::<EngineTime>::bind(layout),
        )
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[0..1]);
        slots.1.flatten(&mut layout[1..2]);
        slots.2.flatten(&mut layout[2..3]);
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        let fields = (&value.node, &value.phase, &value.evaluation_time);
        String::prepare(fields.0, capacity, layouts)?;
        String::prepare(fields.1, capacity, layouts)?;
        EngineTime::prepare(fields.2, capacity, layouts)?;
        Ok(())
    }
    fn install(
        columns: &mut ValueColumns,
        value: Self::Value,
        layouts: &mut Layouts,
    ) -> Self::Slots {
        let fields = (value.node, value.phase, value.evaluation_time);
        (
            ValueSlot::<String>::install(columns, fields.0, layouts),
            ValueSlot::<String>::install(columns, fields.1, layouts),
            ValueSlot::<EngineTime>::install(columns, fields.2, layouts),
        )
    }
    fn release(columns: &mut ValueColumns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
        slots.2.release(columns);
    }
    fn read(columns: &ValueColumns, slots: Self::Slots) -> NodeResult<Self::Value> {
        let fields = (
            slots.0.read(columns)?,
            slots.1.read(columns)?,
            slots.2.read(columns)?,
        );
        Ok(ErrorContext {
            node: fields.0,
            phase: fields.1,
            evaluation_time: fields.2,
        })
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        let fields = (&value.node, &value.phase, &value.evaluation_time);
        let fields = (
            <String as GlobalValue>::retain(fields.0)?,
            <String as GlobalValue>::retain(fields.1)?,
            EngineTime::retain(fields.2)?,
        );
        Ok(ErrorContext {
            node: fields.0,
            phase: fields.1,
            evaluation_time: fields.2,
        })
    }
    fn commit(
        columns: &mut ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        let fields = (value.node, value.phase, value.evaluation_time);
        slots.0.commit(columns, fields.0, layouts);
        slots.1.commit(columns, fields.1, layouts);
        slots.2.commit(columns, fields.2, layouts);
    }
}
/// Cold field maxima provision each independently owned branch.
#[derive(Debug, Default, Clone)]
pub struct ContextBounds {
    /// Bounds of field 0.
    pub field0: <String as PreparedValue>::Bounds,
    /// Bounds of field 1.
    pub field1: <String as PreparedValue>::Bounds,
    /// Bounds of field 2.
    pub field2: <EngineTime as PreparedValue>::Bounds,
}

impl PreparedValue for Context {
    type Bounds = ContextBounds;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        let fields = (&value.node, &value.phase, &value.evaluation_time);
        String::include(&mut bounds.field0, fields.0);
        String::include(&mut bounds.field1, fields.1);
        EngineTime::include(&mut bounds.field2, fields.2);
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        Ok(ValueSlot::from_fields((
            String::allocate(columns, &bounds.field0)?,
            String::allocate(columns, &bounds.field1)?,
            EngineTime::allocate(columns, &bounds.field2)?,
        )))
    }
    fn check_native(
        columns: &ValueColumns,
        destination: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        let fields = (&value.node, &value.phase, &value.evaluation_time);
        String::check_native(columns, destination.fields().0, fields.0)?;
        String::check_native(columns, destination.fields().1, fields.1)?;
        EngineTime::check_native(columns, destination.fields().2, fields.2)?;
        Ok(())
    }
    fn copy_native(columns: &mut ValueColumns, destination: ValueSlot<Self>, value: &Self::Value) {
        let fields = (&value.node, &value.phase, &value.evaluation_time);
        String::copy_native(columns, destination.fields().0, fields.0);
        String::copy_native(columns, destination.fields().1, fields.1);
        EngineTime::copy_native(columns, destination.fields().2, fields.2);
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        String::check_slots(source, from.fields().0, destination, to.fields().0)?;
        String::check_slots(source, from.fields().1, destination, to.fields().1)?;
        EngineTime::check_slots(source, from.fields().2, destination, to.fields().2)?;
        Ok(())
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        String::copy_between(source, from.fields().0, destination, to.fields().0);
        String::copy_between(source, from.fields().1, destination, to.fields().1);
        EngineTime::copy_between(source, from.fields().2, destination, to.fields().2);
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        String::copy_within(columns, from.fields().0, to.fields().0);
        String::copy_within(columns, from.fields().1, to.fields().1);
        EngineTime::copy_within(columns, from.fields().2, to.fields().2);
    }
}
/// Typed storage for a complete Error payload.
#[derive(Debug)]
pub struct Error;
impl GlobalValue for Error {
    type Value = ErrorValue;
    type Slots = (
        ValueSlot<String>,
        ValueSlot<Optional<String>>,
        ValueSlot<Optional<Context>>,
    );
    const WIDTH: usize = 3;
    fn schema() -> OrdinaryType {
        OrdinaryType::Error
    }
    fn slots(layout: &mut &[usize]) -> Self::Slots {
        (
            ValueSlot::<String>::bind(layout),
            ValueSlot::<Optional<String>>::bind(layout),
            ValueSlot::<Optional<Context>>::bind(layout),
        )
    }
    fn flatten(slots: Self::Slots, layout: &mut [usize]) {
        slots.0.flatten(&mut layout[0..1]);
        slots.1.flatten(&mut layout[1..2]);
        slots.2.flatten(&mut layout[2..3]);
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        let fields = (&value.message, value.code.as_ref(), value.context.as_ref());
        String::prepare(fields.0, capacity, layouts)?;
        Optional::<String>::prepare_ref(fields.1, capacity, layouts)?;
        Optional::<Context>::prepare_ref(fields.2, capacity, layouts)?;
        Ok(())
    }
    fn install(
        columns: &mut ValueColumns,
        value: Self::Value,
        layouts: &mut Layouts,
    ) -> Self::Slots {
        let fields = (value.message, value.code, value.context);
        (
            ValueSlot::<String>::install(columns, fields.0, layouts),
            ValueSlot::<Optional<String>>::install(columns, fields.1, layouts),
            ValueSlot::<Optional<Context>>::install(columns, fields.2, layouts),
        )
    }
    fn release(columns: &mut ValueColumns, slots: Self::Slots) {
        slots.0.release(columns);
        slots.1.release(columns);
        slots.2.release(columns);
    }
    fn read(columns: &ValueColumns, slots: Self::Slots) -> NodeResult<Self::Value> {
        let fields = (
            slots.0.read(columns)?,
            slots.1.read(columns)?,
            slots.2.read(columns)?,
        );
        Ok(ErrorValue {
            message: fields.0,
            code: fields.1,
            context: fields.2,
        })
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        let fields = (&value.message, value.code.as_ref(), value.context.as_ref());
        let fields = (
            <String as GlobalValue>::retain(fields.0)?,
            fields.1.map(<String as GlobalValue>::retain).transpose()?,
            fields.2.map(Context::retain).transpose()?,
        );
        Ok(ErrorValue {
            message: fields.0,
            code: fields.1,
            context: fields.2,
        })
    }
    fn commit(
        columns: &mut ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    ) {
        let fields = (value.message, value.code, value.context);
        slots.0.commit(columns, fields.0, layouts);
        slots.1.commit(columns, fields.1, layouts);
        slots.2.commit(columns, fields.2, layouts);
    }
}
/// Cold field maxima provision each independently owned branch.
#[derive(Debug, Default, Clone)]
pub struct ErrorBounds {
    /// Bounds of field 0.
    pub field0: <String as PreparedValue>::Bounds,
    /// Bounds of field 1.
    pub field1: <Optional<String> as PreparedValue>::Bounds,
    /// Bounds of field 2.
    pub field2: <Optional<Context> as PreparedValue>::Bounds,
}

impl PreparedValue for Error {
    type Bounds = ErrorBounds;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        let fields = (&value.message, value.code.as_ref(), value.context.as_ref());
        String::include(&mut bounds.field0, fields.0);
        Optional::<String>::include_ref(&mut bounds.field1, fields.1);
        Optional::<Context>::include_ref(&mut bounds.field2, fields.2);
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        Ok(ValueSlot::from_fields((
            String::allocate(columns, &bounds.field0)?,
            Optional::<String>::allocate(columns, &bounds.field1)?,
            Optional::<Context>::allocate(columns, &bounds.field2)?,
        )))
    }
    fn check_native(
        columns: &ValueColumns,
        destination: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        let fields = (&value.message, value.code.as_ref(), value.context.as_ref());
        String::check_native(columns, destination.fields().0, fields.0)?;
        Optional::<String>::check_ref(columns, destination.fields().1, fields.1)?;
        Optional::<Context>::check_ref(columns, destination.fields().2, fields.2)?;
        Ok(())
    }
    fn copy_native(columns: &mut ValueColumns, destination: ValueSlot<Self>, value: &Self::Value) {
        let fields = (&value.message, value.code.as_ref(), value.context.as_ref());
        String::copy_native(columns, destination.fields().0, fields.0);
        Optional::<String>::copy_ref(columns, destination.fields().1, fields.1);
        Optional::<Context>::copy_ref(columns, destination.fields().2, fields.2);
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        String::check_slots(source, from.fields().0, destination, to.fields().0)?;
        Optional::<String>::check_slots(source, from.fields().1, destination, to.fields().1)?;
        Optional::<Context>::check_slots(source, from.fields().2, destination, to.fields().2)?;
        Ok(())
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        String::copy_between(source, from.fields().0, destination, to.fields().0);
        Optional::<String>::copy_between(source, from.fields().1, destination, to.fields().1);
        Optional::<Context>::copy_between(source, from.fields().2, destination, to.fields().2);
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        String::copy_within(columns, from.fields().0, to.fields().0);
        Optional::<String>::copy_within(columns, from.fields().1, to.fields().1);
        Optional::<Context>::copy_within(columns, from.fields().2, to.fields().2);
    }
}

impl<T: PreparedValue, E: PreparedValue> std::fmt::Debug for ResultBounds<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResultBounds").finish_non_exhaustive()
    }
}

impl<T: PreparedValue, E: PreparedValue> Clone for ResultBounds<T, E>
where
    T::Bounds: Clone,
    E::Bounds: Clone,
{
    fn clone(&self) -> Self {
        Self {
            field0: self.field0,
            field1: self.field1.clone(),
            field2: self.field2.clone(),
        }
    }
}
