"""Regression tests for atomic selection of upstream native contracts."""
import contextlib
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import native_bindings


class NativeBindingsTests(unittest.TestCase):
    def test_authoritative_paths_require_complete_pairs_before_any_work(self):
        for option in ('interface', 'implementation', 'capability-interface', 'capability-implementation'):
            with self.subTest(option=option), patch.object(native_bindings.subprocess, 'run') as run, \
                    patch.object(native_bindings.tempfile, 'TemporaryDirectory') as directory, \
                    contextlib.redirect_stderr(io.StringIO()) as errors:
                args = ['--compiler', 'unused', '--' + option, 'upstream.hgl']
                # A valid first pair must not get generated before validating the second.
                if option.startswith('capability-'):
                    args += ['--interface', 'shared.hgl', '--implementation', 'target.hgl']
                with self.assertRaises(SystemExit) as result:
                    native_bindings.main(args)
                self.assertEqual(result.exception.code, 2)
                self.assertIn('must be supplied together', errors.getvalue())
                run.assert_not_called()
                directory.assert_not_called()

    def test_paired_refresh_uses_and_vendors_selected_upstream_implementation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            shared = root / 'upstream.hgl'
            selected = root / 'upstream-impl.hgl'
            shared.write_text('native const fn audit(value: i64) -> i64\n')
            selected.write_text('native const fn audit(value: i64) -> i64 { inject logger }\n')
            local = root / 'crates/hgl-native/interfaces'
            local.mkdir(parents=True)
            (local / 'scalar.hgl').write_text(shared.read_text())
            (local / 'scalar-impl.hgl').write_text('native const fn audit(value: i64) -> i64 {}\n')
            (local / 'capabilities.hgl').write_text('local shared')
            (local / 'capabilities-impl.hgl').write_text('local implementation')
            compiled = []

            def run(command, **kwargs):
                if command[1] == 'emit-native-rust':
                    compiled.append((Path(command[2]), Path(command[4])))
                    Path(command[6]).write_text(Path(command[4]).read_text())

            with patch.object(native_bindings, 'ROOT', root), patch.object(native_bindings.subprocess, 'run', side_effect=run):
                native_bindings.main(['--compiler', 'compiler', '--interface', str(shared), '--implementation', str(selected)])
                self.assertEqual(compiled, [(shared, selected), (local / 'capabilities.hgl', local / 'capabilities-impl.hgl')])
                self.assertEqual((local / 'scalar-impl.hgl').read_text(), selected.read_text())
                self.assertEqual((root / 'crates/hgl-native/src/scalar_interface.rs').read_text(), selected.read_text())
                compiled.clear()
                native_bindings.main(['--compiler', 'compiler', '--check'])
                self.assertEqual(compiled[0], (local / 'scalar.hgl', local / 'scalar-impl.hgl'))


if __name__ == '__main__':
    unittest.main()
