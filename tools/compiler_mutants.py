"""Try compiler/bootstrap regressions in a disposable copy, then restore baseline."""
import argparse
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
MUTANTS = [
    ('delayed-start', 'src/emit.rs', 'EngineDelta::from_micros(0)', 'EngineDelta::from_micros(1)', 'generated'),
    ('bare-source-fires', 'src/emit.rs', '"false".to_owned()', '"true".to_owned()', 'generated'),
    ('accept-temporal-config', 'src/check.rs', 'if kind != expected {', 'if kind == ValueKind::Void && kind != expected {', 'checking'),
    ('ignore-parameter-contract', 'src/check.rs', 'declaration.parameters == implementation.parameters',
     'declaration.parameters.len() == implementation.parameters.len()', 'checking'),
    ('suppress-equal-print', 'tests/support/runtime.rs', 'println!("{value}");',
     'static LAST: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(0); '
     'if LAST.swap(value, std::sync::atomic::Ordering::Relaxed) != value { println!("{value}"); }', 'generated'),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    observations = []
    with tempfile.TemporaryDirectory(prefix='hgl-compiler-mutants-') as directory:
        root = Path(directory) / 'hgl'
        shutil.copytree(ROOT, root, ignore=shutil.ignore_patterns('.git', 'target', '__pycache__', '.pytest_cache'))
        for name, relative, original, replacement, test in MUTANTS:
            path = root / 'crates/hgl-compiler' / relative
            text = path.read_text()
            if original not in text:
                raise SystemExit(f'Missing mutation anchor: {name}')
            path.write_text(text.replace(original, replacement, 1))
            try:
                result = subprocess.run(['cargo', 'test', '--offline', '-p', 'hgl-compiler', '--test', test],
                                        cwd=root, capture_output=True, text=True)
                killed = result.returncode != 0 and 'test result: FAILED' in result.stdout
                observations.append({'mutant': name, 'compiled_and_killed': killed})
                if not killed:
                    raise SystemExit(result.stdout + result.stderr)
            finally:
                path.write_text(text)
                path.touch()
        subprocess.run(['cargo', 'test', '--offline', '-p', 'hgl-compiler'], cwd=root, check=True,
                       stdout=subprocess.DEVNULL)
    args.output.write_text(json.dumps({'mutants': observations, 'restored_baseline': 'passed'}, indent=2) + '\n')
    print(f'{len(observations)} compiled mutants killed; restored baseline passed')


if __name__ == '__main__':
    main()
