"""Regenerate the scalar Rust trait using the upstream HGL compiler."""
import argparse
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--interface', type=Path, help='Authoritative upstream scalar_values_i64.hgl part')
    args = parser.parse_args()
    source = ROOT / 'crates/hgl-native/interfaces/scalar.hgl'
    upstream = args.interface.resolve() if args.interface else source
    target = ROOT / 'crates/hgl-native/src/scalar_interface.rs'
    with tempfile.TemporaryDirectory(prefix='native-interface-') as directory:
        output = Path(directory) / 'interface.rs'
        subprocess.run([str(args.compiler.resolve()), 'emit-native-rust',
                        str(upstream), '--out', str(output)], check=True)
        subprocess.run(['rustfmt', '--edition', '2024', str(output)], check=True)
        text = output.read_text()
        if args.check:
            if source.read_text() != upstream.read_text() or not target.exists() or target.read_text() != text:
                raise SystemExit('Stale native interface; regenerate with this command without --check')
        else:
            source.write_text(upstream.read_text())
            target.write_text(text)


if __name__ == '__main__':
    main()
