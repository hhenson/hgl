//! Exact hash collisions, ownership copies and finite indexed lookup costs.
use hgl_store::map_values::{Index, MapValue, hash};
use hgl_types::NodeResult;

#[test]
fn collisions_unknown_keys_and_replacement_keep_exact_identity() -> NodeResult {
    let original = MapValue::new(vec![("one", 1), ("two", 2)], |_| Ok(7))?;
    assert_eq!(
        original
            .index()
            .find(7, |position| Ok(original[position].0 == "two"))?,
        Some(1)
    );
    assert_eq!(
        original
            .index()
            .find(7, |position| Ok(original[position].0 == "absent"))?,
        None
    );
    assert_eq!(
        original
            .index()
            .find(9, |_| unreachable!("different hash"))?,
        None
    );
    let mut independent = Index::default();
    independent.prepare(4)?;
    independent.copy(original.index());
    let next = MapValue::new(vec![("three", 3)], |_| Ok(11))?;
    independent.copy(next.index());
    assert_eq!(independent.find(7, |_| unreachable!("removed hash"))?, None);
    assert_eq!(
        original
            .index()
            .find(7, |position| Ok(original[position].0 == "one"))?,
        Some(0)
    );
    let retained = next.index().retained()?;
    let moved = MapValue::from_parts(next.into_parts().0, retained);
    assert_eq!(
        moved
            .index()
            .find(11, |position| Ok(moved[position].0 == "three"))?,
        Some(0)
    );
    Ok(())
}

#[test]
fn prepared_index_copy_and_lookup_allocate_nothing() -> NodeResult {
    let source = Index::from_hashes((0..256_u64).map(|key| hash(&key)).collect())?;
    let mut destination = Index::default();
    destination.prepare(512)?;
    let (result, allocations) = hgl_alloc_count::count_in(|| -> NodeResult {
        for _ in 0..4 {
            destination.copy(&source);
            for key in 0..256_u64 {
                assert_eq!(
                    destination.find(hash(&key), |position| Ok(position as u64 == key))?,
                    Some(usize::try_from(key).unwrap())
                );
            }
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    Ok(())
}

#[test]
fn n_2n_4n_lookups_reuse_hashes_instead_of_rescanning_keys() -> NodeResult {
    for count in [256_usize, 512, 1024] {
        let mut hash_calls = 0;
        let values = MapValue::new((0..count).collect(), |key| {
            hash_calls += 1;
            Ok(hash(key))
        })?;
        assert_eq!(hash_calls, count);
        let mut comparisons = 0;
        for key in 0..count {
            assert_eq!(
                values.index().find(hash(&key), |position| {
                    comparisons += 1;
                    Ok(values[position] == key)
                })?,
                Some(key)
            );
        }
        assert_eq!(comparisons, count);
        assert_eq!(hash_calls, count);
    }
    Ok(())
}

#[test]
fn failed_hashing_preserves_its_origin_code() {
    let failure = MapValue::new(vec![1_u64], |_| {
        Err(hgl_types::NodeError::coded(
            "key lacks hashing",
            "value.capability",
        ))
    })
    .unwrap_err();
    assert_eq!(failure.code, Some("value.capability"));
}

#[test]
fn reprovision_preserves_occupied_buckets_and_does_not_shrink() -> NodeResult {
    let mut index = Index::from_hashes(vec![7, 7, 15])?;
    for capacity in [4, 32, 0, 1] {
        index.prepare(capacity)?;
        for position in 0..3 {
            let hash = if position == 2 { 15 } else { 7 };
            assert_eq!(
                index.find(hash, |found| Ok(found == position))?,
                Some(position)
            );
        }
        assert_eq!(index.find(7, |_| Ok(false))?, None);
        assert_eq!(index.find(99, |_| unreachable!())?, None);
    }
    Ok(())
}
