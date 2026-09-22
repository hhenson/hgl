"""Run focused runtime mutants on a fresh, disposable copy of this checkout."""
from pathlib import Path
import json
import shutil
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[1]
cases = [('child_start_stop',
  'crates/hgl-kernel/src/ctx.rs',
  'graph.start(self.store, self.now)?;\n            if graph.stop_requested()',
  'graph.start(self.store, self.now)?;\n            if false',
  ['-p', 'hgl-nested', '--test', 'start_stop']),
 ('repeated_invalidation',
  'crates/hgl-bindings/src/lib.rs',
  'if self.output(output).modified_at == EngineTime::NEVER {',
  'if false {',
  ['-p', 'hgl-store', '--test', 'invalidation']),
 ('released_parent_membership',
  'crates/hgl-bindings/src/scopes.rs',
  'self.remove(parent, key, now, wake);',
  '// deliberately retain the parent membership',
  ['-p', 'hgl-store', '--test', 'released_members']),
 ('replacement_membership',
  'crates/hgl-bindings/src/scopes.rs',
  '&& self.child_output(parent, key) == Some(o)',
  '&& true',
  ['-p', 'hgl-store', '--test', 'released_members']),
 ('expired_reference_projection',
  'crates/hgl-bindings/src/lib.rs',
  'self.clear_projection(input);',
  '// deliberately retain the expired reference projection',
  ['-p', 'hgl-store', '--test', 'released_members', 'expired_reference']),
 ('generation_wrap',
  'crates/hgl-bindings/src/lib.rs',
  'o.generation.checked_add(1)',
  'Some(o.generation.wrapping_add(1))',
  ['-p', 'hgl-bindings', '--lib']),
 ('released_child_mailbox',
  'crates/hgl-bindings/src/scopes.rs',
  'parent.children[owner.0 as usize].retain(|&id| id != scope);',
  '// deliberately retain old mailbox entry',
  ['-p', 'hgl-bindings', '--test', 'admission']),
 ('restart_stopped_graph',
  'crates/hgl-kernel/src/graph.rs',
  'self.lifecycle != Lifecycle::Instantiated',
  'false',
  ['-p', 'hgl-kernel', '--test', 'lifecycle']),
 ('reference_generation',
  'crates/hgl-bindings/src/lib.rs',
  'o.alive && o.generation == r.generation',
  'o.alive',
  ['-p', 'hgl-store', '--test', 'dynamic']),
 ('sampling_time',
  'crates/hgl-bindings/src/fixed.rs',
  'self.endpoints.inputs[input.0 as usize].sampled_at = now;',
  'self.endpoints.inputs[input.0 as usize].sampled_at = EngineTime::NEVER;',
  ['-p', 'hgl-store', '--test', 'dynamic']),
 ('ancestor_wake',
  'crates/hgl-endpoints/src/scopes.rs',
  'scope = parent;\n            node = owner;',
  'let _ = (parent, owner);\n            return;',
  ['-p', 'hgl-nested', '--test', 'accepted']),
 ('expired_child',
  'crates/hgl-bindings/src/lib.rs',
  'while n < self.retired.len() {',
  'while n < 0 {',
  ['-p', 'hgl-store', '--test', 'dynamic']),
 ('watcher_detach',
  'crates/hgl-bindings/src/lib.rs',
  'watchers.swap_remove(position);',
  '// deliberately retain subscription',
  ['-p', 'hgl-store', '--test', 'dynamic']),
 ('removed_deadline',
  'crates/hgl-nested/src/lib.rs',
  'self.deadlines.remove(slot);',
  '// deliberately retain removed deadline',
  ['-p', 'hgl-nested', '--test', 'timers', 'removal_cancels']),
 ('evaluate_twice',
  'crates/hgl-nested/src/lib.rs',
  'ctx.evaluate_child(&mut child.graph)?;',
  'ctx.evaluate_child(&mut child.graph)?;\n            ctx.evaluate_child(&mut child.graph)?;',
  ['-p', 'hgl-nested', '--test', 'accepted']),
 ('stop_failed_start',
  'crates/hgl-kernel/src/graph.rs',
  'let _rollback = self.stop(store, now);',
  'self.started = rank + 1;\n                let _rollback = self.stop(store, now);',
  ['-p', 'hgl-nested', '--test', 'timers', 'start_failure']),
 ('heap_position',
  'crates/hgl-deadlines/src/lib.rs',
  'self.positions[self.heap[a].1] = a;',
  '// deliberately omit moved position',
  ['-p', 'hgl-deadlines']),
 ('heap_downward',
  'crates/hgl-deadlines/src/lib.rs',
  '        loop {\n            let left',
  '        return;\n        loop {\n            let left',
  ['-p', 'hgl-deadlines']),
 ('dictionary_activity',
  'crates/hgl-bindings/src/lib.rs',
  'self.set_active(child, active);',
  'let _ = child;',
  ['-p', 'hgl-bindings', '--test', 'admission']),
 ('repeated_follow',
  'crates/hgl-bindings/src/lib.rs',
  'if self.input(input).reference_source == Some(reference) {',
  'if self.input(input).reference_source == Some(reference) { self.unbind(input);',
  ['-p', 'hgl-store', '--test', 'dynamic'])]

# Accepted fixed timelines and lifecycle shields exercise these independent errors.
cases.extend([
 ('premature_scope_reuse', 'crates/hgl-bindings/src/scopes.rs',
  'self.scopes.retired.push((scope.index, now));', 'self.scopes.free.push(scope.index);',
  ['-p', 'hgl-store', '--test', 'fixed_lifetime', 'a_new_child_scope']),
 ('stopped_writer_restoration', 'crates/hgl-store/src/fixed.rs',
  '.restorable_output(dict, key)', '.removed_output(dict, key)',
  ['-p', 'hgl-store', '--test', 'fixed_lifetime', 'stopped_writer']),
 ('dead_endpoint_insertion', 'crates/hgl-bindings/src/collections.rs',
  '!o.alive || !self.scopes.alive(o.scope)', 'false',
  ['-p', 'hgl-bindings', '--test', 'admission', 'stopped_scope']),
 ('compound_sample_time', 'crates/hgl-bindings/src/collections.rs',
  'if sampled && self.output(output).modified_at != EngineTime::NEVER {', 'if false {',
  ['-p', 'hgl-store', '--test', 'fixed_edges', 'sampling_dictionary']),
 ('equal_fixed_ref', 'crates/hgl-bindings/src/lib.rs',
  '&& self.output(output).reference == r', '&& false',
  ['-p', 'hgl-store', '--test', 'fixed', 'accepted_reference']),
 ('unchanged_child_resampling', 'crates/hgl-bindings/src/fixed.rs',
  'if self.input(input).source == source && self.input(input).designation.items == r.items {',
  'if false {', ['-p', 'hgl-store', '--test', 'fixed', 'accepted_reference']),
 ('whole_reset', 'crates/hgl-bindings/src/fixed.rs',
  'self.reset_observation(input);', '/* retain local observation */',
  ['-p', 'hgl-store', '--test', 'fixed', 'accepted_collection']),
 ('recursive_all_valid', 'crates/hgl-bindings/src/collections.rs',
  'self.input(id).valid_children == self.input(id).fixed.len()',
  'self.input(id).fixed.iter().all(|&child| self.all_valid(child))',
  ['-p', 'hgl-store', '--test', 'fixed', 'accepted_collection']),
 ('compound_expiry', 'crates/hgl-bindings/src/lib.rs',
  'self.expire(self.output(id).fixed[n]);', 'let _ = n;',
  ['-p', 'hgl-store', '--test', 'fixed', 'accepted_compound']),
 ('descendant_rank', 'crates/hgl-bindings/src/fixed.rs',
  'self.check_designation(child, self.items[n].1[pos])?;', 'let _ = (child, n, pos);',
  ['-p', 'hgl-store', '--test', 'fixed_edges', 'rebind_rejects']),
 ('fixed_ancestor_time', 'crates/hgl-bindings/src/lib.rs',
  'self.notify_input(p, now, event, wake);', 'let _ = p;',
  ['-p', 'hgl-store', '--test', 'fixed', 'accepted_collection']),
])

cases.extend([
 ('description_overlap', 'crates/hgl-plan/src/paths.rs',
  'previous.node == edge.target.node', 'false',
  ['-p', 'hgl-describe', '--test', 'recursive', 'whole_and_descendant']),
 ('description_boundary_overlap', 'crates/hgl-plan/src/validation.rs',
  'earlier.node == edge.target.node', 'false',
  ['-p', 'hgl-describe', '--test', 'recursive', 'child_boundaries']),
 ('frozen_parent_capture', 'crates/hgl-describe/src/child.rs',
  '.follow(target, reference, now, &mut Quiet)',
  '.sample(target, store.bindings().output(reference).reference, now, &mut Quiet)',
  ['-p', 'hgl-describe', '--test', 'recursive', 'validated_nested_descriptions']),
 ('removed_descendant_peer', 'crates/hgl-endpoints/src/lib.rs',
  'self.input(parent).members.live.get(&key) != Some(&input)', 'false',
  ['-p', 'hgl-describe', '--test', 'recursive', 'validated_nested_descriptions']),
 ('frozen_projected_capture', 'crates/hgl-describe/src/child.rs',
  'store.bindings().input(source).reference_source.is_some()', 'false',
  ['-p', 'hgl-describe', '--test', 'recursive', 'projecting_through']),
])

with tempfile.TemporaryDirectory(prefix="hgl-dynamic-mutants-") as directory:
    root = Path(directory)
    files = subprocess.check_output(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=repo
    ).decode().split("\0")
    for name in files:
        source = repo / name
        if name and source.is_file():
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, destination)

    def run(args):
        return subprocess.run(
            ["cargo", "test", "--quiet", *args], cwd=root,
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=120
        )

    baseline = run(["-p", "hgl-store", "-p", "hgl-nested", "-p", "hgl-deadlines", "-p", "hgl-bindings", "-p", "hgl-describe"])
    if baseline.returncode:
        raise RuntimeError("Unmodified baseline failed:\n" + baseline.stdout)
    results = []
    for name, file, old, new, args in cases:
        path = root / file
        original = path.read_text()
        if old not in original:
            raise RuntimeError("Mutation no longer applies: " + name)
        try:
            path.write_text(original.replace(old, new, 1))
            result = run(args)
            killed = (result.returncode != 0 and "test result: FAILED" in result.stdout
                      and "could not compile" not in result.stdout)
            row = {"mutation": name, "killed": killed}
            results.append(row)
            print(json.dumps(row), flush=True)
            if not killed:
                print(result.stdout, flush=True)
        finally:
            path.write_text(original)
            path.touch()
    restored = run(["-p", "hgl-store", "-p", "hgl-nested", "-p", "hgl-deadlines", "-p", "hgl-bindings", "-p", "hgl-describe"])
    if restored.returncode:
        raise RuntimeError("Restored baseline failed:\n" + restored.stdout)
    if not all(row["killed"] for row in results):
        raise RuntimeError("A mutation survived or did not compile")
