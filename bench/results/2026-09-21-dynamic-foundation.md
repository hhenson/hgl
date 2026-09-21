# Scalar cost after dynamic runtime support

Status: measured on the private Linux validation host, 2026-09-21.

Fresh paired measurements of the unchanged P1 scenarios. Same C++ binary
and conditions as [P1](2026-09-20-p1-twins.md): hgraph `36c054c`, GCC 14.3.0,
CMake Release (`-O3 -DNDEBUG`); Rust 1.98.1, release with thin LTO and one
codegen unit. Fifteen fresh processes per half, pinned to core 8. Load average
was 4.43 after the build and 2.49 after measurement. Every sample checks its checksum.

Nanoseconds per cycle; median and median absolute deviation (MAD).

| Scenario | C++ median | C++ MAD | Rust median | Rust MAD | Rust / C++ |
|---|---:|---:|---:|---:|---:|
| tick, 3M cycles | 287.91 | 0.58 | 40.47 | 0.39 | 0.141 |
| chain, depth 100 | 6,528.25 | 17.96 | 1,462.40 | 4.11 | 0.224 |
| wide_chain, 30 × 30 | 62,825.83 | 175.05 | 13,734.87 | 176.96 | 0.219 |

All remain below the 1.05 ceiling. Relative to the recorded P1 Rust medians,
this is 55%, 33% and 25% slower. Generation checks, endpoint metadata and
scope routing have a measurable cost; the C++ headroom remains substantial.
This comparison does not measure TSD/REF/nested workloads against C++.

Reproduce with `cargo xtask bench --samples 15 --pin 8 -- <binary> <scenario>`
for `hgl_baseline` and `hgl_twin`, using `tick`, `chain`, `wide_chain` in turn.
