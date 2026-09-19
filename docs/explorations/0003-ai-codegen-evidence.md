# 0003 — What the evidence says about AI agents, languages and clean code

Status: sketch

Gathered 2026-09-19 as input to
[0001](0001-implementation-language-survey.md). Items marked ✓ were re-read at
source in the session that wrote this; the rest are as reported by research
agents, with the URL to check. Most of this evidence is weak: small
per-language samples, self-reported builds, models that are already old.

## Repository-level benchmarks, by language

| Benchmark | Result |
|---|---|
| SWE-bench Multilingual ✓ (300 tasks, SWE-agent + Claude 3.7) | Rust 58.1, Java 53.5, PHP 48.8, Ruby 43.2, JS/TS 34.9, Go 31.0, **C/C++ 28.6** |
| Multi-SWE-bench (1,632 tasks, Claude 3.7) | Java 23.4, Rust 15.9, C++ 14.7, TS 11.2, C 8.6, Go 7.5 |
| SWE-Bench ProMax (Aug 2026, multi-file refactors, 10–30 tasks per language) | C 75.0, Rust 54.5, Python 48.3, C++ 36.4, TS 35.7, Go 26.1, Java 19.2 |
| Multi-LCB (algorithmic puzzles, mean pass@1) | Python 48, Java ≈ C++ ≈ 44, Go/Rust/Kotlin/C#/TS 33–39 |

- C++ looks fine on puzzles (training volume) and poor on real repositories.
  Rust is the reverse. The SWE-bench authors could not explain the spread:
  Rust patches were the largest yet resolved best.
- Go is middling on benchmarks despite strong practitioner advocacy.
- No credible repository-level numbers exist for Zig, OCaml, Swift, Odin, Mojo.
- In low-resource languages agents burn tokens on non-compiling code, or write
  a Python generator instead of the target language (arXiv 2607.22807,
  2606.10933). Frontier models solved a Zig 0.15 upgrade only after looking up
  changed std APIs on the web.

## The controlled counterweight ✓

Dan Luu had agents write a zstd decoder and a Pandoc clone across 34
languages (<https://danluu.com/pl-tokens/>). Popularity correlates only weakly
to moderately with correctness and cost; static-vs-dynamic did not hold;
obscure languages did poorly. The one folk claim that survived: **memory
safety issues "in all of the C programs and all but one of the C++
programs"**, none of it counted in the scores. His advice is no stronger than
"stick with a mainstream language". A separate study found AI-written C++
about twice as likely as human C++ to trip ASan/UBSan while cppcheck and
clang-tidy saw no difference (arXiv 2607.00107).

## Large agent-driven builds

| Build | Language | Scale | Oracle | Outcome |
|---|---|---|---|---|
| Anthropic C compiler | Rust | 100k lines, 16 agents | GCC torture tests, GCC as reference | Builds Linux; author rates code "reasonable", not expert; needed dedup and critique agents |
| Cursor FastRender | Rust | 1M+ lines | none comparable | CI failing 88% of the time; "a tangle of spaghetti" (a Servo maintainer) |
| Ladybird LibJS ✓ | C++ → Rust | 25k lines, 2 weeks, human-directed, "hundreds of small prompts" | byte-identical bytecode vs the C++ pipeline; 52,898 + 12,461 tests, 0 regressions | Works; "it isn't idiomatic Rust" |
| Bun ✓ | Zig → Rust | ~780k lines, 11 days, up to 64 agents, ~$165k | 1.3M `expect()` calls in a TypeScript suite, none skipped | Shipped; 4% `unsafe`; 19 post-merge regressions; Zig's author: "unreviewed slop" |
| JustHTML | Python, then JS | 3k lines | html5lib suite, 9,200 tests | JS port done in 4.5 hours against the same suite |
| Cloudflare OAuth | TypeScript | small | RFCs, every line human-reviewed | Clean |

Every success had an external oracle. None came out clean by default. The
clean ones were small and human-steered. What Bun credits ✓
(<https://bun.com/blog/bun-in-rust>, summarised by Willison): "a
language-independent test suite with a million assertions, adversarial code
review and when something does go wrong, fixing the process that generates
the code instead of hand-fixing the code." Reviewers got the diff only, never
the implementer's reasoning. Long explanatory comments were treated as a smell.

Practitioner opinion, no data: Ronacher and Yegge favour Go (fast cycles,
greppable package-qualified names, boring diffs); Ronacher lists what hurts
agents — macros, re-exports, aliasing, exceptions, language-server dependence,
and "underrepresented in the weights… and also changing quickly" (Zig).

## Practices, ranked by how much evidence they have

1. **An external, language-independent oracle.** Strongest by far.
2. **A human-built test and triage structure that agents extend.** Naming a
   technique in the prompt does not work ✓: across 26 prompt conditions
   (<https://danluu.com/agentic-testing/>) TDD, differential testing, Verus,
   Lean, Alloy and published skills scored at or below the no-instruction
   default. "Regardless of the library or technique suggested, agents failed
   to use the technique." Structured random inputs and fresh-context
   re-derivation helped.
3. **Adversarial review from a fresh context** (Bun ✓, Ladybird ✓, Luu ✓).
4. **Mechanical enforcement.** Dependency layers held by linters and
   structural tests, review comments turned into lint rules, cleanup agents on
   a schedule (OpenAI harness write-up; self-reported, read via a secondary
   source).
5. **Volume budgets.** Total lines correlated with architectural smells at
   ρ = 0.94 in one Python study (arXiv 2605.02741). Known side effect: agents
   split functions awkwardly to satisfy a metric.
6. **Short instruction files.** Context files did not raise success and added
   over 20% cost; overviews were useless, explicit rules were obeyed
   (arXiv 2602.11988).
7. **Spec-driven tooling** (Kiro, Spec Kit, Tessl): weak. Böckeler found
   agents ignoring the spec and regenerating existing classes — a "false
   sense of control". No controlled study shows it improves quality.
8. **Nothing found** for TLA+, design-by-contract or literate programming at
   this scale; mutation testing has vendor posts only.

## Failure modes that have been measured

Duplicate blocks up 8× 2020–2024 and refactoring down from 24% to 9.5% of
changed lines (GitClear); agent PRs 1.87× as redundant as human ones, yet
reviewed more favourably (arXiv 2601.21276); +2.6 statements per function and
deeper nesting (2603.27130); 36% of agent commits add mocks vs 26%
(2602.00409); error-masking constructs up 47% (GitClear 2026 via LeadDev);
"monolithic, single-file implementations" (ProgramBench).

## What this means here

- **Correctness can be specified, through oracles** — and this project has
  two: the `.hgl` corpus and the reference runtime
  ([0002](0002-reference-implementation-profile.md)).
- **Cleanliness cannot yet be specified; it is enforced.** Budgets, layer
  rules, lints, fresh-context review, scheduled cleanup. The method this
  experiment is looking for should be made mostly of checks that run, not
  prose that describes.
- **Language:** mainstream, memory-safe, one toolchain. C and C++ are the
  worst-supported choice in the data; young languages cost tokens and
  hallucinated APIs. No language prevents bloat.
- **A good oracle makes the language choice reversible.** That is the real
  lesson of Bun, Ladybird and JustHTML, and an argument for building the
  conformance corpus before the runtime.
