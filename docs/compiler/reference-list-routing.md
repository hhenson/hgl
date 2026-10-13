# Fixed REF-list routing

The Rust target executes the Spec's guarded `route3<T>` for fixed
`list<ref<T>,N>` node inputs. Graph sequences assemble uniform temporal
connections into opaque REF carriers. Ordinary ports capture stable endpoint
identity; existing REF producers retain their carrier so a selected entry can
retarget while the numeric index is silent. REF aliases forward the same
identity. Payload ticks flow to ordinary followers without re-emitting a REF.

Dynamic endpoint indices evaluate once per expression, convert i64 fallibly,
and check the declared range before selecting a child. Short-circuit guards
keep invalid and out-of-range indices from executing the index expression.
Equal designations are silent; equal values at distinct endpoints still rebind.
Graph nominal fields and constant fixed-list projections below the selected REF
retain their existing child identity and retargeting behavior.

`reference_list_routing.rs` executes an immutable Std
`c8d738e0387fa455d9d3250b2026b183e528fc9e` snapshot: five named groups and
seven evaluations cover guards, invalid targets, live payload ticks, selected
and sibling entry retargets, equal and duplicate identity, bytes and sparse maps.
The installed production libraries remain Std `d798c88` and Spec `69a2f8c`.
The adjacent provenance manifest records full revisions and source hashes.

Six local groups and seven evaluations cover graph child projections, nominal
assembly and conditional owner entries, selected REF aliases, and ordinary
contextual list/configuration controls. Separate compiled executions assert
errors for -1, N and i64::MAX and success for 0 and N-1. Static controls reject
mixed entries and type/width mismatches. The finite preparation test retains
rejection of genuinely unmapped unbounded selection and separately verifies
carrier endpoints versus designated payload targets.

Both fixture runners use the active CountingAllocator beginning with startup
and the first successful cycle; each successful evaluation cycle and cleanup
must allocate zero. These guarantees apply to the existing finite prepared
execution profile. Ordinary REF-list publishers, unbounded selection, dynamic
graph projections, and node traversal beneath an opaque REF remain explicit
backend/admission gaps. No dictionary selector or ordinary REF storage is added.
