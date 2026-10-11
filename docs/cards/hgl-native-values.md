# Card: hgl-native-values

The native_values module in hgl-store implements a typed opaque owning-buffer ABI for statically linked native scalar providers. Its budget is 260 lines within the unchanged store cap. It uses global_arena, global_value, prepared_value and owned_columns.

Public surface: Contract, NativeType, NativeValue, NativeView and Native. Contract validates canonical identity, fingerprint and required capabilities before construction. Provider types carry static executable operations, with process-lifetime image retention. NativeValue owns independent provider data; NativeView borrows read-only data for one call. Its from_provider boundary and explicit data lifetime support descriptor-selected borrowed callback results; retain materializes independent ownership during execution. Native::check_view and copy_view preflight and copy those results into prepared owning slots. Native markers use exact canonical OrdinaryType::Native identity and independently reserved slots. Preflight precedes copying; copying uses the provider operation without allocation, locks, name lookup or per-value type tests. This ABI does not expose the opaque representation to HGL or infer provider equality from bytes.

Acceptance: exact provider mismatch rejection, independent ordinary global reads, capacity failure preserving retained data, empty replacements, identity-slot copies and measured zero-allocation prepared copying. The physical native/stdlib/atomic_values.rs fixture implements the three approved canonical owning-text contracts; storage tests compile the actual image and verify borrowed callbacks, independent retention and TextOnly capability masking. Source native type admission and selection of this prepared fixture ABI remain a separate compiler integration step.

The Rust owning-buffer copy ABI appends exactly the source encoding width into
an abstract byte sink. The caller clears an owning destination before invoking
the provider; a nested frame instead preserves its prefix and appends directly.
Provider preflight validates complete data and available reserved capacity before
any destination mutation. Copying after successful preflight neither allocates
nor fails, and keeps the provider's declared value meaning. A sizing sink counts
bytes without executing provider copy callbacks. This backend representation
contract adds no HGL constructor, conversion, mutation or source spelling.

Owning Any plans carry their compiled copy codec, including nested provider
leaves. Retaining an Any value follows that codec instead of cloning opaque bytes;
prepared Any frames append through the same sink without temporary owners.
Controls require independent nested aliases, replacement isolation, empty
payloads, failed preflight preserving old data and exact provider-copy counts.

Native::key_view exposes a readonly view of an already prepared key identity to compiled copy paths; key_view_id performs provider hash/equality lookup. Native global replacement and rolling publication use existing typed slot preflight/copy operations. No ownership, name lookup or allocation is introduced at the hot key copy boundary.

NativeView::equal and compare invoke statically selected provider operations and preserve fallible results. GlobalState::native_result executes a borrowed helper once against readonly observation and global views, then preflights and copies into a separate reserved result slot. That destination must not be an argument owner: compiled snapshot planning reserves a fresh temporary before updating an argument global. A failed helper or preflight restores the old destination buffer unchanged.
