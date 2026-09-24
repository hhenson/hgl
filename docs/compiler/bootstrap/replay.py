"""Compare literal const/debug expectations with an installed hgraph engine."""
import argparse
import hashlib
import importlib.metadata
import json
from pathlib import Path
import re
import subprocess
import sys


def run_case(case):
    from hgraph import TS, const, debug_print, graph
    from hgraph.test import eval_node

    @graph
    def source(value: int) -> TS[int]:
        ts = const(value)
        debug_print('bootstrap', ts)
        return ts

    @graph
    def sink(ts: TS[int]) -> TS[int]:
        debug_print('bootstrap', ts)
        return ts

    ticks = eval_node(source, case['value']) if case['kind'] == 'constant' else eval_node(sink, case['input'])
    # Both eval_node harnesses return None when no output ever becomes valid.
    if ticks is None and case['kind'] == 'input':
        ticks = [None] * len(case['input'])
    engine = 'cpp' if '_hgraph' in sys.modules else 'python'
    print('BOOTSTRAP_RESULT=' + json.dumps({'engine': engine, 'ticks': ticks}))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', choices=('python', 'cpp'))
    parser.add_argument('--output', type=Path)
    parser.add_argument('--case', type=int)
    args = parser.parse_args()
    path = Path(__file__).with_name('cases.json')
    cases = json.loads(path.read_text())
    if args.case is not None:
        run_case(cases[args.case])
        return
    if not args.engine or not args.output:
        parser.error('--engine and --output are required')
    observations = {}
    for index, case in enumerate(cases):
        result = subprocess.run([sys.executable, __file__, '--case', str(index)], capture_output=True, text=True, check=True)
        line = re.search(r'^BOOTSTRAP_RESULT=(.*)$', result.stdout, re.MULTILINE)
        if not line:
            raise SystemExit(result.stdout + result.stderr)
        observed = json.loads(line[1])
        if observed.pop('engine') != args.engine:
            raise SystemExit('Loaded the wrong engine')
        observed['printed'] = [int(value) for value in re.findall(r'bootstrap: (-?\d+)', result.stdout)]
        observed['matches'] = all(observed[key] == case[key] for key in ('ticks', 'printed'))
        observations[case['name']] = observed
    import hgraph
    native = sys.modules.get('_hgraph')
    report = {'engine': args.engine, 'package_version': importlib.metadata.version('hgraph'),
              'package_init_sha256': hashlib.sha256(Path(hgraph.__file__).read_bytes()).hexdigest(),
              'native_binary_sha256': hashlib.sha256(Path(native.__file__).read_bytes()).hexdigest() if native else None,
              'corpus_sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'observed': observations}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    if not all(case['matches'] for case in observations.values()):
        raise SystemExit('Reference disagreement: inspect the observations')


if __name__ == '__main__':
    main()
