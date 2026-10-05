# Card: hgl-nominal-batch

Immutable finite nominal definition ownership, independent of source syntax and
runtime representation. No dependencies; budget 160 source lines.

`Definition<I,T>::new(identity,fields,optional)` owns declaration-ordered fields
and presence positions. Read-only `identity`, `fields` and `optional` accessors
preserve those records. `Batch<I,T>::new(identity,definitions)` sorts definitions,
rejects duplicate exact identities and requires the selected root to exist.
`Batch::reference(identity)` represents an edge whose complete definition belongs
to its containing batch. `identity`, `definitions`, and fallible `definition`
provide read-only access; absent edge definitions are errors, never empty structs.
Batch equality and ordering compare exact identity only. Definition contents are
retained construction metadata and do not change nominal identity.

The source semantic resolver checks recursive edge closure and finite generic
specializations before forming complete batches. Each standalone checked root
carries all reachable definitions, including silent-input roots. No registry is
consulted during evaluation. Generic parameters separate identity and field
storage from Ty without introducing a cyclic crate dependency.
