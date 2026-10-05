# Card: hgl-rust-layouts

Emit checked Rust type spellings and nominal ordinary storage layouts. Uses
`hgl-source` and `hgl-rust-ir`; budget 400 source lines. No runtime execution or
third-party dependencies. Expression and statement emission stays in
`hgl-rust-values`; resume control flow stays in `hgl-rust-generators`.

Public surface: `rust_type`, `scalar_type`, `owned_type`, `global_type`,
`global_schema`, `global_markers`. These consume checked concrete types and
plans, never source syntax. The marker collector traverses lifecycle hooks,
ordinary helper expressions and generator bodies, retaining canonical nominal
specializations. Marker implementations stage recursive ordinary storage and
retain the exact existing runtime prepare/install/release contracts. Generated
names encode complete specialization identities without hashing collisions.

Acceptance: unchanged emitted ordinary scalar/list/struct programs and global
metadata; generator locals containing nested ordinary aggregates reference the
same independently owned layouts as existing hook locals and configurations.

Structural delta layouts use private synthetic required fields made of ordinary
lists and child delta owners. Canonical delta origin identities are distinct
from user nominal identities; sparse field vectors have zero or one child.
Homogeneous sparse shapes use paired key/payload vectors; sets use addition and
removal vectors. Generated GlobalValue implementations retain the existing
fallible staging and infallible move-commit contract.

`delta_type(&Ty) -> Ty` forms the checked ordinary publication type.
`delta_storage(&Ty) -> Ty` exposes the backend-only synthetic owning layout,
whose NUL-prefixed canonical name cannot alias a source nominal declaration.
These helpers perform compiler-time shape selection, never runtime dispatch.

## Temporal scalar preparation

Scalar spelling helpers may delegate to hgl-rust-scalars with existing public
paths retained. Extend exact typed layouts for the three temporal scalar leaves;
no provider object or recipe is stored in a published value. Budget unchanged.

Declared enum source types retain nominal identity through generated i64-backed
GlobalValue markers. Enum publication uses one prepared whole-value slot without
structural children. Checked harness capture retains the original declaration
and assigned member number; it never substitutes an ordinary integer.

May use hgl-rust-enums. Expose whole_payload(&Ty) -> Option<&Ty> for an atomic
payload or a declared enum scalar using a prepared complete-value endpoint.

Nominal markers also delegate additive PreparedValue emission to
hgl-rust-prepared-values, retaining exact identity in independently prepared slots.

May use hgl-rust-structs for concrete nominal GlobalValue emission. Optional
positions become Option payloads and Optional field markers; marker collection
retains exact nominal identity and existing declaration indices. Tuple and
delta storage schemas remain required-field forms.

global_types(plan)->BTreeMap<String,Ty> collects complete immutable nominal batches
before emitting any member. Every reachable recursive definition is collected once,
including silent-input roots. Recursive owned representations are named tuple
structs; edge schemas use exact RecursiveReference identities instead of expanding
the target. Bare internal edge metadata supplies a name only and is never emitted
as an empty struct definition.

Family types map to the fixed tagged nominal storage emitted by hgl-rust-families.
The collector visits every declared concrete member, including unobserved members.
Reexports family_storage, family_member and family_coerce; their ownership contract
is specified by the hgl-rust-families card.

Ordinary Set/Map layouts use exact schema markers emitted by hgl-rust-collections with independently retained Vec members or key/value pairs. Reachable pair and nested payload layouts are collected with other ordinary descendants.
Growing delta storage has index, child-publication and removed-index vectors, retaining its exact synthetic nominal origin.
