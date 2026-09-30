//! Owned scalar storage borrowed by the HGL replay and record capabilities.
use hgl_kernel::{NodeError, NodeResult};
use hgl_store::Scalar;
use hgl_types::{Date, EngineDelta, EngineTime, ScalarType, Time};

type Result<T> = std::result::Result<T, Box<NodeError>>;

/// Scalars whose independent copies report allocation failures as node errors.
pub trait BufferScalar: Scalar {
    /// Return an independent delta or a translated allocation error.
    fn copy_delta(&self) -> Result<Self>;
}
impl BufferScalar for bool {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for i64 {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for f64 {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for Date {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for Time {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for EngineTime {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for EngineDelta {
    fn copy_delta(&self) -> Result<Self> {
        Ok(*self)
    }
}
impl BufferScalar for String {
    fn copy_delta(&self) -> Result<Self> {
        let mut owned = Self::new();
        owned
            .try_reserve(self.len())
            .map_err(|e| NodeError::new(e.to_string()))?;
        owned.push_str(self);
        Ok(owned)
    }
}

/// Immutable typed dense slots; the HGL source owns the replay cursor.
#[derive(Debug)]
pub struct ReplayInput<T> {
    slots: Vec<Option<T>>,
    length: i64,
}
impl<T: BufferScalar> ReplayInput<T> {
    /// Validate dense length and its final timestamp before graph start.
    pub fn new(slots: Vec<Option<T>>, start: EngineTime) -> Result<Self> {
        let length = i64::try_from(slots.len()).map_err(|error| {
            NodeError::new(format!("replay_input: length does not fit i64: {error}"))
        })?;
        if !(EngineTime::MIN_START..EngineTime::MAX_END).contains(&start) {
            return Err(NodeError::new("replay_input: invalid run start"));
        }
        if length > 0
            && start
                .checked_add(EngineDelta::from_micros(length - 1))
                .is_none_or(|last| last >= EngineTime::MAX_END)
        {
            return Err(NodeError::new(
                "replay_input: final dense instant is outside the run time domain",
            ));
        }
        Ok(Self { slots, length })
    }
    /// Count every slot, including absent ones.
    pub fn length(&self) -> i64 {
        self.length
    }
    /// Test slot presence independently of its payload.
    pub fn has_tick(&self, index: i64) -> Result<bool> {
        Ok(self.slot(index)?.is_some())
    }
    /// Return an independently owned scalar delta at a present position.
    pub fn delta_at(&self, index: i64) -> Result<T> {
        self.slot(index)?
            .as_ref()
            .ok_or_else(|| NodeError::new("replay_input: slot has no tick"))?
            .copy_delta()
    }
    fn slot(&self, index: i64) -> Result<&Option<T>> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.slots.get(i))
            .ok_or_else(|| NodeError::new("replay_input: index out of range"))
    }
}

/// A single writer's unbegun or begun recording with independent scalar ticks.
#[derive(Debug)]
pub struct Capture<T> {
    begun: bool,
    last: Option<EngineTime>,
    ticks: Vec<(EngineTime, T)>,
}
impl<T: BufferScalar> Default for Capture<T> {
    fn default() -> Self {
        Self::new()
    }
}
impl<T: BufferScalar> Capture<T> {
    /// Allocate logical storage without beginning the HGL recording.
    pub fn new() -> Self {
        Self {
            begun: false,
            last: None,
            ticks: Vec::new(),
        }
    }
    /// Begin once in the HGL start hook, including when no tick will arrive.
    pub fn begin(&mut self) -> NodeResult {
        if self.begun {
            return Err(NodeError::new("capture: already begun"));
        }
        self.begun = true;
        Ok(())
    }
    /// Copy a current evaluation's scalar tick after ordered validation.
    pub fn append(
        &mut self,
        time: EngineTime,
        delta: &T,
        evaluation_time: EngineTime,
    ) -> NodeResult {
        if !self.begun {
            return Err(NodeError::new("capture: not begun"));
        }
        if time != evaluation_time {
            return Err(NodeError::new("capture: timestamp is not evaluation time"));
        }
        if self.last.is_some_and(|last| time <= last) {
            return Err(NodeError::new("capture: timestamp did not advance"));
        }
        let owned = delta.copy_delta()?;
        self.ticks
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.ticks.push((time, owned));
        self.last = Some(time);
        Ok(())
    }
    /// Transfer owned ticks after stop; an unbegun recording is not empty success.
    pub fn take_ticks(&mut self) -> Result<Vec<(EngineTime, T)>> {
        if !self.begun {
            return Err(NodeError::new("capture: not begun"));
        }
        Ok(std::mem::take(&mut self.ticks))
    }
}

/// The storage capability required by a compiled node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferRole {
    /// Immutable dense scalar input.
    ReplayInput,
    /// A single-writer output recording.
    Capture,
}
impl BufferRole {
    fn name(self) -> &'static str {
        match self {
            Self::ReplayInput => "replay_input",
            Self::Capture => "capture",
        }
    }
}
/// Construction metadata for one capability-bearing node.
#[derive(Debug, Clone, Copy)]
pub struct BufferRequirement {
    /// Node identity within the fresh graph.
    pub node: u32,
    /// Required capability role.
    pub role: BufferRole,
    /// Exact contextual payload type.
    pub scalar: ScalarType,
}
/// A typed construction binding, never looked up by the running node.
#[derive(Debug)]
pub struct BufferBinding {
    run: u64,
    node: u32,
    buffer: u64,
    role: BufferRole,
    scalar: ScalarType,
}
impl BufferBinding {
    /// Describe an immutable typed input in a construction-local run.
    pub fn replay<T: Scalar>(run: u64, node: u32, buffer: u64) -> Self {
        Self {
            run,
            node,
            buffer,
            role: BufferRole::ReplayInput,
            scalar: T::TYPE,
        }
    }
    /// Describe a typed capture's sole writer in a construction-local run.
    pub fn capture<T: Scalar>(run: u64, node: u32, buffer: u64) -> Self {
        Self {
            run,
            node,
            buffer,
            role: BufferRole::Capture,
            scalar: T::TYPE,
        }
    }
}
/// Check every configured capability before any node can start.
pub fn validate_bindings(
    run: u64,
    required: &[BufferRequirement],
    actual: &[BufferBinding],
) -> NodeResult {
    for (i, binding) in actual.iter().enumerate() {
        let capability = binding.role.name();
        if binding.run != run {
            return Err(NodeError::new(format!(
                "{capability}: binding belongs to another run"
            )));
        }
        let requirement = required
            .iter()
            .find(|r| r.node == binding.node)
            .ok_or_else(|| {
                NodeError::new(format!("{capability}: binding names an unknown node"))
            })?;
        if requirement.role != binding.role || requirement.scalar != binding.scalar {
            return Err(NodeError::new(format!(
                "{}: binding role or type mismatch",
                requirement.role.name()
            )));
        }
        if actual[..i].iter().any(|b| b.node == binding.node) {
            return Err(NodeError::new(format!(
                "{capability}: node has duplicate bindings"
            )));
        }
        if binding.role == BufferRole::Capture
            && actual[..i]
                .iter()
                .any(|b| b.role == BufferRole::Capture && b.buffer == binding.buffer)
        {
            return Err(NodeError::new("capture: buffer has multiple writers"));
        }
    }
    for (i, requirement) in required.iter().enumerate() {
        if required[..i].iter().any(|r| r.node == requirement.node) {
            return Err(NodeError::new(format!(
                "{}: node has duplicate requirements",
                requirement.role.name()
            )));
        }
        if !actual.iter().any(|b| b.node == requirement.node) {
            return Err(NodeError::new(format!(
                "{}: missing binding",
                requirement.role.name()
            )));
        }
    }
    Ok(())
}
