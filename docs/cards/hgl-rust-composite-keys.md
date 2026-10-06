# Card: hgl-rust-composite-keys

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `composite_keys` module of `hgl-rust` (`crates/hgl-rust/src/composite_keys.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Generate exact finite ordinary tuple/struct Key operations. Depends only on
hgl-source; budget 250 source lines. No runtime reflection, allocation, provider
lookup or dynamic type selection occurs in emitted lookup/transport operations.

- `composite(&Ty) -> bool` identifies admitted finite tuple/struct keys.
- `implementation(&Ty, usize, fn(&Ty)->String) -> String` emits a marker's Key
  implementation plus prepared `key_slot_id` and `copy_key` methods. The domain
  ordinal belongs only to the shared marker definition; independent plans never
  recompute it. Cold preparation provisions that ordinal in each run's Keys.
- `slot_id(&Ty, &str, fn(&Ty)->String) -> String` resolves a typed prepared value
  using stack component tokens, without first constructing an owning value.
- `copy(&Ty, &str, &str, fn(&Ty)->String) -> String` copies a prepared identity
  into reserved exact typed slots, borrowing scalar owning leaves.

Every component token and optional presence bit participates in equality.
Nested composite children use their statically selected domains; source nominal
identity belongs to the exact marker. Owning capture reconstructs a full value,
while prepared capture copies each component without an allocating intermediary.
Hash collisions never establish equality. Unprepared keys fail explicitly.
