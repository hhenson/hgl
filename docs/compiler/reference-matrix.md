# Reference matrix implementation record

The six reference suites authored at hgraph_std `d798c88` contain 28 named
groups. The baseline is HGL `265facc`, hgraph_spec `69a2f8c`, and the compatible
installed hgraph_std `7c26434`; the six test files were read at `d798c88` without
changing the installed library pin. Every run uses the normal `hglc test` CLI
with the entire `external/hgraph_std/hgl/hgraph` and `native/stdlib` libraries.
Assertions and source spelling are unchanged.

All six baseline suites failed before executing any group. Source admission
rejected REF own-output metadata, nominal graph projections and graph variables
without initial bindings. The structural suite reached Rust emission but its
Set width dependency through REF had no declaration. The native leaf suite
rejects its atomic Tuple result containing a borrowed native text helper.

The implemented projection node captures statically resolved child identity
through its existing bound input view and publishes a REF designation. It never
reads the child's payload. Existing runtime suppression avoids an extra tick
for unchanged designations; ordinary consumers follow child publications and
sample newly selected endpoints even when values are equal. REF inputs remain
opaque in source node handlers. Set width preparation follows REF dependencies
without running handlers.

Current focused validation executes all 24 groups / 50 evals in the scalar,
projection, structural and native leaf suites. Four authored groups remain
unexecuted in the two lifecycle suites. The supported transport slice was
revalidated after merging the corrected reviewed Native implementation at `283ac5b`
(actual main ancestry `2be08a4`). The integration revisions `94e982e` and `6e2c791` pass
normal CLI execution for each of the four suites, the combined real allocation
probe, focused compiler/store regressions, affected clippy and the module/card
budget gate. Full CI remains pending host coordination; this record does not
claim completion.

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

The remaining lifecycle source contracts are already defined by
[control flow](../../external/hgraph_spec/language/docs/design/control-flow.md),
[conditional expiry](../../external/hgraph_spec/compiler/cases_reference_conditional_expiry.md),
and [owned export](../../external/hgraph_spec/compiler/cases_reference_owner_export.md).
The current compiler rejects temporal graph conditionals; implementing them
requires checked child graph descriptions, definite assignment and output
boundary export rather than changing reference payload semantics.

The prepared generated allocation probe executes the exact 24 reference groups
and their 50 evaluations with a real CountingAllocator. Every successful cycle,
including first designation sampling, has zero allocations. Scalar/projection
cases and structural/native cases were observed independently; the required-read
failure retains its existing error behavior. Imported native test groups were
excluded from this targeted probe, matching the normal CLI target scope.

Cold route analysis follows only checked REF capture dependencies and fixed
projection indices. It reserves target subscription buffers and the union of
possible absent collection views without binding, sampling or publishing them.
The focused regression covers a retained invalid Map target and distinct Native
text payloads inside an atomic Tuple under actual allocation counting.
