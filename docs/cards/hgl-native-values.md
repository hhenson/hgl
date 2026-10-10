# Card: hgl-native-values

The native_values module in hgl-store implements a typed opaque owning-buffer ABI for statically linked native scalar providers. Its budget is 260 lines within the unchanged store cap. It uses global_arena, global_value, prepared_value and owned_columns.

Public surface: Contract, NativeType, NativeValue, NativeView and Native. Contract validates canonical identity, fingerprint and required capabilities before construction. Provider types carry static executable operations, with process-lifetime image retention. NativeValue owns independent provider data; NativeView borrows read-only data for one call. Native markers use exact canonical OrdinaryType::Native identity and independently reserved slots. Preflight precedes copying; copying uses the provider operation without allocation, locks, name lookup or per-value type tests. This ABI does not expose the opaque representation to HGL or infer provider equality from bytes.

Acceptance: exact provider mismatch rejection, independent ordinary global reads, capacity failure preserving retained data, empty replacements, identity-slot copies and measured zero-allocation prepared copying. Source native type admission and executable package discovery remain a separate compiler integration step.
