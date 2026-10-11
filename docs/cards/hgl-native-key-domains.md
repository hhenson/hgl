# Card: Native key domains

The native_keys module of hgl-store owns compiled Native<T, DOMAIN> key tables. Domain numbers and provider markers are selected before execution; Store has no dependency on compiler metadata.

Native::key_view and key_view_id borrow exact prepared keys; Key::prepare retains each new identity independently, Key::id resolves it, Key::value makes an explicit owning cold copy, and Key::ids exposes stable internal IDs. A hash table selects provider-hash collision buckets, then provider equality establishes identity. Distinct nominal domains remain separate even for identical bytes. Missing capabilities and provider errors propagate; no byte equality or invented ordering is substituted.

Native key publication and recording copy a borrowed key view after typed destination preflight. Successful prepared paths allocate nothing. Hash collisions may require linear scans within one bucket; no worst-case bound beyond the provider Eq/Hash contract is claimed. Controls cover collision identity, retained aliases, coded provider failures, and n/2n/4n/8n callback counts.

Initial module ceiling: native_keys 100 lines, within the unchanged Store aggregate.
