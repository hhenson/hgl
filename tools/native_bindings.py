"""Regenerate Rust native contracts using the upstream HGL compiler."""
import argparse
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', required=True, type=Path)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--interface', type=Path, help='Authoritative upstream scalar_values_i64.hgl part')
    parser.add_argument('--capability-interface', type=Path, help='Authoritative upstream native-provider.hgl fixture')
    parser.add_argument('--implementation', type=Path, help='Authoritative scalar target implementation part')
    parser.add_argument('--capability-implementation', type=Path, help='Authoritative capability target implementation part')
    args = parser.parse_args(argv)
    for interface, implementation in (
        ('interface', 'implementation'),
        ('capability_interface', 'capability_implementation'),
    ):
        if (getattr(args, interface) is None) != (getattr(args, implementation) is None):
            parser.error(f"--{interface.replace('_', '-')} and "
                         f"--{implementation.replace('_', '-')} must be supplied together")
    contracts = [
        ('crates/hgl-native/interfaces/scalar.hgl', 'crates/hgl-native/src/scalar_interface.rs', args.interface, 'crates/hgl-native/interfaces/scalar-impl.hgl', args.implementation),
        ('crates/hgl-native/interfaces/capabilities.hgl', 'crates/hgl-native/tests/support/capability_interface.rs',
         args.capability_interface, 'crates/hgl-native/interfaces/capabilities-impl.hgl', args.capability_implementation),
    ]
    for source_name, target_name, authority, implementation_name, implementation_authority in contracts:
        source = ROOT / source_name
        upstream = authority.resolve() if authority else source
        implementation = ROOT / implementation_name
        selected = implementation_authority.resolve() if implementation_authority else implementation
        target = ROOT / target_name
        with tempfile.TemporaryDirectory(prefix='native-interface-') as directory:
            output = Path(directory) / 'interface.rs'
            subprocess.run([str(args.compiler.resolve()), 'emit-native-rust',
                            str(upstream), '--part', str(selected), '--out', str(output)], check=True)
            subprocess.run(['rustfmt', '--edition', '2024', str(output)], check=True)
            text = output.read_text()
            if args.check:
                if (source.read_text() != upstream.read_text() or implementation.read_text() != selected.read_text()
                        or not target.exists() or target.read_text() != text):
                    raise SystemExit(f'Stale native interface {source_name}; regenerate without --check')
            else:
                source.write_text(upstream.read_text())
                implementation.write_text(selected.read_text())
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text)


if __name__ == '__main__':
    main()
