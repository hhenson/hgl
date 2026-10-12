"""Cache reuse must preserve Cargo's detection of changed source and fixtures."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest
from unittest.mock import patch

import ci_build_cache as cache


class BuildCacheTests(unittest.TestCase):
    def test_restores_only_identical_existing_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / 'source.rs'
            source.write_text('old')
            old_time = source.stat().st_mtime_ns
            cache.restore(root)
            checkout_time = old_time + 10_000_000_000
            os.utime(source, ns=(checkout_time, checkout_time))
            cache.restore(root)
            self.assertEqual(source.stat().st_mtime_ns, old_time)
            source.write_text('new')
            os.utime(source, ns=(checkout_time, checkout_time))
            added = root / 'added.rs'
            added.write_text('added')
            added_time = added.stat().st_mtime_ns
            cache.restore(root)
            self.assertEqual(source.stat().st_mtime_ns, checkout_time)
            self.assertEqual(added.stat().st_mtime_ns, added_time)
            source.unlink()
            cache.restore(root)
            self.assertNotIn('source.rs', json.loads((root / cache.STATE).read_text()))

    def test_cargo_reuses_unchanged_build_and_rebuilds_changed_included_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            (root / 'Cargo.toml').write_text(
                '[package]\nname = "cache-probe"\nversion = "0.1.0"\nedition = "2024"\n')
            source = root / 'src/lib.rs'
            source.write_text('pub const VALUE: &str = include_str!("../value.hgl");\n')
            fixture = root / 'value.hgl'
            fixture.write_text('first')

            def build():
                result = subprocess.run(
                    ['cargo', 'build', '--offline', '--message-format=json',
                     '--target-dir', str(root / 'target')], cwd=root,
                    text=True, capture_output=True, check=True)
                artifacts = [json.loads(line) for line in result.stdout.splitlines()]
                return next(item['fresh'] for item in artifacts
                            if item['reason'] == 'compiler-artifact')

            cache.restore(root)
            self.assertFalse(build())
            # Emulate checkout timestamps on a new runner with restored artifacts.
            for path in (source, fixture):
                future = time.time_ns() + 2_000_000_000
                os.utime(path, ns=(future, future))
            cache.restore(root)
            self.assertTrue(build())
            fixture.write_text('other')
            cache.restore(root)
            self.assertFalse(build())
            source.write_text('pub const VALUE: &str = "changed Rust";\n')
            cache.restore(root)
            self.assertFalse(build())

    def test_environment_key_tracks_toolchain_flags_and_cargo_configuration(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'Cargo.toml'
            manifest.write_text('first')
            with patch.object(cache.subprocess, 'check_output', return_value=b'rustc one'), \
                    patch.dict(os.environ, {}, clear=True):
                original = cache.environment_key(root)
                (root / 'source.rs').write_text('source changes allow fallback reuse')
                self.assertEqual(cache.environment_key(root), original)
                with patch.dict(os.environ, {'RUSTFLAGS': '-C opt-level=1'}):
                    self.assertNotEqual(cache.environment_key(root), original)
                manifest.write_text('second')
                self.assertEqual(cache.environment_key(root), original)
                configuration = root / '.cargo/config.toml'
                configuration.parent.mkdir()
                configuration.write_text('[build]\njobs=2\n')
                self.assertNotEqual(cache.environment_key(root), original)
                configuration.unlink()
                with patch.object(cache.subprocess, 'check_output', return_value=b'rustc two'):
                    self.assertNotEqual(cache.environment_key(root), original)

    def test_reused_cache_rebuilds_changed_manifest_features_and_version(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            manifest = root / 'Cargo.toml'
            original = ('[package]\nname="cache-feature-probe"\nversion="0.1.0"\n'
                        'edition="2024"\n[features]\nsecond=[]\ndefault=[]\n')
            manifest.write_text(original)
            (root / 'src/main.rs').write_text(
                'fn main(){println!("{}:{}",env!("CARGO_PKG_VERSION"),'
                'if cfg!(feature="second"){2}else{1});}\n')

            def run():
                result = subprocess.run(
                    ['cargo', 'run', '--offline', '--quiet'], cwd=root,
                    text=True, capture_output=True, check=True)
                return result.stdout.strip()

            cache.restore(root)
            self.assertEqual(run(), '0.1.0:1')
            manifest.write_text(original.replace('default=[]', 'default=["second"]'))
            cache.restore(root)
            self.assertEqual(run(), '0.1.0:2')
            manifest.write_text(manifest.read_text().replace('0.1.0', '0.2.0'))
            cache.restore(root)
            self.assertEqual(run(), '0.2.0:2')


if __name__ == '__main__':
    unittest.main()
