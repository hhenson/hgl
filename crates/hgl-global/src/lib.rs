//! Run-owned ordinary values, with names and types resolved before hooks run.
use std::{collections::HashMap, marker::PhantomData};

use hgl_columns::{Columns, Scalar};
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, NodeResult, ScalarType, Time};

/// An entry in one owning global state; its type is selected at compilation.
#[derive(Debug, Clone)]
pub struct Global<T: Scalar> {
    entry: usize,
    slot: usize,
    payload: PhantomData<T>,
}
impl<T: Scalar> Copy for Global<T> {}

/// Ordinary scalar entries shared by every graph scope of one run.
#[derive(Debug, Default)]
pub struct GlobalState {
    provisioned: bool,
    entries: HashMap<String, (ScalarType, usize, usize)>,
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
    pub fn bind<T: Scalar>(&mut self, key: &str) -> Result<Global<T>, Box<NodeError>> {
        if !self.provisioned {
            return Err(NodeError::new(format!(
                "global_state: unprovisioned store for key {key:?}"
            )));
        }
        let (entry, slot) = if let Some(&(ty, entry, slot)) = self.entries.get(key) {
            if ty != T::TYPE {
                return Err(NodeError::new(format!(
                    "global_state: type conflict for key {key:?}"
                )));
            }
            (entry, slot)
        } else {
            let entry = self.keys.len();
            let column = T::column_mut(&mut self.values);
            let slot = column.len();
            column.push(T::default());
            self.keys.push(key.to_owned());
            self.present.push(false);
            self.entries.insert(key.to_owned(), (T::TYPE, entry, slot));
            (entry, slot)
        };
        Ok(Global {
            entry,
            slot,
            payload: PhantomData,
        })
    }
    /// Reconcile a description's type during graph construction, never in a hook.
    pub fn prepare(&mut self, key: &str, ty: ScalarType) -> NodeResult {
        match ty {
            ScalarType::Bool => self.bind::<bool>(key).map(|_| ()),
            ScalarType::I64 => self.bind::<i64>(key).map(|_| ()),
            ScalarType::F64 => self.bind::<f64>(key).map(|_| ()),
            ScalarType::Text => self.bind::<String>(key).map(|_| ()),
            ScalarType::Date => self.bind::<Date>(key).map(|_| ()),
            ScalarType::Time => self.bind::<Time>(key).map(|_| ()),
            ScalarType::DateTime => self.bind::<EngineTime>(key).map(|_| ()),
            ScalarType::Duration => self.bind::<EngineDelta>(key).map(|_| ()),
        }
    }
    /// Copy a present value using its prepared typed slot; no name/type lookup.
    pub fn get<T: Scalar>(&self, handle: Global<T>) -> Result<T, Box<NodeError>> {
        if !self.present[handle.entry] {
            return Err(NodeError::new(format!(
                "global_state: missing value for key {:?}",
                self.keys[handle.entry]
            )));
        }
        T::column(&self.values)[handle.slot].try_clone()
    }
    /// Copy successfully before replacing an entry; no publication or scheduling.
    pub fn set<T: Scalar>(&mut self, handle: Global<T>, value: &T) -> NodeResult {
        let owned = value.try_clone()?;
        T::column_mut(&mut self.values)[handle.slot] = owned;
        self.present[handle.entry] = true;
        Ok(())
    }
}
