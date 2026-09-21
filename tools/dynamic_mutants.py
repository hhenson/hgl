"""Run focused runtime mutants on a fresh, disposable copy of this checkout."""
from pathlib import Path
import json
import shutil
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[1]
cases = [('expired_dictionary_projection',
  'crates/hgl-bindings/src/lib.rs',
  'self.clear_projection(i);',
  '// deliberately retain expired child views',
  ['-p', 'hgl-store', '--test', 'dynamic', 'expired_dictionary']),
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
  'crates/hgl-bindings/src/lib.rs',
  'self.inputs[input.0 as usize].sampled_at = now;',
  'self.inputs[input.0 as usize].sampled_at = EngineTime::NEVER;',
  ['-p', 'hgl-store', '--test', 'dynamic']),
 ('ancestor_wake',
  'crates/hgl-bindings/src/scopes.rs',
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
  'if false {',
  ['-p', 'hgl-store', '--test', 'dynamic'])]

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

    baseline = run(["-p", "hgl-store", "-p", "hgl-nested", "-p", "hgl-deadlines", "-p", "hgl-bindings"])
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
    restored = run(["-p", "hgl-store", "-p", "hgl-nested", "-p", "hgl-deadlines", "-p", "hgl-bindings"])
    if restored.returncode:
        raise RuntimeError("Restored baseline failed:\n" + restored.stdout)
    if not all(row["killed"] for row in results):
        raise RuntimeError("A mutation survived or did not compile")
