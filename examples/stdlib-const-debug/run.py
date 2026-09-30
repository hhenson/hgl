"""Compile the HGL example, build the generated Rust crate, and run it."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
EXAMPLE = Path(__file__).resolve().parent


def run(output):
    output = output.resolve()
    (output / 'src').mkdir(parents=True, exist_ok=True)
    subprocess.run(['cargo', 'run', '--quiet', '-p', 'hgl-compiler', '--bin', 'hglc', '--',
                    'emit-rust', str(EXAMPLE / 'main.hgl'),
                    '--library', str(ROOT / 'external/hgraph_std/hgl/hgraph'), '--part', str(EXAMPLE / 'rust.hgl'),
                    '--out', str(output / 'src/generated.rs')], cwd=ROOT, check=True)
    subprocess.run(['rustfmt', '--edition', '2024', str(output / 'src/generated.rs')], check=True)
    for source in ('main.rs', 'native.rs'):
        shutil.copyfile(EXAMPLE / source, output / 'src' / source)
    manifest = ['[package]', 'name = "stdlib-const-debug-example"', 'version = "0.0.0"',
                'edition = "2024"', '[workspace]', '[dependencies]']
    for name in ('hgl-describe', 'hgl-kernel', 'hgl-store', 'hgl-types'):
        manifest.append(f'{name} = {{ path = {json.dumps(str(ROOT / "crates" / name))} }}')
    (output / 'Cargo.toml').write_text('\n'.join(manifest) + '\n')
    subprocess.run(['cargo', 'run', '--offline', '--quiet', '--manifest-path', str(output / 'Cargo.toml')], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out-dir', type=Path, help='Keep the generated crate here for inspection')
    args = parser.parse_args()
    if args.out_dir:
        run(args.out_dir)
    else:
        with tempfile.TemporaryDirectory(prefix='hgl-stdlib-const-debug-') as directory:
            run(Path(directory))


if __name__ == '__main__':
    main()
