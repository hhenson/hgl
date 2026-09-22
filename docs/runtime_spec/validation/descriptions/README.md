# Description-boundary comparisons

Five scenarios, each replayed three times in separate Python and C++ processes.
All fingerprints are stable. The [reasoned states](reasoned.json) were written
before execution; [observations](observed.json) retain runtime and harness hashes.
No runtime or compiler implementation was changed during this validation.

| Result | Assertions |
|---|---:|
| Reasoning, Python and C++ agree | 9,330 |
| Reasoning and one runtime agree | 614 |
| Existing explicit ruling supplies the contract | 46 |

The cases differ only in binding: owned, assembled, or assembled with a peered
left list. Each has eleven cycles through a switch and keyed child graph,
REF selection, accumulation, timer replacement, removal and recreation.
Two further cases rebind the captured parent input while its child stays alive;
the child follows the new target without restarting its state.
Every aggregate and leaf is observed on parent inputs, evaluated child inputs
and outputs, live dictionary members and removed members.

## Variations

- Python omits invalid bundle fields; C++ preserves them (TS-24).
- A sampled owned list: Python includes its invalid slot in the delta; C++
  omits the valid slot. Sixteen observations use the existing TS-14 ruling:
  only valid sampled children contribute.
- Python removes the stopped child's data immediately. C++ retains its whole
  value and time but raises on descendant access. Thirty descendant
  observations use the existing TS-11 ruling: values and leaf times remain
  readable through removal. An unavailable C++ read is not a matching result.
- Python's owned TSB `all_valid` accessor raises `AttributeError`; C++ matches
  the expected false value. These missing reads are retained explicitly.
- Both engines detach the removed input's root. The initial expectation that
  retention implied peering was wrong. [Corrections](corrections.json) separate
  detached binding from retained data; Python also confirms child detachment.

[assessment.json](assessment.json) contains every difference. [rulings.json](rulings.json)
identifies prior user decisions; these are not newly inferred approvals. Missing
reads remain absent from raw evidence, including where a prior ruling accepts
the proposed contract. No new unresolved three-way disagreement remains.

## Replay

Prepare the isolated harness at the base named by `../fixed/harness_identity.py`
and apply [adapter.patch](adapter.patch). The existing verifier checks every
tracked/non-ignored file against that base plus adapter before replay.

```sh
python -m tools.parity validate /path/to/descriptions/recipes
python ../fixed/replay.py --corpus /path/to/descriptions \
  --harness /path/to/harness --reference-python /path/to/reference/bin/python \
  --candidate-python /path/to/candidate/bin/python --raw-results /tmp/description-results
python check.py
```

The bounded adapter reads public runtime properties. It captures accessor
errors as missing observations rather than inventing fallback values. It does
not implement binding, timing, validity or retention semantics. These traces
validate runtime boundaries; malformed serialized descriptions additionally
require static GRF-4/6/7/9 rejection tests in the description builder.
