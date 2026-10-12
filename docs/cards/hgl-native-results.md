# Card: Prepared native results

The native_results module of hgl-store provides PreparedTick::native_result and text_result. Each executes one checked borrowed-result callback, preflights the result against reserved capacity, clears the destination once and appends the independent owning copy, then publishes. A failed helper or preflight leaves the prior destination and publication unchanged. Empty results replace prior contents. No allocation, name lookup or per-value type test is permitted on success.

The native_results module of hgl-rust-codegen selects supported borrowed expression trees and emits publication/write code. Its limits pass invokes descriptor-selected pure bound functions during cold provisioning. Bound composition never executes semantic helper bodies. Arithmetic overflow and unsound result sizes are failures; they cannot silently truncate. The native_values module emits exact canonical markers, inert metadata, cold encode/decode and byte-sink append calls. Source phase/type admission remains above this backend.

Controls count helper execution, reject missing and unsound bounds, preserve old destinations after failure, exercise empty results and aliases, and run generated source simulations under allocation counting.

Initial module ceilings: Store native_results 80 lines; codegen native_results 180 and native_values 60 lines. All remain within their unchanged aggregate ceilings.

The compiler-only native_owners module stages constructor operands in written
order before any destination write. Native helper results receive independent
reserved locals; nested branches retain their execution scope. Constructor
metadata is added only afterwards, so bounds remain inert. It has an initial
80-line ceiling, no sibling dependencies, and unchanged codegen aggregate cap.
Mutable canonical constructor owners and ordinary/boxed alias regressions cover
the shared storage path. GlobalState::native_result uses a distinct reserved
temporary, preserving helper input globals until the independent copy completes.

Standalone/generic emission retains borrowed provider results into ordinary owners
using NativeView::retain or text ownership conversion. Reserved Native snapshots
and constructor operand staging apply only to prepared plans. Generic comparison
emission holds both owning operands through the fallible provider operation;
finite prepared views keep the allocation-free path described above.

GlobalState::text_result(observation, ValueSlot<String>, execute) retains a
borrowed helper result into a distinct reserved ordinary slot. It runs the
helper once, preflights capacity before modifying the prior buffer, and restores
that buffer after either helper or preflight failure. It publishes no temporal
endpoint. The compiler reserves canonical Tuple construction stages for selected
bounded text helpers at complete atomic result positions; source REF operands
retain endpoint identity and require no native payload equality or hash.
