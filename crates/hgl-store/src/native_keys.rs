//! Native key domains use compiled indices and provider hash/equality, never opaque byte equality.
use crate::{
    Key, Keys,
    native_values::{Native, NativeType, NativeValue, NativeView},
};
use hgl_types::{NodeError, NodeResult};
use std::sync::{Mutex, OnceLock};
/// A generated marker caches a cold-resolved slot for one exact provider scalar.
pub trait NativeKeyDomain: std::fmt::Debug + 'static {
    /// Statically selected callback and contract owner.
    type Scalar: NativeType;
    /// Selected executable image and marker namespace for the scalar ABI.
    const MAPPING: (&'static str, &'static str);
    /// Process-lifetime cache, read without identity lookup during execution.
    fn slot() -> &'static OnceLock<usize>;
    /// Resolve this module's marker before construction, including observer-only domains.
    fn prepare() -> NodeResult
    where
        Self: Sized,
    {
        resolve::<Self>().map(|_| ())
    }
}
fn slot<D: NativeKeyDomain>() -> NodeResult<usize> {
    D::slot()
        .get()
        .copied()
        .ok_or_else(|| NodeError::new("native key domain was not prepared"))
}
struct Mapping {
    contract: &'static crate::native_values::Contract,
    image: (&'static str, &'static str),
}
static CONTRACTS: Mutex<Vec<Mapping>> = Mutex::new(Vec::new());
fn resolve<D: NativeKeyDomain>() -> NodeResult<usize> {
    if let Some(index) = D::slot().get() {
        return Ok(*index);
    }
    let contract = &D::Scalar::CONTRACT;
    let mut contracts = CONTRACTS
        .lock()
        .map_err(|_poisoned| NodeError::new("native key domain registry poisoned"))?;
    let index = if let Some(index) = contracts
        .iter()
        .position(|c| c.contract.canonical == contract.canonical)
    {
        let prior = contracts[index].contract;
        let mapping = contracts[index].image;
        if prior.provider != contract.provider
            || prior.fingerprint != contract.fingerprint
            || prior.capabilities != contract.capabilities
            || mapping != D::MAPPING
        {
            return Err(NodeError::new("incompatible native key domain contract"));
        }
        index
    } else {
        let index = contracts.len();
        contracts.push(Mapping {
            contract,
            image: D::MAPPING,
        });
        index
    };
    if D::slot().set(index).is_err() && D::slot().get() != Some(&index) {
        return Err(NodeError::new("native key domain cache identity mismatch"));
    }
    Ok(index)
}
#[derive(Debug, Default)]
pub(crate) struct Domain {
    values: Vec<Vec<u8>>,
    buckets: std::collections::HashMap<u64, Vec<i64>>,
    ids: Vec<i64>,
}
impl Domain {
    fn find<T: NativeType>(&self, value: NativeView<'_, T>) -> NodeResult<Option<i64>> {
        if let Some(bucket) = self.buckets.get(&T::hash(value.data())?) {
            for &id in bucket {
                if T::equal(
                    &self.values
                        [usize::try_from(id).unwrap_or_else(|_| unreachable!("prepared key"))],
                    value.data(),
                )? {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }
    fn get(&self, id: i64) -> NodeResult<&[u8]> {
        usize::try_from(id)
            .ok()
            .and_then(|id| self.values.get(id))
            .map(Vec::as_slice)
            .ok_or_else(|| NodeError::new("unknown prepared native key"))
    }
}
impl<T: NativeType, D: NativeKeyDomain<Scalar = T>> Native<T, D> {
    /// Borrow an already prepared identity without allocating an owning intermediate.
    pub fn key_view(keys: &Keys, id: i64) -> NodeResult<NativeView<'_, T>> {
        let table = keys
            .natives
            .get(slot::<D>()?)
            .ok_or_else(|| NodeError::new("native key domain was not prepared"))?;
        Ok(NativeView::<T>::from_provider(table.get(id)?))
    }
    /// Resolve a borrowed native key in its compiled domain without retaining data.
    pub fn key_view_id(keys: &Keys, value: NativeView<'_, T>) -> NodeResult<i64> {
        keys.natives
            .get(slot::<D>()?)
            .ok_or_else(|| NodeError::new("native key domain was not prepared"))?
            .find(value)?
            .ok_or_else(|| NodeError::new("native key was not prepared before start"))
    }
}
impl<T: NativeType, D: NativeKeyDomain<Scalar = T>> Key for Native<T, D> {
    fn prepare(keys: &mut Keys, value: &NativeValue<T>) -> NodeResult {
        if !T::CONTRACT.capabilities[2] || !T::CONTRACT.capabilities[3] {
            return Err(NodeError::new("native key requires equality and hash"));
        }
        let index = resolve::<D>()?;
        keys.natives
            .resize_with(keys.natives.len().max(index + 1), Domain::default);
        let table = &mut keys.natives[index];
        if table.find(value.view())?.is_some() {
            return Ok(());
        }
        let hash = T::hash(value.view().data())?;
        let value = value.retain()?.into_owned();
        let id = i64::try_from(table.values.len()).map_err(|e| NodeError::new(e.to_string()))?;
        table.buckets.entry(hash).or_default().push(id);
        table.values.push(value);
        table.ids.push(id);
        Ok(())
    }
    fn id(keys: &Keys, value: &NativeValue<T>) -> NodeResult<i64> {
        Self::key_view_id(keys, value.view())
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<NativeValue<T>> {
        Self::key_view(keys, id)?.retain()
    }
    fn ids(keys: &Keys) -> &[i64] {
        D::slot()
            .get()
            .and_then(|index| keys.natives.get(*index))
            .map_or(&[], |table| &table.ids)
    }
}
