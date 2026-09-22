# Fixed collection runtime measurements

Measured on the private Linux validation host, 2026-09-22. Rust sources:
`6077f1b`; hgraph SDK: `36c054ceb636cb1ce3bb93dc2a5d40cc775b047d`.
C++: GCC 14.3.0, CMake Release (`-O3 -DNDEBUG`, C++23).
Rust: 1.98.1, release with thin LTO and one codegen unit.

AMD Ryzen Threadripper 9980X; one-minute load 6.11 before and 2.78 after
measurement. Fifteen fresh processes per half, pinned to core 8, C++ then Rust for each
pair. Both executables check their checksum and evaluation count. The fixed
scenarios run 200,000 cycles; wiring is outside the timer. Values are ns/cycle,
reported as median and median absolute deviation (MAD).

| Scenario | C++ median | C++ MAD | Rust median | Rust MAD | Rust / C++ |
|---|---:|---:|---:|---:|---:|
| Owned TSL[TSB], four leaves | 1,230.63 | 2.95 | 132.21 | 0.23 | 0.107 |
| Assembled TSL[TSB], four leaves | 836.27 | 4.55 | 125.76 | 0.46 | 0.150 |
| Alternating whole REF[TSL[TSB]] | 1,830.43 | 6.66 | 327.50 | 1.09 | 0.179 |
| Scalar tick, 3M cycles | 287.35 | 1.15 | 45.13 | 0.18 | 0.157 |
| Scalar chain, depth 100 | 6,541.29 | 32.22 | 1,730.69 | 4.30 | 0.265 |
| Scalar wide_chain, 30 × 30 | 62,761.21 | 448.73 | 15,985.22 | 155.37 | 0.255 |

All six pairs pass the 1.05 ceiling. Scalar costs are about 12–18% above the
[previous Rust measurements](2026-09-21-dynamic-foundation.md), reflecting
additional collection bookkeeping. These measurements cover the stated
native graph shapes; they do not establish parity for arbitrary TSD churn,
non-peered REF construction or nested graph lifecycles.

The owned and assembled fixed checksums are 80,000,800,000; the alternating
REF checksum is 80,040,800,000. Each fixed run evaluates its sink 200,000 times.
C++ uses public native views; Rust projects typed leaf handles during wiring.

Reproduce with `cargo xtask bench --samples 15 --pin 8 -- <binary> <scenario>`.
Fixed binaries: `hgl_fixed_baseline` and `hgl-fixed-bench`; scenarios:
`owned`, `assembled`, `reference`. Scalar binaries: `hgl_baseline` and
`hgl_twin`; scenarios: `tick`, `chain`, `wide_chain`.
