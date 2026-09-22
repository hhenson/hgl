"""Compare literal description-boundary expectations with isolated observations."""
from collections import Counter
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT.parent / 'fixed'))
from check import at, equal
from harness_identity import BASE


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True).encode()).hexdigest()


def assertions(value, path=''):
    """Keep values and deltas whole; check collection sizes and every state field."""
    name = path.rsplit('/', 1)[-1]
    if name in ('value', 'delta') or not isinstance(value, (dict, list)):
        yield path, value, None
    elif isinstance(value, list):
        yield path, len(value), 'length'
        for i, child in enumerate(value):
            yield from assertions(child, path + '/' + str(i))
    else:
        if name in ('children', 'rows', 'retired'):
            yield path, sorted(value), 'keys'
        for key, child in value.items():
            yield from assertions(child, path + '/' + key)


def verify(evidence):
    p = evidence['provenance']
    assert p['harness_base'] == BASE
    for field, file in [('reasoning_sha256','reasoned.json'),('adapter_sha256','adapter.patch')]:
        assert p[field] == hashlib.sha256((ROOT / file).read_bytes()).hexdigest(), file
    for name in ['reference_identity','candidate_python_identity']:
        identity = p[name]
        assert digest({k:v for k,v in identity.items() if k != 'identity_sha256'}) == identity['identity_sha256']
    expected_cases=json.loads((ROOT / 'reasoned.json').read_text())['cases']
    assert set(evidence['cases']) == set(expected_cases), 'Every reasoned case needs replay evidence'
    for case, sides in evidence['cases'].items():
        assert set(sides) == {'python','cpp'}, 'Both runtime records are required'
        recipe = json.loads((ROOT / 'recipes' / (case + '.json')).read_text())
        fingerprint = hashlib.sha256(json.dumps(recipe,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode()).hexdigest()
        for side in sides.values():
            assert side['status'] == 'ok' and side['exception'] is None, 'Failed replay is not usable evidence'
            assert side['recipe_fingerprint'] == fingerprint
            expected = digest({k:side[k] for k in ('status','observation','exception')})
            assert side['stable'] and side['replay_digests'] == [expected]*3


def assess(reasoned, observed):
    results=[]
    if set(reasoned) != set(observed):
        raise ValueError('Every reasoned case needs replay evidence')
    for case, expected in reasoned.items():
        if set(observed[case]) != {'python','cpp'}:
            raise ValueError('Both runtime records are required')
        for path, wanted, projection in assertions(expected):
            sides={}
            for side, record in observed[case].items():
                if record.get('status') != 'ok' or record.get('exception', 'missing') is not None:
                    raise ValueError('Failed replay is not usable evidence')
                try:
                    value=at(record['observation'],path)
                    sides[side]=len(value) if projection=='length' else sorted(value) if projection=='keys' else value
                except (KeyError,IndexError,TypeError,AttributeError):
                    sides[side]={'unavailable': True}
            matches=[side for side,value in sides.items() if equal(wanted,value)]
            status=('both-agree' if len(matches)==2 else 'accepted-with-variation' if matches
                    else 'unvalidated' if any(value=={'unavailable':True} for value in sides.values())
                    else 'recheck-reasoning' if equal(sides['python'],sides['cpp']) else 'needs-decision')
            results.append(dict(case=case,path=path,projection=projection,expected=wanted,**sides,status=status))
    return results


def main():
    evidence=json.loads((ROOT/'observed.json').read_text())
    verify(evidence)
    reasoned=json.loads((ROOT/'reasoned.json').read_text())['cases']
    corrections=json.loads((ROOT/'corrections.json').read_text())
    for correction in corrections:
        parent,field=correction['path'].rsplit('/',1)
        target=at(reasoned[correction['case']],parent)
        assert equal(target[field],correction['previous']), 'Stale reasoning correction'
        target[field]=correction['expected']
    results=assess(reasoned,evidence['cases'])
    indexed={(r['case'],r['path']):r for r in results}
    for ruling in json.loads((ROOT/'rulings.json').read_text()):
        result=indexed[(ruling['case'],ruling['path'])]
        result['rule']=ruling['rule']
        result['authority']=ruling['authority']
        if result['status'] in ('unvalidated','needs-decision','recheck-reasoning'):
            result['comparison']=result['status']
            result['status']='accepted-by-existing-ruling' 
    report={'counts':dict(Counter(r['status'] for r in results)),
            'differences':[r for r in results if r['status']!='both-agree']}
    (ROOT/'assessment.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(report['counts'],sort_keys=True))
    if any(r['status'] in ('unvalidated','recheck-reasoning','needs-decision') for r in results):
        raise SystemExit(1)


if __name__=='__main__':
    main()
