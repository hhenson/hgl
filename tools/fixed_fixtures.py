"""Export accepted assertions without reading runtime observations."""
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'docs/runtime_spec/validation/fixed'
sys.path.insert(0, str(SOURCE))
from expand import assertions
from check import apply_decisions


def render():
    rows = assertions()
    for row in rows:
        for correction in json.loads((SOURCE / 'reasoning_corrections.json').read_text()):
            if row['case'] in correction['cases'] and row['path'] == correction['path']:
                assert row['expected'] == correction['initial']
                row['expected'] = correction['expected']
    apply_decisions(rows, json.loads((SOURCE / 'decisions.json').read_text()))
    return ''.join('\t'.join((r['case'], r['path'], r.get('projection', ''), json.dumps(r['expected'], sort_keys=True, separators=(',', ':')))) + '\n' for r in rows)


if __name__ == '__main__':
    target = ROOT / 'crates/hgl-store/tests/fixed_support/accepted.tsv'
    content = render()
    if '--check' in sys.argv:
        assert target.read_text() == content, 'Regenerate accepted fixtures with tools/fixed_fixtures.py'
    else:
        target.write_text(content)
