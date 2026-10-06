use crate::keys::{Keys, Table, hash};
use hgl_types::{NodeError, NodeResult};
impl Keys {
    /// Retain complete component tokens in a statically selected exact key domain.
    pub fn prepare_composite(&mut self, domain: usize, parts: &[i64]) -> NodeResult {
        if domain >= self.composites.len() {
            self.composites
                .try_reserve(domain + 1 - self.composites.len())
                .map_err(|e| NodeError::new(e.to_string()))?;
            self.composites.resize_with(domain + 1, Table::default);
        }
        let table = &mut self.composites[domain];
        if find(table, parts).is_some() {
            return Ok(());
        }
        let mut retained = Vec::new();
        retained
            .try_reserve(parts.len())
            .map_err(|e| NodeError::new(e.to_string()))?;
        retained.extend_from_slice(parts);
        table.prepare(retained)
    }
    /// Resolve an already prepared complete identity without allocating.
    pub fn composite_id(&self, domain: usize, parts: &[i64]) -> NodeResult<i64> {
        self.composites
            .get(domain)
            .and_then(|table| find(table, parts))
            .ok_or_else(|| NodeError::new("composite collection key was not prepared before start"))
    }
    /// Borrow all component identities of one exact prepared key.
    pub fn composite_parts(&self, domain: usize, id: i64) -> NodeResult<&[i64]> {
        self.composites
            .get(domain)
            .ok_or_else(|| NodeError::new("unknown composite key domain"))?
            .get(id)
            .map(Vec::as_slice)
    }
    /// Enumerate only the keys prepared in this statically selected domain.
    pub fn composite_ids(&self, domain: usize) -> &[i64] {
        self.composites
            .get(domain)
            .map_or(&[], |table| table.ids.as_slice())
    }
}
fn find(table: &Table<Vec<i64>>, parts: &[i64]) -> Option<i64> {
    table
        .buckets
        .get(&hash(&parts))?
        .iter()
        .copied()
        .find(|&id| {
            table.values[usize::try_from(id).unwrap_or_else(|_| unreachable!("prepared ID"))]
                == parts
        })
}
#[cfg(test)]
mod tests {
    use super::{Table, find, hash};
    #[test]
    fn collision_candidates_require_every_component() -> hgl_types::NodeResult {
        let mut table = Table::default();
        table.prepare(vec![1, 7, 0, 0])?;
        table.prepare(vec![1, 7, 1, 0])?;
        table
            .buckets
            .insert(hash(&[1_i64, 7, 0, 0].as_slice()), vec![1, 0]);
        assert_eq!(find(&table, &[1, 7, 0, 0]), Some(0));
        Ok(())
    }
}
