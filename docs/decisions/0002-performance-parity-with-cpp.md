# 0002 — Performance is judged against the C++ runtime, within 5%

Status: accepted
Date: 2026-09-19
Exploration: ../explorations/0009-designing-for-speed.md

## Decision

The runtime has to be very fast, and nothing obviously slow is acceptable at
any stage, the prototype included. "Fast" is not an absolute number. For each
benchmark scenario:

1. the scenario is written against hgraph's **C++** interface — not through
   Python — and tested there;
2. it is measured, and that figure is the scenario's **baseline**;
3. the same scenario is written on this runtime, shown to give the same
   ticks, and measured the same way.

A scenario passes when it is **within 5% of its baseline**. More than 5%
slower fails. Faster is welcome and is not the goal.

Each scenario is a pair — the C++ baseline and its Rust twin — kept side by
side in this repository, the C++ half built against an installed hgraph.
Both halves are built optimised and measured on the owner's private Linux
validation host, back to back, as fresh-process samples with the median and
its spread reported. The hgraph commit, both compilers and their flags are
recorded with every result. A figure from another machine is not a result.

## Reason

An absolute budget would be a guess, and would go stale with the hardware.
The existing C++ runtime is the thing this one has to be able to stand in
for, so it is the only meaningful yardstick; writing the baseline in C++
rather than driving hgraph from Python keeps the comparison like for like.

## Given up

- A simple pass/fail number that can be checked on a laptop. Every
  performance claim needs the validation host and a baseline written first.
- Baselines are not cross-checked against hgraph's Python benchmark pack;
  Python is out of scope for now. A slip in writing a C++ baseline is caught
  only by that baseline's own tests.
- 5% is close to the noise on small scenarios, so the measuring method is
  part of the decision, not a detail.

## Upstream

None. The baselines use hgraph's public C++ interface as it stands.
