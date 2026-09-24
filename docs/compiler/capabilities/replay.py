"""Reference helper calls with explicit forwarding; HGL infers that forwarding."""
import argparse
import importlib.metadata
import json
import hashlib
from datetime import datetime
import sys
from pathlib import Path
import hgraph
from hgraph import TS, LOGGER, EvaluationClock, MIN_ST, MIN_TD, compute_node
from hgraph.test import eval_node

calls = []


def leaf(value, logger):
    calls.append(value)
    logger.info('capability helper')
    return value


def middle(value, logger):
    return leaf(value, logger)


@compute_node
def caller(value: TS[int], logger: LOGGER = None) -> TS[int]:
    return middle(value.value, logger)


def clock_helper(clock):
    return clock.evaluation_time


@compute_node
def stamped(value: TS[int], clock: EvaluationClock = None) -> TS[datetime]:
    return clock_helper(clock)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--engine', choices=('python', 'cpp'), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    engine = 'cpp' if '_hgraph' in sys.modules else 'python'
    assert engine == args.engine, (engine, args.engine)
    ticks = eval_node(caller, [2, None, 2, 3])
    clock_ticks = eval_node(stamped, [2, None, 2, 3])
    offsets = [None if tick is None else (tick - MIN_ST) // MIN_TD for tick in clock_ticks]
    native = sys.modules.get('_hgraph')
    report = {'package_sha256': hashlib.sha256(Path(hgraph.__file__).read_bytes()).hexdigest(),
              'native_sha256': hashlib.sha256(Path(native.__file__).read_bytes()).hexdigest() if native else None,
              'clock_offsets': offsets, 'expected_clock_offsets': [0, None, 2, 3], 'engine': engine, 'version': importlib.metadata.version('hgraph'),
              'ticks': ticks, 'helper_calls': calls,
              'expected_ticks': [2, None, 2, 3], 'expected_helper_calls': [2, 2, 3]}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    assert ticks == report['expected_ticks']
    assert calls == report['expected_helper_calls']
    assert offsets == report['expected_clock_offsets']


if __name__ == '__main__':
    main()
