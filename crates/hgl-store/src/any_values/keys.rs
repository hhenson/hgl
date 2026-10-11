//! Fallible contained capabilities in an independently retained cold key domain.
use crate::{Key, Keys};
use hgl_types::AnyValue;
use hgl_types::{NodeError, NodeResult};
#[derive(Debug, Default)]
pub(crate) struct AnyKeys {
    values: Vec<AnyValue>,
    buckets: std::collections::HashMap<u64, Vec<i64>>,
    ids: Vec<i64>,
}
impl AnyKeys {
    fn find(&self, value: &AnyValue, hash: u64) -> NodeResult<Option<i64>> {
        if let Some(bucket) = self.buckets.get(&hash) {
            for &id in bucket {
                if self.values[usize::try_from(id).unwrap_or_else(|_| unreachable!("key ID"))]
                    .equal(value)?
                {
                    return Ok(Some(id));
                }
            }
        }
        Ok(None)
    }
    fn get(&self, id: i64) -> NodeResult<&AnyValue> {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.values.get(i))
            .ok_or_else(|| NodeError::new("unknown prepared any key"))
    }
}
impl Key for crate::Any {
    fn prepare(keys: &mut Keys, value: &AnyValue) -> NodeResult {
        let hash = value.key_hash()?;
        let table = &mut keys.anys;
        if table.find(value, hash)?.is_some() {
            return Ok(());
        }
        let value = value.retain()?;
        table
            .values
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        table
            .ids
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        table
            .buckets
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        let id = i64::try_from(table.values.len()).map_err(|e| NodeError::new(e.to_string()))?;
        let bucket = table.buckets.entry(hash).or_default();
        bucket
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        bucket.push(id);
        table.values.push(value);
        table.ids.push(id);
        Ok(())
    }
    fn id(keys: &Keys, value: &AnyValue) -> NodeResult<i64> {
        keys.anys
            .find(value, value.key_hash()?)?
            .ok_or_else(|| NodeError::new("any key was not prepared before start"))
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<AnyValue> {
        keys.anys.get(id)?.retain()
    }
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&AnyValue) -> R) -> NodeResult<R> {
        Ok(visit(keys.anys.get(id)?))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.anys.ids
    }
}
