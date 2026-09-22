"""Export independent accepted expectations, never runtime observations."""
import json
import sys
import runpy
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'docs/runtime_spec/validation/descriptions'
sys.path.insert(0, str(SOURCE))
checker = runpy.run_path(str(SOURCE / 'check.py'))
assertions, at = checker['assertions'], checker['at']


def render():
    cases = json.loads((SOURCE / 'reasoned.json').read_text())['cases']
    for correction in json.loads((SOURCE / 'corrections.json').read_text()):
        parent, field = correction['path'].rsplit('/', 1)
        target = at(cases[correction['case']], parent)
        assert target[field] == correction['previous']
        target[field] = correction['expected']
    return ''.join('\t'.join((case, path, projection or '', json.dumps(value, sort_keys=True, separators=(',', ':')))) + '\n'
                   for case, trace in cases.items() for path, value, projection in assertions(trace))


if __name__ == '__main__':
    target = ROOT / 'crates/hgl-describe/tests/recursive_support/accepted.tsv'
    content = render()
    if '--check' in sys.argv:
        assert target.read_text() == content, 'Regenerate with tools/description_fixtures.py'
    else:
        target.write_text(content)
