# Card: hgl-member-table

Status: accepted.

Membership storage with a cold finite domain. May use the standard library only.
Budget: 220 source lines. This crate owns occupancy, not key identity, endpoint
lifetimes, publication or projection policy.

Public `Table<T>: Debug + Default` provides `prepare(&[i64])`, `prepared()`,
`get(i64)`, `contains_key(i64)`, `len()`, `is_empty()`, `insert(i64,T)`,
`insert_initial(i64,T)`, `remove(i64)`, `pop_first()`, `clear()`, `item(usize)`,
`dynamic_after(Option<i64>)`, `iter()`, `keys()`, `values()` and consuming `into_values()`.

Unprepared tables retain ordered dynamic BTree storage; `dynamic_after` advances
a key cursor without rescanning earlier entries. Preparing an empty table
allocates a sorted immutable domain and dense occupancy indices. Unknown-domain
insertion is an internal construction error. Insert, replace, remove and clear
reuse capacity; membership iteration visits occupied slots only. Prepared order
is unspecified. `item` addresses a dense occupied position without scanning
absent domain keys. Removal updates the moved entry's dense position.

Acceptance: middle removal, replacement, reinsertion and clear preserve occupied
entries with a large mostly unused domain. Full publication allocation tests
live in hgl-store and include the first membership change.
