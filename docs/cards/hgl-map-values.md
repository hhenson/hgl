# Card: hgl-map-values

The `map_values` module in hgl-store retains ordinary dictionary hash indexes.
It uses hgl-types and the standard library only, with no source, provider lookup,
unsafe code or third-party dependency. Its 180-line target is informational.

`hash` and `combine` hash statically selected primitive and composite key data.
`Index::{prepare,retained,copy,from_hashes,find}` owns cached hashes and reusable
open-addressing buckets. Exact key equality remains a typed caller callback for
collisions; valid unknown keys return None without domain registration or allocation.
Copying prepared metadata invokes no provider and allocates nothing.

`MapValue<T>::{new,from_parts,entries,index,into_parts}` retains an independent
ordered entry owner together with its index. Read-only slice and iteration access
preserve existing collection behavior. Source mutation cannot write entries
without replacing their index. Complete retention, move and replacement carry
matching metadata; source key/payload identity and ownership are unchanged.

Ordinary map code generation uses MapValue for owning values and retains the
existing one-descriptor List slot layout for typed descendants. GlobalValue
reads/retains copy index metadata with independently retained values. PreparedValue
allocation provisions buckets for the finite bound; native, between-arena and
within-arena copies update cached metadata with payload publication. Captures and
cold codecs reconstruct indexed owners through exact compiled key capabilities.
No provider hash/equality executes during infallible prepared copying.

Acceptance covers hash collisions, unknown identities, replacement, independent
retention, zero-allocation prepared copies/queries, and 256/512/1024-key equality
call counts. These are algorithm and allocation checks, not runtime parity timings.
