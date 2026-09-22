"""Acceptance cannot manufacture a missing observation or hide disagreement."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('description_check', ROOT / 'check.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class Comparisons(unittest.TestCase):
    def result(self, python, cpp):
        return check.assess({'case':{'value':7}}, {'case':{
            'python':{'status':'ok','exception':None,'observation':python}, 'cpp':{'status':'ok','exception':None,'observation':cpp}}})[0]

    def test_one_reference_plus_reasoning_is_sufficient(self):
        result=self.result({'value':7},None)
        self.assertEqual(result['status'],'accepted-with-variation')
        self.assertEqual(result['cpp'],{'unavailable':True})

    def test_agreeing_references_require_reasoning_review(self):
        self.assertEqual(self.result({'value':8},{'value':8})['status'],'recheck-reasoning')

    def test_three_distinct_results_require_decision(self):
        self.assertEqual(self.result({'value':8},{'value':9})['status'],'needs-decision')

    def test_missing_reference_is_never_a_vote(self):
        self.assertEqual(self.result(None,{'value':8})['status'],'unvalidated')

    def test_values_include_unexpected_fields(self):
        rows=list(check.assertions({'value':{'left':7},'children':{'left':{'valid':True}}}))
        self.assertIn(('/value',{'left':7},None),rows)
        self.assertIn(('/children',['left'],'keys'),rows)

    def test_complete_failed_trace_is_rejected(self):
        evidence=json.loads((ROOT/'observed.json').read_text())
        record=evidence['cases']['owned']['python']
        for status, exception in [('error',None), ('ok',{'type':'RuntimeError'})]:
            record.update(status=status,exception=exception)
            record['replay_digests']=[check.digest({k:record[k] for k in ('status','observation','exception')})]*3
            with self.assertRaises(AssertionError):
                check.verify(evidence)
            with self.assertRaises(ValueError):
                check.assess({'owned':{'events':['start','stop','start','stop']}},{'owned':evidence['cases']['owned']})

    def test_missing_or_extra_runtime_is_rejected(self):
        original=json.loads((ROOT/'observed.json').read_text())
        for side in ['python','cpp','extra']:
            evidence=copy.deepcopy(original)
            if side=='extra':
                evidence['cases']['owned'][side]=evidence['cases']['owned']['python']
            else:
                del evidence['cases']['owned'][side]
            with self.assertRaises(AssertionError):
                check.verify(evidence)
            with self.assertRaises(ValueError):
                check.assess({'owned':{'value':7}},{'owned':evidence['cases']['owned']})

    def test_missing_case_is_rejected(self):
        evidence=json.loads((ROOT/'observed.json').read_text())
        del evidence['cases']['owned']
        with self.assertRaises(AssertionError):
            check.verify(evidence)
        with self.assertRaises(ValueError):
            check.assess({'owned':{'value':7}},evidence['cases'])

    def test_evidence_is_untampered(self):
        evidence=json.loads((ROOT/'observed.json').read_text())
        check.verify(evidence)
        changed=copy.deepcopy(evidence)
        changed['cases']['owned']['python']['observation']['events']=[]
        with self.assertRaises(AssertionError):
            check.verify(changed)


if __name__=='__main__':
    unittest.main()
