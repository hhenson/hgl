"""Compile HGL eval assertions to Rust and run them on the simulation engine."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def run(args, build):
    source = build / 'src'
    source.mkdir(parents=True, exist_ok=True)
    command = ['cargo', 'run', '--quiet', '-p', 'hgl-compiler', '--bin', 'hglc', '--',
               'emit-tests', str(args.file.resolve()), '--out', str(source / 'main.rs')]
    for part in args.part:
        command += ['--part', str(part.resolve())]
    for library in args.library:
        command += ['--library', str(library.resolve())]
    subprocess.run(command, cwd=ROOT, check=True)
    with (source / 'main.rs').open('a') as file:
        file.write('\nstruct Provider;\nmod native;\n')
    if args.native_rust:
        shutil.copyfile(args.native_rust, source / 'native.rs')
    else:
        (source / 'native.rs').write_text('')
    manifest = ['[package]', 'name="hgl-eval-tests"', 'version="0.0.0"',
                'edition="2024"', '[workspace]', '[dependencies]']
    for name in ('hgl-describe', 'hgl-kernel', 'hgl-store', 'hgl-types', 'hgl-testkit', 'hgl-std-native'):
        manifest.append(f'{name} = {{ path = {json.dumps(str(ROOT / "crates" / name))} }}')
    (build / 'Cargo.toml').write_text('\n'.join(manifest) + '\n')
    subprocess.run(['cargo', 'run', '--quiet', '--offline', '--manifest-path', str(build / 'Cargo.toml')], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('file', type=Path, nargs='?')
    parser.add_argument('--stdlib', action='store_true', help='run all tests in the pinned standard library')
    parser.add_argument('--part', type=Path, action='append', default=[])
    parser.add_argument('--library', type=Path, action='append', default=[])
    parser.add_argument('--native-rust', type=Path)
    parser.add_argument('--build-dir', type=Path)
    args = parser.parse_args()
    if args.stdlib:
        if args.file or args.part or args.library or args.native_rust:
            parser.error('--stdlib selects its sources and Rust provider')
        library = ROOT / 'external/hgraph_std/hgl/hgraph'
        files = sorted((library / 'tests').glob('*.hgl'))
        if not files:
            parser.error('materialize the pinned standard library first')
        args.file, *args.part = files
        args.part.append(ROOT / 'native/stdlib/rust.hgl')
        args.library = [library]
        args.native_rust = ROOT / 'native/stdlib/native.rs'
    elif args.file is None:
        parser.error('provide FILE or --stdlib')
    if args.build_dir:
        run(args, args.build_dir.resolve())
    else:
        with tempfile.TemporaryDirectory(prefix='hgl-eval-') as directory:
            run(args, Path(directory))


if __name__ == '__main__':
    main()
