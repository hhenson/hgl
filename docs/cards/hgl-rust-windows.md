# Card: hgl-rust-windows

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `windows` module of `hgl-rust` (`crates/hgl-rust/src/windows.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Static rolling marker, query and arrival transport emission. Uses source and
rust-layouts; budget 100 source lines. No runtime execution or third-party deps.

marker(ty) emits the exact typed Rolling marker. read/apply/from/pass/capture/ready
emit latest-arrival extraction, native publication, configuration slot copying,
independent forwarding, prepared recording and readiness. query returns only
window-specific delta_value/all_valid forms; other endpoint queries remain with
the existing metadata emitter. All helpers receive a checked complete rolling
shape. The arrival marker is the ordinary V, never a retained-window snapshot.
scalar(ty,input,output) borrows a scalar input through prepared rolling transport,
copying one arrival into the independent reserved output ring.

Acceptance: six unchanged rolling shared cases execute in debug/release with
complete Graph::evaluate allocation measurements, plus nested structural child
and generic payload regressions. Finite cold capacity is the compiler's duty;
these fragments contain no dynamic schema interpretation or fallback allocation.
