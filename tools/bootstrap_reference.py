"""Check bootstrap evidence and optional upstream source identity."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--upstream', type=Path)
    args = parser.parse_args()
    directory = ROOT / 'docs/compiler/bootstrap'
    corpus = directory / 'cases.json'
    cases = json.loads(corpus.read_text())
    digest = hashlib.sha256(corpus.read_bytes()).hexdigest()
    for engine in ('python', 'cpp'):
        report = json.loads((directory / f'{engine}.json').read_text())
        if report['engine'] != engine or report['corpus_sha256'] != digest:
            raise SystemExit(f'Stale {engine} bootstrap evidence')
        if set(report['observed']) != {case['name'] for case in cases}:
            raise SystemExit(f'Incomplete {engine} bootstrap evidence')
        for case in cases:
            observed = report['observed'][case['name']]
            if not all(observed[key] == case[key] for key in ('ticks', 'printed')):
                raise SystemExit(f'{engine} disagrees on {case["name"]}')
    if args.upstream:
        for local, source in (
            ('main.hgl', 'language/examples/const-debug.hgl'),
            ('rust.hgl', 'language/examples/impl/const-debug-rust.hgl'),
        ):
            if (ROOT / 'examples/const-debug' / local).read_bytes() != (args.upstream / source).read_bytes():
                raise SystemExit(f'Bootstrap source differs from upstream: {local}')
    print('Bootstrap source/evidence checks passed')


if __name__ == '__main__':
    main()
