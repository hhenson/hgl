# Reference matrix implementation record

All six authored REF suites execute unchanged: 28 named groups / 58
evaluations. The tracked transport and ownership runners use byte-for-byte
snapshots of hgraph_std `d798c88dec4b729f1e96d2adcd087ca8df7f11ba`, with
[paths and hashes](../../crates/hgl-program/tests/fixtures/reference_matrix/PROVENANCE.json).
Installed dependencies are hgraph_spec `69a2f8c` and hgraph_std `d798c88`.
The complete loaded Std corpus passes in both profiles: 360 named tests / 957
evaluations, including test blocks in production modules. The existing
contextual fixtures add 12 tests / 22 evaluations; the combined checked source
corpus is 372 / 979. Separate existing harness regression and expected-failure
images also pass. CI and platform evidence is recorded in [PR #83](https://github.com/hhenson/hgl/pull/83).

Normal CLI execution uses the complete production Std and native/stdlib.
The [transport runner](../../crates/hgl-program/tests/reference_matrix.rs)
verifies 24 groups / 50 evaluations and the
[ownership runner](../../crates/hgl-program/tests/reference_conditional.rs)
verifies 4 groups / 8 evaluations. Both profiles pass with zero failures and
zero allocations on every successful cycle from first activation. Ownership
startup and final cleanup also allocate nothing.

Correction: earlier 24/50 reports used the older `d298500` scalar snapshot;
the other three snapshots already matched `d798c88`. The latest scalar assertion
requires signal consumers to count selected target ticks and re-point sampling
under TS-18. The compiler now follows that contract while retaining signal
payload opacity. All current authored assertions are unchanged.

| Suite | Named group | Outcome |
| --- | --- | --- |
| `reference_conditional_expiry` | `reference_stopped_branch_target_expires_at_next_cycle_and_never_revives` | Passed, executed |
| `reference_conditional_expiry` | `reference_stopping_a_forwarding_branch_preserves_outer_target` | Passed, executed |
| `reference_leaf_values` | `held_reference_bytes_keep_empty_equal_distinct_and_silent_values` | Passed, executed |
| `reference_leaf_values` | `held_reference_any_keeps_one_box_leaf_across_contained_type_changes` | Passed, executed |
| `reference_leaf_values` | `held_reference_native_token_keeps_owning_scalar_publications` | Passed, executed |
| `reference_leaf_values` | `held_reference_text_only_needs_no_payload_equality_or_hash` | Passed, executed |
| `reference_owner_export_values` | `reference_exports_compose_through_two_conditional_owners` | Passed, executed |
| `reference_owner_export_values` | `reference_exported_child_tree_keeps_sparse_ticks_and_samples_changed_children` | Passed, executed |
| `reference_projection_values` | `reference_graph_projections_follow_children_and_ignore_sibling_ticks` | Passed, executed |
| `reference_projection_values` | `reference_graph_projections_retarget_even_when_child_values_are_equal` | Passed, executed |
| `reference_structural_values` | `held_reference_set_and_map_consumers_publish_original_deltas` | Passed, executed |
| `reference_structural_values` | `held_reference_lists_keep_sparse_indices_and_tail_removal` | Passed, executed |
| `reference_structural_values` | `held_reference_tuple_and_nominal_consumers_preserve_child_omissions` | Passed, executed |
| `reference_structural_values` | `held_reference_atomic_consumers_keep_complete_replacement_and_retention` | Passed, executed |
| `reference_structural_values` | `held_reference_rolling_consumer_uses_arrivals_before_readiness` | Passed, executed |
| `reference_structural_values` | `held_reference_enum_and_complete_family_preserve_nominal_identity` | Passed, executed |
| `reference_structural_values` | `held_reference_structural_consumers_keep_empty_and_silent_horizons` | Passed, executed |
| `reference_structural_values` | `held_reference_map_target_remains_selected_when_route_changes` | Passed, executed |
| `reference_structural_values` | `held_reference_to_an_invalid_map_waits_for_its_own_publication` | Passed, executed |
| `reference_structural_values` | `reference_following_guards_invalid_targets_and_unset_children` | Passed, executed |
| `reference_structural_values` | `followed_reference_partial_observation_uses_existing_required_read_failure` | Passed, executed |
| `reference_values` | `reference_binding_follows_target_ticks_and_silence` | Passed, executed |
| `reference_values` | `reference_equality_compares_endpoints_instead_of_target_values` | Passed, executed |
| `reference_values` | `reference_selection_samples_new_targets_and_suppresses_equal_designations` | Passed, executed |
| `reference_values` | `reference_metadata_distinguishes_invalid_targets_from_valid_designations` | Passed, executed |
| `reference_values` | `reference_rebinding_samples_the_input_without_changing_producer_time` | Passed, executed |
| `reference_values` | `reference_held_output_keeps_a_live_designation_across_later_routes` | Passed, executed |
| `reference_values` | `reference_bindings_preserve_fixed_and_nominal_child_identity` | Passed, executed |

Conditional ownership follows
[control flow](../../external/hgraph_spec/language/docs/design/control-flow.md),
[conditional expiry](../../external/hgraph_spec/compiler/cases_reference_conditional_expiry.md),
and [owned export](../../external/hgraph_spec/compiler/cases_reference_owner_export.md).
Both branches definitely assign the declared REF result. Each activation starts
a fresh real child graph, stops the previous child first, and exports its actual
designation. A held owned target remains readable during the stop cycle, expires
at the next cycle and never revives. Forwarding an enclosing target preserves
that target's lifetime. Nested owners and fixed nominal assemblies preserve
individual child identity and sparse sampling.

The current Rust target proves finite activation from the dense horizon, checked
direct source yields, replay configuration lengths, startup hooks and emitted
startup-scheduled evaluations, including REF inputs and fixed assemblies. Periods
may separate those arrivals into additional timestamps. A tracked control drives
eight selector timestamps after one dense input row; it passes without allocation.
Zero-horizon controls activate both branches from REF-input selectors and an
assembled nominal selector; each executes one startup cycle without allocation.
Child generators, scheduler hooks, owning cache resets and unknown finite transport proofs produce an
explicit Rust target diagnostic while preserving checked source compilation.
A stopped instance is never reused; bound exhaustion is an error. Cold recursive
preparation currently reserves each child's independent pools, so its memory cost
grows with nested ownership depth.

A real CountingAllocator measures successful cycles in both tracked runners;
failed required-read cycles retain their authored error expectation. Cold
preparation reserves possible target subscriptions and structural input views
without publishing payloads or extending endpoint lifetimes. Separate runtime
controls cover foreign ownership, failed activation, unused descendants,
repeated cleanup and pool exhaustion.
