//! Reusable typed columns and prepared list element layouts.
use hgl_columns::Scalar;
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, NodeResult, ScalarType, Time};

/// An element's positions are interpreted only by its compile-time value marker.
pub type ListData = Vec<Vec<usize>>;
/// Upper bounds on slots required before an infallible installation.
#[derive(Debug, Default)]
pub struct Capacity([usize; 12]);
impl Capacity {
    /// Count one statically typed primitive slot.
    pub fn scalar<T: Scalar>(&mut self) {
        let count = &mut self.0[kind::<T>()];
        *count = count.saturating_add(1);
    }
    /// Count one list descriptor.
    pub fn list(&mut self) {
        self.lists(1);
    }
    /// Count a known number of list descriptors, saturating into reservation failure.
    pub fn lists(&mut self, count: usize) {
        self.0[11] = self.0[11].saturating_add(count);
    }
}
/// Private payloads remain in type-specific columns; all handles are stable indices.
#[derive(Debug, Default)]
pub struct Columns {
    values: hgl_columns::Columns,
    free: [Vec<usize>; 11],
    lists: Vec<ListData>,
    free_lists: Vec<usize>,
}
impl Columns {
    /// Obtain every capacity needed by installation and subsequent reclamation.
    pub fn reserve(&mut self, capacity: &Capacity) -> NodeResult {
        self.reserve_scalar::<bool>(capacity)?;
        self.reserve_scalar::<i64>(capacity)?;
        self.reserve_scalar::<f64>(capacity)?;
        self.reserve_scalar::<String>(capacity)?;
        self.reserve_scalar::<Date>(capacity)?;
        self.reserve_scalar::<Time>(capacity)?;
        self.reserve_scalar::<EngineTime>(capacity)?;
        self.reserve_scalar::<EngineDelta>(capacity)?;
        self.reserve_scalar::<hgl_types::CivilDateTime>(capacity)?;
        self.reserve_scalar::<hgl_types::ZoneId>(capacity)?;
        self.reserve_scalar::<hgl_types::ZonedDateTime>(capacity)?;
        let extra = capacity.0[11].saturating_sub(self.free_lists.len());
        self.lists
            .try_reserve(extra)
            .map_err(|error| NodeError::new(error.to_string()))?;
        self.free_lists
            .try_reserve(
                self.lists
                    .len()
                    .saturating_add(extra)
                    .saturating_sub(self.free_lists.len()),
            )
            .map_err(|error| NodeError::new(error.to_string()))
    }
    fn reserve_scalar<T: Scalar>(&mut self, capacity: &Capacity) -> NodeResult {
        let free = &mut self.free[kind::<T>()];
        let values = T::column_mut(&mut self.values);
        let extra = capacity.0[kind::<T>()].saturating_sub(free.len());
        values
            .try_reserve(extra)
            .map_err(|error| NodeError::new(error.to_string()))?;
        free.try_reserve(
            values
                .len()
                .saturating_add(extra)
                .saturating_sub(free.len()),
        )
        .map_err(|error| NodeError::new(error.to_string()))
    }
    /// Install a retained primitive after reservation.
    pub fn insert<T: Scalar>(&mut self, value: T) -> usize {
        let values = T::column_mut(&mut self.values);
        if let Some(slot) = self.free[kind::<T>()].pop() {
            values[slot] = value;
            slot
        } else {
            let slot = values.len();
            values.push(value);
            slot
        }
    }
    /// Read the scalar column selected when the caller was compiled.
    pub fn scalar<T: Scalar>(&self, slot: usize) -> &T {
        &T::column(&self.values)[slot]
    }
    /// Move a fully retained scalar into an existing slot.
    pub fn replace<T: Scalar>(&mut self, slot: usize, value: T) {
        T::column_mut(&mut self.values)[slot] = value;
    }
    /// Drop a scalar and make its slot available without allocating.
    pub fn release<T: Scalar>(&mut self, slot: usize) {
        self.replace(slot, T::default());
        self.free[kind::<T>()].push(slot);
    }
    /// Install a prepared list after reservation.
    pub fn insert_list(&mut self, value: ListData) -> usize {
        if let Some(slot) = self.free_lists.pop() {
            self.lists[slot] = value;
            slot
        } else {
            let slot = self.lists.len();
            self.lists.push(value);
            slot
        }
    }
    /// Inspect an already prepared list descriptor.
    pub fn list(&self, slot: usize) -> &ListData {
        &self.lists[slot]
    }
    /// Mutate an already prepared list descriptor.
    pub fn list_mut(&mut self, slot: usize) -> &mut ListData {
        &mut self.lists[slot]
    }
    /// Swap a stable root descriptor, returning its old elements for typed reclamation.
    pub fn replace_list(&mut self, slot: usize, value: ListData) -> ListData {
        std::mem::replace(&mut self.lists[slot], value)
    }
    /// Reclaim an empty descriptor after its descendants have been released.
    pub fn release_list(&mut self, slot: usize) {
        self.lists[slot].clear();
        self.free_lists.push(slot);
    }
    /// Allocated positions, including reusable ones, for bounded-storage validation.
    pub fn slot_counts(&self) -> (usize, usize) {
        (
            count::<bool>(&self.values)
                + count::<i64>(&self.values)
                + count::<f64>(&self.values)
                + count::<String>(&self.values)
                + count::<Date>(&self.values)
                + count::<Time>(&self.values)
                + count::<EngineTime>(&self.values)
                + count::<EngineDelta>(&self.values)
                + count::<hgl_types::CivilDateTime>(&self.values)
                + count::<hgl_types::ZoneId>(&self.values)
                + count::<hgl_types::ZonedDateTime>(&self.values),
            self.lists.len(),
        )
    }
}

fn kind<T: Scalar>() -> usize {
    match T::TYPE {
        ScalarType::Bool => 0,
        ScalarType::I64 => 1,
        ScalarType::F64 => 2,
        ScalarType::Text => 3,
        ScalarType::Date => 4,
        ScalarType::Time => 5,
        ScalarType::DateTime => 6,
        ScalarType::Duration => 7,
        ScalarType::CivilDateTime => 8,
        ScalarType::TimeZone => 9,
        ScalarType::ZonedDateTime => 10,
    }
}

/// Preallocated list layouts, consumed in generated declaration order at commit.
#[derive(Debug, Default)]
pub struct Layouts {
    values: Vec<ListData>,
    position: usize,
}
impl Layouts {
    /// Discard prior preparation while retaining reusable scratch capacity.
    pub fn reset(&mut self) {
        self.values.clear();
        self.position = 0;
    }

    /// Retain a complete layout before touching live values.
    pub fn push(&mut self, value: ListData) -> NodeResult {
        self.values
            .try_reserve(1)
            .map_err(|error| NodeError::new(error.to_string()))?;
        self.values.push(value);
        Ok(())
    }
    /// Move the next prepared layout without allocation.
    pub fn take(&mut self) -> ListData {
        let value = std::mem::take(&mut self.values[self.position]);
        self.position += 1;
        value
    }
}

fn count<T: Scalar>(columns: &hgl_columns::Columns) -> usize {
    T::column(columns).len()
}
