"""F60: isolated end-to-end OS approval handoff against the real provider API.

No root, no installed Boss mutation, no system authority receipts. Each fixture
uses a private temporary territory and retains the provider's validation code.
"""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

provider_path = Path(__file__).with_name('platform_provider.py')
spec = importlib.util.spec_from_file_location('f60_provider', provider_path)
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


class TestOsApprovalHandoff(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='neebles-f60-')
        base = Path(self.temporary.name)
        self.candidate_root = base / 'reviews'
        self.helpers = base / 'helpers'
        self.policies = base / 'policies'
        self.journal = base / 'journal'
        for directory in (self.candidate_root, self.helpers,
                          self.policies, self.journal):
            directory.mkdir()
        self.manager = provider.Manager(self.candidate_root, self.helpers,
                                        self.policies, self.journal, strict=False)
        self.manager.approval_root.mkdir()
        self.module = 'future-capability'
        self.candidate = self.candidate_root / self.module
        (self.candidate / 'contracts').mkdir(parents=True)
        (self.candidate / 'runtime').mkdir()
        self.write_material()

    def tearDown(self):
        self.temporary.cleanup()

    def write_material(self, *, op='start', helper=b'\x7fELFfuture-v1'):
        (self.candidate / 'manifest.json').write_text(json.dumps({
            'schema': 4, 'name': self.module,
            'platform_operations': 'contracts/platform-operations.json',
        }))
        (self.candidate / 'contracts/platform-operations.json').write_text(
            json.dumps({'schema': '1', 'module': self.module, 'operations': {
                op: {'executable': str(self.helpers / (self.module + '-platform')),
                     'argv': ['--' + op]}}}))
        binary = self.candidate / 'runtime' / (self.module + '-platform')
        binary.write_bytes(helper)
        binary.chmod(0o700)

    def approve_cli(self, expected, *, confirm=True):
        args = ['approve', '--module', self.module, '--candidate-dir', str(self.candidate)]
        if confirm:
            args.append('--confirm-root-authority')
        args.extend(['--expect-manifest-sha256', expected['manifest_sha256'],
                     '--expect-contract-sha256', expected['contract_sha256'],
                     '--expect-helper-sha256', expected['helper_sha256']])
        with patch.object(provider.os, 'geteuid', return_value=0), \
             patch.object(provider, 'Manager', return_value=self.manager), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            provider.administer(args)
            return output.getvalue()

    def review_cli(self):
        with patch.object(provider.os, 'geteuid', return_value=0), \
             patch.object(provider, 'Manager', return_value=self.manager), \
             contextlib.redirect_stdout(io.StringIO()) as output:
            provider.administer(['review', '--module', self.module,
                                 '--candidate-dir', str(self.candidate)])
            return json.loads(output.getvalue())

    def test_unapproved_review_is_read_only_and_preflight_denies(self):
        expected = self.manager.fingerprint(self.module)
        review = self.review_cli()
        self.assertEqual(review['fingerprint'], expected)
        self.assertIn('start', review['operations'])
        self.assertFalse(self.manager.approval_path(self.module).exists())
        with self.assertRaises((FileNotFoundError, ValueError)):
            self.manager.preflight_approved_candidate(self.module, expected)

    def test_exact_cli_authorization_and_retry_preflight(self):
        expected = self.manager.fingerprint(self.module)
        self.assertIn('APPROVED OS privileged material', self.approve_cli(expected))
        self.assertTrue(self.manager.preflight_approved_candidate(self.module, expected))
        self.assertEqual(json.loads(self.manager.approval_path(self.module).read_text()), expected)
        self.assertFalse((self.policies / (self.module + '.json')).exists())
        self.assertFalse((self.helpers / (self.module + '-platform')).exists())

    def test_missing_separate_confirmation_writes_no_receipt(self):
        expected = self.manager.fingerprint(self.module)
        with self.assertRaises(ValueError):
            self.approve_cli(expected, confirm=False)
        self.assertFalse(self.manager.approval_path(self.module).exists())

    def test_wrong_digest_refuses_without_previous_receipt(self):
        expected = self.manager.fingerprint(self.module)
        expected['helper_sha256'] = '0' * 64
        with self.assertRaises(ValueError):
            self.approve_cli(expected)
        self.assertFalse(self.manager.approval_path(self.module).exists())

    def test_changed_candidate_denied_and_previous_receipt_unchanged(self):
        original = self.manager.fingerprint(self.module)
        self.approve_cli(original)
        binary = self.candidate / 'runtime' / (self.module + '-platform')
        binary.write_bytes(b'\x7fELFfuture-v2')
        with self.assertRaises(ValueError):
            self.approve_cli(original)
        self.assertEqual(json.loads(self.manager.approval_path(self.module).read_text()), original)
        with self.assertRaises((FileNotFoundError, ValueError)):
            self.manager.preflight_approved_candidate(self.module,
                                                       self.manager.fingerprint(self.module))

    def test_new_approval_archives_old_for_rollback(self):
        original = self.manager.fingerprint(self.module)
        self.approve_cli(original)
        self.write_material(op='stop', helper=b'\x7fELFfuture-v2')
        updated = self.manager.fingerprint(self.module)
        self.assertNotEqual(original, updated)
        self.approve_cli(updated)
        self.assertTrue(self.manager.preflight_approved_candidate(self.module, updated))
        self.assertTrue(self.manager.preflight_approved_candidate(self.module, original))
        self.assertEqual(json.loads(self.manager.approval_path(self.module).read_text()), updated)

    def test_module_ipc_cannot_approve(self):
        expected = self.manager.fingerprint(self.module)
        reply = provider.handle_control({
            'schema': '1', 'module': self.module,
            'operation': 'platform.policy.approve', 'candidate': expected,
        }, self.manager)
        self.assertFalse(reply['ok'])
        self.assertFalse(self.manager.approval_path(self.module).exists())


if __name__ == '__main__':
    unittest.main()
