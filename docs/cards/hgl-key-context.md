# Card: hgl-key-context

The key_context module in hgl-eval assigns internal native identities during one
cold operation. It may use composite_keys, ir and value_eval; its initial module
budget is 80 lines within the unchanged hgl-eval ceiling.

Public surface: Context::key materializes one complete key with fallible provider
operations. A hash table selects an exact canonical-type/provider-hash bucket;
provider equality resolves collisions and each new identity retains its own
provider copy. Local IDs serve existing internal set/map indexing only and never
introduce observable ordering. Source-known identities continue deferring native
leaves, without executing providers. Collection construction shares a context
across members; publication validation shares it across slots and descendants.
Provider error codes remain intact during ordinary evaluation.

Controls cover equal meanings with different encodings, deliberate collisions,
distinct canonical types, hash/equality failures, and n/2n/4n/8n well-distributed
keys. Hash and equality callback counts scale linearly for that distribution.
Adversarial provider hash collisions may require linear work within a bucket;
no worst-case constant-time guarantee is claimed. This table never runs on the
per-tick path and does not add runtime names, type discovery or allocation there.
