//! Static endpoint shapes and construction-validated projection tokens.
use hgl_bindings::{BindError, Bindings, InputId, OutputId};
use hgl_types::{ScalarType, TsType};
use std::marker::PhantomData;

/// A compile-time endpoint shape; metadata is used only during construction.
pub trait Shape {
    /// Exact structural metadata.
    fn shape() -> TsType;
}
/// A statically selected heterogeneous child.
pub trait Field<const N: usize>: Shape {
    /// Child shape at N.
    type Child: Shape;
}
/// A homogeneous fixed or keyed child shape.
pub trait Elements: Shape {
    /// Repeated child shape.
    type Child: Shape;
}
/// A complete ordinary value carried by one temporal endpoint.
#[derive(Debug)]
pub struct Atomic<T>(PhantomData<T>);
impl<T: hgl_global_value::GlobalValue> Shape for Atomic<T> {
    fn shape() -> TsType {
        TsType::Atomic(T::schema())
    }
}
/// Fixed temporal list marker.
#[derive(Debug)]
pub struct Fixed<S, const N: usize>(PhantomData<S>);
/// Integer-keyed temporal map marker.
#[derive(Debug)]
pub struct Map<S, K = i64>(PhantomData<(S, K)>);
/// Canonical bool/i64 set marker.
#[derive(Debug)]
pub struct Set<K>(PhantomData<K>);
impl<S: Shape, const N: usize> Shape for Fixed<S, N> {
    fn shape() -> TsType {
        TsType::List(Box::new(S::shape()), N)
    }
}
impl<S: Shape, const N: usize> Elements for Fixed<S, N> {
    type Child = S;
}
impl<S: Shape, K: hgl_keys::Key> Shape for Map<S, K> {
    fn shape() -> TsType {
        if K::schema() == hgl_types::OrdinaryType::Scalar(ScalarType::I64) {
            TsType::Dictionary(Box::new(S::shape()))
        } else {
            TsType::KeyedDictionary(K::schema(), Box::new(S::shape()))
        }
    }
}
impl<S: Shape, K: hgl_keys::Key> Elements for Map<S, K> {
    type Child = S;
}
impl<K: hgl_keys::Key> Shape for Set<K> {
    fn shape() -> TsType {
        match K::schema() {
            hgl_types::OrdinaryType::Scalar(t) => TsType::Set(t),
            identity @ (hgl_types::OrdinaryType::OptionalField(_)
            | hgl_types::OrdinaryType::Enum(_)
            | hgl_types::OrdinaryType::List(..)
            | hgl_types::OrdinaryType::Tuple(_)
            | hgl_types::OrdinaryType::Struct(..)) => TsType::KeyedSet(identity),
        }
    }
}
impl<K: hgl_keys::Key> Elements for Set<K> {
    type Child = bool;
}
impl Shape for bool {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::Bool)
    }
}
impl Shape for i64 {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::I64)
    }
}
impl Shape for f64 {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::F64)
    }
}
impl Shape for String {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::Text)
    }
}
impl Shape for hgl_types::Date {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::Date)
    }
}
impl Shape for hgl_types::Time {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::Time)
    }
}
impl Shape for hgl_types::EngineTime {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::DateTime)
    }
}
impl Shape for hgl_types::EngineDelta {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::Duration)
    }
}
/// A shape-checked input identity; projections never copy held values.
pub struct Input<S> {
    id: InputId,
    marker: PhantomData<S>,
}
/// A shape-checked writing identity with its allocation generation.
pub struct Output<S> {
    id: OutputId,
    generation: u32,
    marker: PhantomData<S>,
}
impl<S> Copy for Input<S> {}
impl<S> Clone for Input<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> std::fmt::Debug for Input<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Input")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
impl<S> Copy for Output<S> {}
impl<S> Clone for Output<S> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<S> std::fmt::Debug for Output<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Output")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}
impl<S: Shape> Input<S> {
    /// Validate a root once, before using typed projections.
    pub fn bind(bindings: &Bindings, id: InputId) -> Result<Self, BindError> {
        if bindings.input(id).kind != S::shape() {
            return Err(BindError::ShapeMismatch);
        }
        Ok(Self {
            id,
            marker: PhantomData,
        })
    }
    /// Erased identity for validity and modification metadata.
    pub fn id(self) -> InputId {
        self.id
    }
    /// Project a compile-time field position.
    pub fn field<const N: usize>(self, b: &Bindings) -> Input<S::Child>
    where
        S: Field<N>,
    {
        Input {
            id: b.fixed_input(self.id, N),
            marker: PhantomData,
        }
    }
    /// Project a homogeneous dense position, whose bounds are checked by the child table.
    pub fn index(self, b: &Bindings, n: usize) -> Input<S::Child>
    where
        S: Elements,
    {
        Input {
            id: b.fixed_input(self.id, n),
            marker: PhantomData,
        }
    }
    /// Project live keyed membership without consulting shape metadata.
    pub fn member(self, b: &Bindings, key: i64) -> Option<Input<S::Child>>
    where
        S: Elements,
    {
        b.child_input(self.id, key).map(|id| Input {
            id,
            marker: PhantomData,
        })
    }
}
impl<S: Shape> Output<S> {
    /// Validate a root once during construction.
    pub fn bind(bindings: &Bindings, id: OutputId) -> Result<Self, BindError> {
        if bindings.output(id).kind != S::shape() {
            return Err(BindError::ShapeMismatch);
        }
        Ok(Self::project(bindings, id))
    }
    fn project(b: &Bindings, id: OutputId) -> Self {
        Self {
            id,
            generation: b.output(id).generation,
            marker: PhantomData,
        }
    }
    fn current(self, b: &Bindings) -> OutputId {
        debug_assert_eq!(
            b.output(self.id).generation,
            self.generation,
            "prepared output generation"
        );
        self.id
    }
    /// Erased identity for membership operations.
    pub fn id(self) -> OutputId {
        self.id
    }
    /// Generation retained when adapting scalar leaf handles.
    pub fn generation(self) -> u32 {
        self.generation
    }
    /// Project a compile-time field position.
    pub fn field<const N: usize>(self, b: &Bindings) -> Output<S::Child>
    where
        S: Field<N>,
    {
        Output::project(b, b.fixed_output(self.current(b), N))
    }
    /// Project a homogeneous dense position.
    pub fn index(self, b: &Bindings, n: usize) -> Output<S::Child>
    where
        S: Elements,
    {
        Output::project(b, b.fixed_output(self.current(b), n))
    }
    /// Project existing keyed membership.
    pub fn member(self, b: &Bindings, key: i64) -> Option<Output<S::Child>>
    where
        S: Elements,
    {
        b.child_output(self.current(b), key)
            .map(|id| Output::project(b, id))
    }
}

impl Shape for hgl_types::CivilDateTime {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::CivilDateTime)
    }
}
impl Shape for hgl_types::ZoneId {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::TimeZone)
    }
}
impl Shape for hgl_types::ZonedDateTime {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::ZonedDateTime)
    }
}

impl Shape for hgl_types::ZonedTime {
    fn shape() -> TsType {
        TsType::Ts(ScalarType::ZonedTime)
    }
}
