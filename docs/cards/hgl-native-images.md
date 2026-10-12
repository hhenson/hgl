# Card: hgl-native-images

Status: implemented checked native type reachability and executable image union.

hgl-semantics::native_images uses library, checked ir, ir_visit, harness_ir and
hgl-source. Public scalars(Plan) and harness_scalars(&[Test]) collect canonical
native scalar contracts recursively through retained storage and expressions.
This includes ports, globals, configuration, wiring, ordinary layouts, generator
locals, nominal/recursive/family children and concrete values placed into Any.
select(Library, scalars, selected_images) returns only reached inert Image mappings, using the
validated canonical alias contract. It neither reads images nor calls providers.
Unused native declarations require no executable mapping.
Selected helper image rows take precedence. For type-only reachability, Ty::Native
retains its canonical identity and capabilities, while package validation proves
that every complete mapping for that identity has the same provider, fingerprint,
image path and marker. Selection chooses one complete canonical mapping, skipping
unmapped portable aliases, without replacing the source capability contract.

Plan.native_images and Suite.native_images preserve those mappings separately
from actual helper signatures. Suite selection and batching preserve candidates;
the backend selects again from retained tests and graph layouts before linking.
There are no synthetic helper calls or provider-generated checking values.

The internal hgl-rust::native_images module uses checked ir, native_values,
hgl-semantics::native_images and hgl-rust-codegen::native_image. emit(Plan)
unions reached type mappings with invoked helper providers, emits one module
per provider, and emits one alias/domain per reached canonical scalar. All
contributing modules retain their compatible marker mappings. Selected image
identity, fingerprint and scalar ABI checks run before execution; unreachable
providers and unused scalar mappings remain unlinked.

Acceptance: helper-free configured empty List<Native>, globals, silent native
ports, generator locals, compound configuration, boxed ordinary empty native
lists, compatible aliases and multi-module shared images execute in debug and
optimized profiles. Unused unmapped/nonexistent images stay unlinked; selected
fingerprint mismatches fail before execution. Successful graph cycles allocate
nothing under the actual CountingAllocator beginning with the first cycle.
