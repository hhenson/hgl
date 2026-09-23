"""Run the reasoned node cases against an installed Python or C++ hgraph."""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import sys

import hgraph
from hgraph import TS, graph, bit_and, sample, dedup
from hgraph.test import eval_node


@graph
def bit_and_i64(lhs: TS[int], rhs: TS[int]) -> TS[int]:
    return bit_and(lhs, rhs)


@graph
def sample_i64(signal: TS[int], ts: TS[int]) -> TS[int]:
    return sample(signal, ts)


@graph
def dedup_i64(ts: TS[int]) -> TS[int]:
    return dedup(ts)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', choices=('python', 'cpp'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    engine = 'cpp' if '_hgraph' in sys.modules else 'python'
    if engine != args.engine:
        raise SystemExit(f'Expected {args.engine}, loaded {engine}')
    corpus = Path(__file__).with_name('cases.json')
    cases = json.loads(corpus.read_text())
    nodes = {'bit_and_i64': bit_and_i64, 'sample_i64': sample_i64, 'dedup_i64': dedup_i64}
    observed = {}
    for case in cases:
        ticks = eval_node(nodes[case['node']], **case['inputs'])
        observed[case['name']] = {'ticks': ticks, 'matches': ticks == case['expected']}
    native = sys.modules.get('_hgraph')
    binary = Path(native.__file__) if native else None
    report = {
        'engine': engine,
        'package_version': importlib.metadata.version('hgraph'),
        'package_init_sha256': hashlib.sha256(Path(hgraph.__file__).read_bytes()).hexdigest(),
        'native_binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest() if binary else None,
        'corpus_sha256': hashlib.sha256(corpus.read_bytes()).hexdigest(),
        'observed': observed,
    }
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    if not all(item['matches'] for item in observed.values()):
        raise SystemExit('Trace disagreement: inspect the report')


if __name__ == '__main__':
    main()
