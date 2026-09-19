# 0001 — The new compiler back end, runtime and generated code are Rust

Status: accepted
Date: 2026-09-19
Exploration: ../explorations/0001-implementation-language-survey.md

## Decision

The compiler, the runtime kernel, the code the compiler emits and the native
escape are all Rust: one stable toolchain, one cargo workspace. Separately
built plugins cross a C ABI; Python is reached through PyO3. No parser
generator, no LLVM, no MLIR.

HGL itself is unchanged: the language is whatever `hgraph/language` specifies,
and an extension lands there first.

## Reason

Only Rust and C++ pass the gates (no GC on the tick path, first-class Python,
mature Kafka/Arrow/async libraries, three operating systems, mainstream enough
for agents). C++23 is the owner's stronger language and would have been a
sound choice. Rust was chosen because this is an experiment and the point is
to try the alternative: the evidence puts agents' C++ at the bottom and their
Rust at the top on repository-scale work, and Rust turns rules the reference
holds in prose — ownership, threading, lifetimes, exhaustiveness, layering —
into compiler errors, which is what a method built on checks needs.

## Given up

- The owner reviewing in their strongest language. Rust written here has to
  be plain enough to read as a second language; that is now a requirement on
  the code, not a courtesy.
- Reuse of the C++ front end (38k lines), the nanobind bridge and the C++
  extensions.
- A clean experiment: language and method change together, so an improvement
  cannot be credited to either alone.
- Cheap scripted execution by compiling generated code. Rust builds are too
  slow for that, so scripted runs need another route
  (exploration 0001, open question 1).
- A stable native ABI. Generic Rust cannot cross a plugin boundary.

## Upstream

None for the language choice. It makes ADR 0008's target mappings the first
upstream dependency: `native fn` bodies are C++ text today, and a Rust
projection has to be specified in hgraph before it can be used here.
