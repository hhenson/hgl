//! Run-owned ordinary values, with names and types resolved before hooks run.
pub use hgl_global_value::{Columns, GlobalValue, ValueSlot};
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::{collections::HashMap, fmt::Debug};

/// An entry in one owning global state; its type is selected at compilation.
pub struct Global<T: GlobalValue> {
    entry: usize,
    slot: ValueSlot<T>,
}
impl<T: GlobalValue> Copy for Global<T> {}
impl<T: GlobalValue> Clone for Global<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: GlobalValue> Debug for Global<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Global")
            .field("entry", &self.entry)
            .field("slot", &self.slot)
            .finish()
    }
}

/// Ordinary entries shared by every graph scope of one run.
#[derive(Debug, Default)]
pub struct GlobalState {
    provisioned: bool,
    entries: HashMap<String, (OrdinaryType, usize, Vec<usize>)>,
    keys: Vec<String>,
    present: Vec<bool>,
    values: Columns,
}
impl GlobalState {
    /// Enable owner-supplied storage, preserving any existing entries.
    pub fn provision(&mut self) {
        self.provisioned = true;
    }
    /// Whether this run has an owner-supplied store.
    pub fn provisioned(&self) -> bool {
        self.provisioned
    }
    /// Bind one exact type without inventing an initial value.
    pub fn bind<T: GlobalValue>(&mut self, key: &str) -> Result<Global<T>, Box<NodeError>> {
        self.prepare(key, T::schema())?;
        let (_, entry, layout) = &self.entries[key];
        Ok(Global {
            entry: *entry,
            slot: ValueSlot::bind(&mut layout.as_slice()),
        })
    }
    /// Reconcile a description's type during graph construction, never in a hook.
    pub fn prepare(&mut self, key: &str, ty: OrdinaryType) -> NodeResult {
        if !self.provisioned {
            return Err(NodeError::new(format!(
                "global_state: unprovisioned store for key {key:?}"
            )));
        }
        if let Some((existing, _, _)) = self.entries.get(key) {
            if *existing != ty {
                return Err(NodeError::new(format!(
                    "global_state: type conflict for key {key:?}"
                )));
            }
        } else {
            let entry = self.keys.len();
            let mut layout = Vec::new();
            hgl_global_value::allocate(&ty, &mut self.values, &mut layout);
            self.keys.push(key.to_owned());
            self.present.push(false);
            self.entries.insert(key.to_owned(), (ty, entry, layout));
        }
        Ok(())
    }
    /// Check root presence without copying payloads or resolving keys and types.
    pub fn borrow<T: GlobalValue>(
        &self,
        handle: Global<T>,
    ) -> Result<ValueSlot<T>, Box<NodeError>> {
        if !self.present[handle.entry] {
            return Err(NodeError::new(format!(
                "global_state: missing value for key {:?}",
                self.keys[handle.entry]
            )));
        }
        Ok(handle.slot)
    }
    /// Extract an owning value; aggregate hooks call borrow except at retention boundaries.
    pub fn get<T: GlobalValue>(&self, handle: Global<T>) -> Result<T::Value, Box<NodeError>> {
        self.read(self.borrow(handle)?)
    }
    /// Read a prepared field at an explicit owning retention boundary.
    pub fn read<T: GlobalValue>(&self, slot: ValueSlot<T>) -> Result<T::Value, Box<NodeError>> {
        slot.read(&self.values)
    }
    /// Retain the complete replacement before moving any of its leaves.
    pub fn write<T: GlobalValue>(&mut self, slot: ValueSlot<T>, value: &T::Value) -> NodeResult {
        let owned = T::retain(value)?;
        slot.commit(&mut self.values, owned);
        Ok(())
    }
    /// Copy successfully before replacing an entry; no publication or scheduling.
    pub fn set<T: GlobalValue>(&mut self, handle: Global<T>, value: &T::Value) -> NodeResult {
        self.write(handle.slot, value)?;
        self.present[handle.entry] = true;
        Ok(())
    }
}
