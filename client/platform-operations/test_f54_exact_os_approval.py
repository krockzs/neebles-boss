"""F54: independent OS review and explicit triple-SHA administrative approval."""
import importlib.util
import json
import pathlib
import tempfile
import unittest
from unittest.mock import patch

HERE = pathlib.Path(__file__).parent
spec = importlib.util.spec_from_file_location('platform_provider_f54', HERE / 'platform_provider.py')
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


class ExactApproval(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = pathlib.Path(self.tmp.name)
        modules = root / 'modules'
        helpers = root / 'helpers'
        policy = root / 'policy'
        journal = root / 'journal'
        for p in (modules, helpers, policy, journal):
            p.mkdir()
        self.m = provider.Manager(modules, helpers, policy, journal, strict=False)
        self.m.approval_root.mkdir()
        self.name = 'generic-module'
        p = modules / self.name
        (p/'contracts').mkdir(parents=True)
        (p/'runtime').mkdir()
        (p/'manifest.json').write_text(json.dumps({'schema': 4, 'name': self.name,
                            'platform_operations':'contracts/platform-operations.json'}))
        (p/'contracts/platform-operations.json').write_text(json.dumps({
            'schema':'1', 'module':self.name, 'operations':{'operate':{
                'executable':str(helpers/(self.name+'-platform')), 'argv':['--fixed']}}}))
        self.elf = p/'runtime'/(self.name+'-platform')
        self.elf.write_bytes(b'\x7fELFalpha')
        self.elf.chmod(0o755)
        self.p = p

    def tearDown(self):
        self.tmp.cleanup()

    def test_review_is_read_only_and_contains_fixed_operations(self):
        report = self.m.review_candidate(self.name)
        self.assertEqual(report['fingerprint'], self.m.fingerprint(self.name))
        self.assertIn('operate', report['operations'])
        self.assertFalse(self.m.approval_path(self.name).exists())
        self.assertEqual(list(self.m.approval_root.iterdir()), [])

    def test_approved_exact_hashes_pass(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        self.assertEqual(original, self.m.approve_exact_candidate(self.name, original))
        self.assertTrue(self.m.preflight_approved_candidate(self.name, original))

    def test_wrong_digest_denies_without_receipt(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        modified = dict(original, helper_sha256='f'*64)
        with self.assertRaisesRegex(ValueError, 'changed'):
            self.m.approve_exact_candidate(self.name, modified)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_changed_helper_between_review_and_approve_denied(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        self.elf.write_bytes(b'\x7fELFbeta')
        with self.assertRaisesRegex(ValueError, 'changed'):
            self.m.approve_exact_candidate(self.name, original)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_changed_contract_between_review_and_approve_denied(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        path = self.p/'contracts/platform-operations.json'
        path.write_text(path.read_text().replace('--fixed','--changed'))
        with self.assertRaisesRegex(ValueError, 'changed'):
            self.m.approve_exact_candidate(self.name, original)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_approval_does_not_change_root_policy_until_sync(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        self.m.approve_exact_candidate(self.name, original)
        self.assertEqual(list(self.m.policy_root.glob('*.json')), [])
        self.assertEqual(list(self.m.helpers_root.glob('*')), [])

    def test_pending_policy_journal_denied(self):
        original = self.m.fingerprint(self.name)
        path = self.m.journal_root / (self.name+'.json')
        path.write_text('{}')
        with self.assertRaisesRegex(ValueError,'unreconciled'):
            self.m.approve_exact_candidate(self.name, original)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_previous_receipt_survives_explicit_new_version(self):
        original = self.m.review_candidate(self.name)['fingerprint']
        self.m.approve_exact_candidate(self.name, original)
        self.elf.write_bytes(b'\x7fELFbeta')
        updated = self.m.review_candidate(self.name)['fingerprint']
        self.m.approve_exact_candidate(self.name, updated)
        self.assertTrue(self.m.approved_fingerprint(self.name, original))
        self.assertTrue(self.m.approved_fingerprint(self.name, updated))

    def test_invalid_module_identity_fails_closed(self):
        with self.assertRaises(ValueError):
            self.m.review_candidate('../escape')

    def test_root_boundary_remains_root_only(self):
        original = self.m.fingerprint(self.name)
        self.m.strict = True
        with patch.object(provider.os, 'geteuid', return_value=1000):
            with self.assertRaisesRegex(ValueError, 'root'):
                self.m.approve_exact_candidate(self.name, original)

    def test_cli_missing_confirmation_denied_before_read(self):
        # Existing F23 regression expects ValueError, not argparse SystemExit.
        with patch.object(provider.os, 'geteuid', return_value=0):
            with self.assertRaisesRegex(ValueError, 'confirm-root-authority'):
                provider.administer(['approve', '--module', self.name,
                    '--candidate-dir', str(self.p)])
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_cli_missing_expected_hashes_denied_before_read(self):
        with patch.object(provider.os, 'geteuid', return_value=0):
            with self.assertRaisesRegex(ValueError, 'three SHA-256'):
                provider.administer(['approve', '--module', self.name,
                    '--candidate-dir', str(self.p), '--confirm-root-authority'])
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_missing_receipt_yields_typed_challenge_without_writes(self):
        expected = self.m.fingerprint(self.name)
        reply = provider.handle_control({'schema':'1','module':self.name,
                      'operation':'platform.policy.preflight','candidate':expected},self.m)
        self.assertFalse(reply['ok'])
        self.assertTrue(reply.get('approval_required'))
        self.assertEqual(reply['fingerprint'], expected)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_mismatched_receipt_yields_typed_challenge(self):
        before = self.m.fingerprint(self.name)
        self.m.approve_exact_candidate(self.name, before)
        self.elf.write_bytes(b'\x7fELFbeta')
        after = self.m.fingerprint(self.name)
        reply = provider.handle_control({'schema':'1','module':self.name,
                      'operation':'platform.policy.preflight','candidate':after},self.m)
        self.assertFalse(reply['ok'])
        self.assertTrue(reply.get('approval_required'))
        self.assertEqual(reply['fingerprint'], after)
        self.assertTrue(self.m.approved_fingerprint(self.name, before))

    def test_pending_transaction_never_yields_approval_challenge(self):
        expected = self.m.fingerprint(self.name)
        (self.m.journal_root/(self.name+'.json')).write_text('{}')
        reply = provider.handle_control({'schema':'1','module':self.name,
                      'operation':'platform.policy.preflight','candidate':expected},self.m)
        self.assertFalse(reply['ok'])
        self.assertNotIn('approval_required', reply)

    def test_invalid_candidate_never_yields_approval_challenge(self):
        reply = provider.handle_control({'schema':'1','module':self.name,
                      'operation':'platform.policy.preflight','candidate':{'schema':'1'}},self.m)
        self.assertFalse(reply['ok'])
        self.assertNotIn('approval_required', reply)

    def test_exact_approval_preflight_second_fingerprint_is_checked_before_write(self):
        expected = self.m.fingerprint(self.name)
        original = self.m.fingerprint
        count = [0]
        def mutate_at_second_fingerprint(module):
            count[0] += 1
            if count[0] == 2:
                self.elf.write_bytes(b'\x7fELFchanged-on-second-read')
            return original(module)
        with patch.object(self.m, 'fingerprint', side_effect=mutate_at_second_fingerprint):
            with self.assertRaisesRegex(ValueError, 'changed'):
                self.m.approve_exact_candidate(self.name, expected)
        self.assertFalse(self.m.approval_path(self.name).exists())

    def test_no_provider_socket_approval_operation(self):
        original = self.m.fingerprint(self.name)
        reply = provider.handle_control({'schema':'1', 'module':self.name,
                      'operation':'platform.policy.approve','candidate':original},self.m)
        self.assertFalse(reply['ok'])
        self.assertFalse(self.m.approval_path(self.name).exists())


if __name__ == '__main__':
    unittest.main(verbosity=2)
