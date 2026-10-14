//! Owning boxes use wiring-selected layouts; encoded data contains no borrowed handles.
use crate::{NodeError, NodeResult};
use std::{
    any::TypeId,
    cmp::Ordering,
    hash::{Hash, Hasher},
};

mod data;
pub use data::AnyView;

/// A collision-free compile-time chunk of canonical UTF-8 type identity.
#[derive(Debug)]
pub struct IdentityChunk<const BYTES: u128, const LENGTH: usize>;

/// Append a checked provider encoding into a selected owning sink.
pub type NativeCopy = fn(&[u8], &mut dyn AnyBuffer);
/// Validate provider retention before mutation.
pub type NativeCheck = fn(&[u8], usize) -> NodeResult;
/// Fallible cold-only conversion with no dependency on the checked value model.
pub type ColdCapture = fn(AnyView<'_>) -> NodeResult<Box<dyn std::any::Any>>;

/// Cold-only validation of untrusted descriptor payloads before decoding.
pub type ColdValidate = fn(AnyView<'_>) -> NodeResult;

/// Recursive runtime operations supplied without a runtime dependency in Types.
#[derive(Debug)]
pub struct AnyExecution {
    /// Exact equality.
    pub equal: fn(&AnyPlan, AnyView<'_>, AnyView<'_>) -> NodeResult<bool>,
    /// Exact partial ordering.
    pub compare: fn(&AnyPlan, AnyView<'_>, AnyView<'_>) -> NodeResult<Option<Ordering>>,
    /// Capability-checked hashing.
    pub hash: fn(&AnyPlan, AnyView<'_>) -> NodeResult<u64>,
}
/// An exact concrete layout and its declared value capabilities.
#[derive(Debug)]
pub struct AnyPlan {
    /// Runtime layout execution selected at compilation.
    pub execution: &'static AnyExecution,
    /// Generated cold conversion; its carrier belongs to the compiler layer.
    pub capture: Option<ColdCapture>,
    /// Complete cold payload validation, absent for descriptors with no cold mapping.
    pub validate: Option<ColdValidate>,
    /// Canonical nominal marker identity, independent of its native representation.
    pub identity: fn() -> TypeId,
    /// Precompiled operations over this layout.
    pub operation: AnyOperation,
    /// Compiled provider-preserving owning copy after preflight.
    pub copy: fn(AnyView<'_>, &mut dyn AnyBuffer),
    /// Equality, hash and order capability declarations.
    pub capabilities: [bool; 3],
}
/// A field's existing optional presence is part of its owning data.
#[derive(Debug)]
pub struct AnyField {
    /// Concrete child layout.
    pub plan: &'static AnyPlan,
    /// Whether a presence bit precedes the child frame.
    pub optional: bool,
}
/// Storage operations selected during compilation, never inferred from payload tags.
#[derive(Debug)]
pub enum AnyOperation {
    /// Operations supplied by the checked opaque provider.
    Native {
        /// Provider equality.
        equal: fn(&[u8], &[u8]) -> NodeResult<bool>,
        /// Provider hash.
        hash: fn(&[u8]) -> NodeResult<u64>,
        /// Provider ordering.
        compare: fn(&[u8], &[u8]) -> NodeResult<Option<Ordering>>,
    },
    /// Signed integer or enum member.
    Integer,
    /// IEEE floating-point value, preserving NaN and signed-zero semantics.
    Float,
    /// Exact octets, including text and bool encodings.
    Octets,
    /// Declaration-ordered independent fields.
    Fields(&'static [AnyField]),
    /// Complete list or unordered set/map entries.
    Sequence(&'static AnyPlan, bool),
    /// A single tagged outcome delegates equality to its selected ordinary payload.
    Result(&'static AnyPlan, &'static AnyPlan),
    /// A diagnostic retains provenance but compares only its message and nullable code.
    Error(&'static AnyPlan),
    /// A closed abstract family retains its concrete member.
    Family(&'static [&'static AnyPlan]),
    /// A nested box selects an already compiled descriptor.
    Boxed,
    /// Owning data with no equality, hash or order, such as a structural delta.
    Data,
}
/// Complete retained bytes and the concrete layout selected before publication.
#[derive(Debug, Default)]
pub struct AnyValue {
    plan: Option<&'static AnyPlan>,
    data: Vec<u8>,
    descriptors: Vec<&'static AnyPlan>,
}
impl Clone for AnyValue {
    fn clone(&self) -> Self {
        let mut retained = Self {
            data: Vec::with_capacity(self.data.len()),
            descriptors: Vec::with_capacity(self.descriptors.len()),
            plan: None,
        };
        retained.copy_prepared(self);
        retained
    }
}
impl AnyValue {
    /// Install independently owned data produced by its statically selected codec.
    pub fn from_encoded(plan: &'static AnyPlan, data: impl Into<Self>) -> Self {
        Self {
            plan: Some(plan),
            ..data.into()
        }
    }
    /// Read the descriptor without inspecting or unboxing source values.
    pub fn plan(&self) -> Option<&'static AnyPlan> {
        self.plan
    }
    /// Borrow encoded owning storage within the consumer's existing lifetime.
    pub fn data(&self) -> AnyView<'_> {
        AnyView::from_encoded(&self.data, &self.descriptors)
    }
    /// Reserve owning capacity before hooks execute.
    pub fn reserve(&mut self, size: usize) -> NodeResult {
        self.data
            .try_reserve(size.saturating_sub(self.data.len()))
            .and_then(|()| {
                self.descriptors
                    .try_reserve((size / 8).saturating_sub(self.descriptors.len()))
            })
            .map_err(|error| NodeError::new(error.to_string()))
    }
    /// Prepared width is the complete encoded size.
    pub fn capacity(&self) -> usize {
        self.data.capacity().min(
            self.descriptors
                .capacity()
                .saturating_mul(8)
                .saturating_add(7),
        )
    }
    /// Validate complete reserved byte and descriptor storage before mutation.
    pub fn check_capacity(&self, width: usize) -> NodeResult {
        if self.capacity() < width {
            return Err(NodeError::new("prepared any capacity exceeded"));
        }
        Ok(())
    }
    /// Copy only after the caller has checked the complete prepared capacity.
    pub fn copy_prepared(&mut self, source: &Self) {
        self.encode_prepared(source.plan, |out| {
            if let Some(plan) = source.plan {
                (plan.copy)(source.data(), out);
            }
        });
    }
    /// Assemble a completely preflighted publication in reserved owning capacity.
    pub fn encode_prepared(
        &mut self,
        plan: Option<&'static AnyPlan>,
        encode: impl FnOnce(&mut Self),
    ) {
        self.data.clear();
        self.descriptors.clear();
        encode(self);
        self.plan = plan;
    }
    /// Retain independently with allocation failures on the ordinary error channel.
    pub fn retain(&self) -> NodeResult<Self> {
        let mut retained = Self::default();
        retained.reserve(self.data.len())?;
        self.preflight()?;
        retained.copy_prepared(self);
        Ok(retained)
    }
    /// Validate provider retention before changing any owning destination.
    pub fn preflight(&self) -> NodeResult {
        let mut width = AnyWidth::default();
        if let Some(plan) = self.plan {
            (plan.copy)(self.data(), &mut width);
        }
        width.finish().map(|_| ())
    }
    /// Recover the compiler's cold carrier, rejecting incompatible descriptors as errors.
    pub fn capture<T: 'static>(&self) -> NodeResult<T> {
        let callback = self
            .plan
            .and_then(|p| p.capture)
            .ok_or_else(|| NodeError::new("missing cold any descriptor conversion"))?;
        callback(self.data())?
            .downcast::<T>()
            .map(|value| *value)
            .map_err(|_carrier| NodeError::new("incompatible cold any descriptor carrier"))
    }
    /// Compare exact concrete identity and contents.
    pub fn equal(&self, other: &Self) -> NodeResult<bool> {
        self.borrowed().equal(&other.borrowed())
    }
    /// Compare exact concrete identity and contents without coercion.
    pub fn compare(&self, other: &Self) -> NodeResult<Option<Ordering>> {
        self.borrowed().compare(&other.borrowed())
    }
    /// Hash complete contents through their retained descriptor.
    pub fn key_hash(&self) -> NodeResult<u64> {
        self.borrowed().key_hash()
    }
    /// Borrow the exact retained descriptor and contents without copying ownership.
    pub fn borrowed(&self) -> AnyRef<'_> {
        AnyRef {
            plan: self.plan,
            data: self.data(),
        }
    }
}
/// A descriptor and its owning-data borrow, used for root and nested dispatch.
#[derive(Debug, Clone, Copy)]
pub struct AnyRef<'a> {
    /// Exact retained descriptor, or an empty box.
    pub plan: Option<&'static AnyPlan>,
    /// Borrowed payload and descriptor context.
    pub data: AnyView<'a>,
}
impl AnyRef<'_> {
    /// Compare concrete identity before requesting contained equality.
    pub fn equal(&self, other: &Self) -> NodeResult<bool> {
        match (self.plan, other.plan) {
            (None, None) => Ok(true),
            (Some(a), Some(b)) if (a.identity)() == (b.identity)() => {
                a.equal(self.data, other.data)
            }
            _ => Ok(false),
        }
    }
    /// Empty precedes populated; different concrete types remain unordered.
    pub fn compare(&self, other: &Self) -> NodeResult<Option<Ordering>> {
        match (self.plan, other.plan) {
            (None, None) => Ok(Some(Ordering::Equal)),
            (None, Some(_)) => Ok(Some(Ordering::Less)),
            (Some(_), None) => Ok(Some(Ordering::Greater)),
            (Some(a), Some(b)) if (a.identity)() == (b.identity)() => {
                a.compare(self.data, other.data)
            }
            _ => Ok(None),
        }
    }
    /// Validate every contained capability and floating key restriction before key use.
    pub fn key_hash(&self) -> NodeResult<u64> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        if let Some(plan) = self.plan {
            (plan.identity)().hash(&mut hash);
            plan.hash(self.data)?.hash(&mut hash);
        }
        Ok(hash.finish())
    }
}

/// Read one compiled eight-byte scalar or frame header.
pub fn number(data: &[u8]) -> u64 {
    u64::from_le_bytes(
        data.try_into()
            .unwrap_or_else(|_| unreachable!("compiled scalar layout")),
    )
}
/// Decode a compiled frame or member index on the target architecture.
pub fn index(value: u64) -> usize {
    usize::try_from(value).unwrap_or_else(|_| unreachable!("compiled layout index"))
}
/// Borrow complete child frames at cold capture boundaries.
#[derive(Debug)]
pub struct Frames<'a>(pub AnyView<'a>);
impl<'a> Iterator for Frames<'a> {
    type Item = AnyView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let size = index(number(&self.0[..8]));
        let value = self.0.slice(8..8 + size);
        self.0 = self.0.slice(8 + size..self.0.len());
        Some(value)
    }
}
impl AnyPlan {
    /// Execute equality selected during compilation.
    pub fn equal(&self, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<bool> {
        (self.execution.equal)(self, a, b)
    }
    /// Execute partial order selected during compilation.
    pub fn compare(&self, a: AnyView<'_>, b: AnyView<'_>) -> NodeResult<Option<Ordering>> {
        (self.execution.compare)(self, a, b)
    }
    /// Execute hash selected during compilation.
    pub fn hash(&self, data: AnyView<'_>) -> NodeResult<u64> {
        (self.execution.hash)(self, data)
    }
}

/// A statically selected encoded sink, either preflight counting or reserved storage.
pub trait AnyBuffer {
    /// Current complete encoded width.
    fn len(&self) -> usize;
    /// Whether no encoded data has been written.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Append exact data; storage callers preflight capacity before entering the codec.
    fn write(&mut self, bytes: &[u8]);
    /// Preserve provider copying; sizing validates without executing the copy.
    fn native(&mut self, data: &[u8], copy: NativeCopy, check: NativeCheck);
    /// Append a typed descriptor header into the independently owned metadata.
    fn descriptor(&mut self, plan: Option<&'static AnyPlan>);
    /// Finish a frame's width at its reserved header.
    fn width(&mut self, index: usize, width: usize);
    /// Retain a complete child frame without temporary buffers.
    fn frame(&mut self, encode: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        let start = self.len();
        self.write(&[0; 8]);
        encode(self);
        self.width(start, self.len() - start - 8);
    }
}
/// Preflight sink used before mutating an owning destination.
#[derive(Debug, Default)]
pub struct AnyWidth(pub usize, Option<Box<NodeError>>);
impl AnyWidth {
    /// Complete fallible provider preflight before publication.
    pub fn finish(self) -> NodeResult<usize> {
        self.1.map_or(Ok(self.0), Err)
    }
}
impl AnyBuffer for AnyWidth {
    fn native(&mut self, data: &[u8], _: NativeCopy, check: NativeCheck) {
        self.1 = self.1.take().or_else(|| check(data, data.len()).err());
        self.write(data);
    }
    fn len(&self) -> usize {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }
    fn width(&mut self, _: usize, _: usize) {}
    fn descriptor(&mut self, _: Option<&'static AnyPlan>) {
        self.0 += 8;
    }
}
