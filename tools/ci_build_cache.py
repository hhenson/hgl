"""Reuse Cargo artifacts across checkouts without hiding changed build inputs."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


EXCLUDED = {'.git', 'target', '__pycache__', '.venv', '.idea', '.vscode'}
STATE = Path('target/ci-source-mtimes.json')


def inputs(root):
    for directory, children, files in os.walk(root):
        children[:] = sorted(name for name in children if name not in EXCLUDED)
        for name in sorted(files):
            path = Path(directory) / name
            if name != '.git' and not path.is_symlink():
                yield path


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def restore(root):
    state = root / STATE
    previous = json.loads(state.read_text()) if state.exists() else {}
    current = {}
    restored = 0
    for path in inputs(root):
        name = path.relative_to(root).as_posix()
        checksum = digest(path)
        stat = path.stat()
        old = previous.get(name)
        # Checkout timestamps alone must not invalidate identical source bytes.
        if old and old['sha256'] == checksum:
            os.utime(path, ns=(stat.st_atime_ns, old['mtime_ns']))
            restored += 1
        current[name] = {'sha256': checksum, 'mtime_ns': path.stat().st_mtime_ns}
    # Snapshot before compilation: later source edits must invalidate artifacts.
    state.parent.mkdir(parents=True, exist_ok=True)
    state.write_text(json.dumps(current, sort_keys=True))
    print(f'Restored timestamps for {restored}/{len(current)} unchanged inputs')


def environment_key(root):
    compiler = subprocess.check_output(['rustc', '-vV'])
    environment = {name: value for name, value in sorted(os.environ.items())
                   if name.startswith(('CARGO', 'RUST', 'CC', 'CXX', 'CFLAGS', 'CMAKE'))}
    manifests = {path.relative_to(root).as_posix(): digest(path) for path in inputs(root)
                 if path.name in {'Cargo.toml', 'Cargo.lock', 'rust-toolchain',
                                  'rust-toolchain.toml'} or '.cargo' in path.parts}
    return hashlib.sha256(compiler + json.dumps([environment, manifests],
                                               sort_keys=True).encode()).hexdigest()


def main():
    root = Path(__file__).resolve().parents[1]
    if sys.argv[1:] == ['key']:
        with open(os.environ['GITHUB_OUTPUT'], 'a', encoding='utf-8') as output:
            output.write(f'key={environment_key(root)}\n')
    elif sys.argv[1:] == ['restore']:
        restore(root)
    else:
        raise SystemExit('usage: ci_build_cache.py key|restore')


if __name__ == '__main__':
    main()
