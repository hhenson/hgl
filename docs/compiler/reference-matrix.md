# Reference matrix implementation record

The supported slice executes the scalar, projection, structural and native leaf
REF suites: 24 named groups / 50 evaluations. Four conditional ownership groups
remain unexecuted. Installed dependencies are hgraph_spec `69a2f8c` and the
compatible hgraph_std `7c26434`; the Std pin has not advanced.

The tracked [matrix runner](../../crates/hgl-program/tests/reference_matrix.rs)
compiles byte-for-byte snapshots of hgraph_std
`d798c88dec4b729f1e96d2adcd087ca8df7f11ba`, with
[paths and hashes](../../crates/hgl-program/tests/fixtures/reference_matrix/PROVENANCE.json).
It uses normal source loading and target-module suite planning with the entire
production Std and native/stdlib. Debug and optimized runs verify all 24 groups,
50 evaluations and zero failures. Normal CLI execution uses the same sources
and complete libraries. Full CI remains pending root verification.

Correction: earlier 24/50 reports used the older `d298500` scalar snapshot;
the other three snapshots already matched `d798c88`. The latest scalar assertion
requires signal consumers to count selected target ticks and re-point sampling
under TS-18. The compiler now follows that contract while retaining signal
payload opacity. All current authored assertions are unchanged.

| Suite | Named group | Outcome |
| --- | --- | --- |
| `reference_conditional_expiry` | `reference_stopped_branch_target_expires_at_next_cycle_and_never_revives` | Admission blocked, unexecuted |
| `reference_conditional_expiry` | `reference_stopping_a_forwarding_branch_preserves_outer_target` | Admission blocked, unexecuted |
| `reference_leaf_values` | `held_reference_bytes_keep_empty_equal_distinct_and_silent_values` | Passed, executed |
| `reference_leaf_values` | `held_reference_any_keeps_one_box_leaf_across_contained_type_changes` | Passed, executed |
| `reference_leaf_values` | `held_reference_native_token_keeps_owning_scalar_publications` | Passed, executed |
| `reference_leaf_values` | `held_reference_text_only_needs_no_payload_equality_or_hash` | Passed, executed |
| `reference_owner_export_values` | `reference_exports_compose_through_two_conditional_owners` | Admission blocked, unexecuted |
| `reference_owner_export_values` | `reference_exported_child_tree_keeps_sparse_ticks_and_samples_changed_children` | Admission blocked, unexecuted |
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

The remaining contracts are defined in
[control flow](../../external/hgraph_spec/language/docs/design/control-flow.md),
[conditional expiry](../../external/hgraph_spec/compiler/cases_reference_conditional_expiry.md),
and [owned export](../../external/hgraph_spec/compiler/cases_reference_owner_export.md).
They require checked child graphs, definite assignment, fresh branch lifetimes
and declared REF output exports. Temporal graph conditionals remain unsupported.

A real CountingAllocator measures every successful evaluation cycle in the
tracked 24/50 runner, including first designation sampling: zero allocations
in both profiles. The required-read error preserves its authored expectation;
failed cycles are not included in the successful-cycle allocation guarantee.
Cold preparation reserves finite target subscriptions and absent collection
views without publishing, sampling or granting membership/lifetime.
