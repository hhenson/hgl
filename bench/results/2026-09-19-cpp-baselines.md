# C++ baselines — 2026-09-19

The first baselines: `tick`, `chain` and `wide_chain` on hgraph's C++ runtime.
These are the figures the prototype's P1 slice is held to, within 5%
([0008](../../docs/explorations/0008-prototype-outline.md)).

## Conditions

| | |
|---|---|
| Machine | The owner's private Linux validation host: AMD Ryzen Threadripper 9980X (64 cores, 128 threads), Ubuntu 26.04.1 LTS, governor `performance` |
| hgraph | `36c054c` (main, "Merge pull request #1016"), core only: no Python bindings, no extensions, static library |
| Compiler | g++ 14, `-O3 -DNDEBUG` (CMake `Release`), C++23 — for hgraph and for the baselines alike |
| Method | `cargo xtask bench --samples 15 --pin 8`: 15 fresh-process samples, pinned to core 8 |
| Load average | 0.93 before, 0.96 after (1 minute). The 5-minute figure was 5.8, from the hgraph build that preceded the run |
| Timed | `run()` only: wiring and executor construction are outside the clock |

Every sample passed its own checksum check.

## Figures

| Scenario | Size | ns / cycle (median) | MAD | min | max | ns / evaluated node |
|---|---|---|---|---|---|---|
| `tick` | 3,000,000 cycles, 3 nodes | **288.29** | 1.35 (0.47%) | 283.98 | 292.81 | 96.1 |
| `chain` | 100,000 cycles, 102 nodes | **6,506.88** | 17.69 (0.27%) | 6,450.00 | 6,687.58 | 63.8 |
| `wide_chain` | 20,000 cycles, 961 nodes | **62,568.39** | 217.77 (0.35%) | 62,257.50 | 67,197.01 | 65.1 |

## The targets these set

Within 5%, in nanoseconds per cycle:

| Scenario | Passes at or below |
|---|---|
| `tick` | 302.70 |
| `chain` | 6,832.22 |
| `wide_chain` | 65,696.81 |

## Reading them

- The spread is under half a percent, so a 5% difference is well clear of the
  noise on this machine with this method.
- A node costs the reference about **64 ns** to evaluate once a graph is large
  enough for the per-cycle fixed cost to stop mattering; `chain` and
  `wide_chain` agree to within 2%.
- `tick` is dearer per node (96 ns) because a third of its nodes is the
  source, which goes through the node scheduler every cycle, and because the
  cycle's fixed cost is spread over only three nodes.
- These runs were made shortly after a large build on the same machine. They
  should be repeated on an idle machine before a twin is judged against them.
