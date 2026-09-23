"""The accepted port traces and fixtures must stay tied to their evidence."""
import hashlib
import json
from pathlib import Path
import unittest

HERE = Path(__file__).resolve().parent


class Evidence(unittest.TestCase):
    def test_independent_engines_match_reasoned_ticks(self):
        corpus = HERE / 'cases.json'
        cases = json.loads(corpus.read_text())
        expected = {case['name']: case['expected'] for case in cases}
        self.assertEqual(len(expected), len(cases))
        for engine in ('python', 'cpp'):
            with self.subTest(engine=engine):
                evidence = json.loads((HERE / f'{engine}.json').read_text())
                self.assertEqual(evidence['engine'], engine)
                self.assertEqual(evidence['corpus_sha256'], hashlib.sha256(corpus.read_bytes()).hexdigest())
                self.assertEqual(set(evidence['observed']), set(expected))
                self.assertEqual(bool(evidence['native_binary_sha256']), engine == 'cpp')
                for name, ticks in expected.items():
                    self.assertEqual(evidence['observed'][name]['ticks'], ticks)
                    self.assertTrue(evidence['observed'][name]['matches'])

    def test_hgl_check_is_not_reported_as_runtime_execution(self):
        evidence = json.loads((HERE / 'hgl-reference.json').read_text())
        self.assertEqual(evidence['check_exit_code'], 0)
        self.assertEqual(evidence['runtime_test_status'], 'blocked')
        self.assertNotEqual(evidence['runtime_test_exit_code'], 0)
        for name, digest in evidence['checked_source_sha256'].items():
            self.assertEqual(hashlib.sha256((HERE / name).read_bytes()).hexdigest(), digest)


if __name__ == '__main__':
    unittest.main()
