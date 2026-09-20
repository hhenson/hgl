# Benchmarks

Performance here is judged one way only
([decision 0002](../docs/decisions/0002-performance-parity-with-cpp.md)): a
scenario on this runtime must run **within 5% of the same scenario on
hgraph's C++ runtime**, measured on the owner's private Linux validation
host.

## Pairs

Every scenario exists twice and the two are kept side by side:

| Half | Where | What it is |
|---|---|---|
| Baseline | `baselines/cpp/` | The scenario written against hgraph's C++ interface, as plain static nodes |
| Twin | *(arrives with the prototype)* | The same graph on this runtime |

Both halves print one line of JSON with `"ns_per_cycle"`, a `"checksum"`, and
`"ok"`. `ok` is true only if the checksum equals a closed form worked out
from the scenario's parameters, so a time is never reported for a graph that
computed the wrong thing. The twin must give the same checksum as the
baseline.

Nodes are hand-written, not library operators: what is timed is the runtime —
scheduling, notification, reading and writing a time-series — and not
operator dispatch.

## Scenarios

| Scenario | Graph | Default size | Measures |
|---|---|---|---|
| `tick` | pulse → add one → checksum | 3,000,000 cycles, 3 nodes | The fixed cost of a cycle |
| `chain` | pulse → add one × depth → checksum | 100,000 cycles, depth 100 | Cost per evaluated node |
| `wide_chain` | pulse → *width* chains of *depth*, folded by add → checksum | 20,000 cycles, 30 × 30 | Fan-out, and a wide rank order |

`pulse` emits 0, 1, 2, … one value per engine cycle, rescheduling itself with
its node scheduler. Every node is evaluated in every cycle.

## Measuring

```sh
cargo xtask bench --samples 15 --pin 8 -- <program> <scenario> [--cycles N] [--depth D] [--width W]
```

Fifteen fresh-process samples, pinned to one core; the median and the median
absolute deviation are reported. Both halves of a pair are measured with this
one command, back to back, on a quiet machine. Record the load average with
the result.

## Building the baselines

Against an installed hgraph, with the compiler and optimisation level hgraph
itself was built with:

```sh
cmake -S bench/baselines/cpp -B <build> -G Ninja -DCMAKE_BUILD_TYPE=Release \
      -DCMAKE_CXX_COMPILER=<as hgraph> -DCMAKE_PREFIX_PATH=<hgraph install> \
      -DHGRAPH_USE_PYARROW_ARROW=ON -DPython_EXECUTABLE=<python with pyarrow>
cmake --build <build>
```

The last two options are needed only because hgraph's installed package
looks for Arrow, and the hgraph build on the validation host takes Arrow from
pyarrow. Nothing in these scenarios uses Arrow or Python.

## Results

One file per measuring session under `results/`, recording the hgraph
commit, both compilers and their flags, the machine, the load, and every
figure.
