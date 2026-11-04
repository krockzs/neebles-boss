"""Boss 2.0.14 F41: root administrative provenance and OS approval preflight."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

PATH = Path(__file__).with_name('platform_provider.py')
spec = importlib.util.spec_from_file_location('f41_provider', PATH)
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


class RootAuthorityGates(unittest.TestCase):
    def test_untrusted_peer_rejected(self):
        self.assertFalse(provider.authorized_boss_peer(
            123, 1000, 0, '/usr/lib/neebles/authorized',
            peer_verifier=lambda pid, executable: True))
        self.assertFalse(provider.authorized_boss_peer(
            123, 0, 0, '/usr/lib/neebles/authorized',
            peer_verifier=lambda pid, executable: False))
        self.assertFalse(provider.authorized_boss_peer(
            123, 0, 0, None,
            peer_verifier=lambda pid, executable: True))

    def test_runtime_cgroup_still_allowed(self):
        with patch.object(provider, 'is_boss_runtime_process', return_value=True):
            self.assertTrue(provider.authorized_boss_peer(
                123, 0, 0, '/usr/lib/neebles/authorized',
                peer_verifier=lambda pid, executable: True))

    def test_root_cli_requires_exact_action_and_all_root_uids(self):
        class FakeProc:
            def read_text(self):
                return 'Name:\tneebles-backend\nUid:\t0\t0\t0\t0\n'

            def read_bytes(self):
                return b'/opt/neebles/client/backend/neebles-backend\0modules\0update\0example-module\0'
        with patch.object(provider, 'Path', return_value=FakeProc()):
            self.assertTrue(provider.is_authorized_root_boss_administrator(200))
            with patch.object(provider, 'is_boss_runtime_process', return_value=False):
                self.assertTrue(provider.authorized_boss_peer(
                    200, 0, 0, '/opt/neebles/client/backend/neebles-backend',
                    peer_verifier=lambda pid, executable: True))

        class FakeNonRoot(FakeProc):
            def read_text(self):
                return 'Uid:\t1000\t0\t0\t0\n'
        with patch.object(provider, 'Path', return_value=FakeNonRoot()):
            self.assertFalse(provider.is_authorized_root_boss_administrator(200))

        class FakeWrongAction(FakeProc):
            def read_bytes(self):
                return b'/opt/neebles/client/backend/neebles-backend\0modules\0available\0example-module\0'
        with patch.object(provider, 'Path', return_value=FakeWrongAction()):
            self.assertFalse(provider.is_authorized_root_boss_administrator(200))


class ReadOnlyPreflight(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        home = Path(self.tmp.name)
        self.roots = [home / part for part in ('modules', 'helpers', 'policies', 'journal')]
        for directory in self.roots:
            directory.mkdir()
        self.manager = provider.Manager(*self.roots, strict=False)
        self.manager.approval_root.mkdir()
        self.name = 'future-hardware'
        directory = self.roots[0] / self.name
        (directory / 'contracts').mkdir(parents=True)
        (directory / 'runtime').mkdir()
        (directory / 'manifest.json').write_text(json.dumps({
            'schema':4,'name':self.name,'platform_operations':'contracts/platform-operations.json'}))
        (directory / 'contracts/platform-operations.json').write_text(json.dumps({
            'schema':'1','module':self.name,'operations':{
                'reset':{'executable':str(self.roots[1] / (self.name + '-platform')),'argv':['--reset']}}}))
        (directory / 'runtime' / (self.name+'-platform')).write_bytes(b'\x7fELFtests')
        (directory / 'runtime' / (self.name+'-platform')).chmod(0o755)

    def tearDown(self):
        self.tmp.cleanup()

    def request(self, candidate):
        return provider.handle_control({'schema':'1','module':self.name,
                                        'operation':'platform.policy.preflight',
                                        'candidate':candidate}, self.manager)

    def test_missing_approval_is_fail_closed_and_read_only(self):
        digest = self.manager.fingerprint(self.name)
        self.assertFalse(self.request(digest)['ok'])
        self.assertFalse(self.manager.approval_path(self.name).exists())
        self.assertEqual(list(self.roots[2].glob('*.json')), [])
        self.assertEqual(list(self.roots[1].glob('*')), [])

    def test_approval_required_exact_identity(self):
        digest = self.manager.approve(self.name)
        self.assertTrue(self.request(digest)['ok'])
        other = dict(digest)
        other['helper_sha256'] = 'a'*64
        self.assertFalse(self.request(other)['ok'])
        self.assertFalse(self.request({'schema':'1'})['ok'])
        self.assertEqual(list(self.roots[2].glob('*.json')), [])
        self.assertEqual(list(self.roots[1].glob('*')), [])

    def test_no_approval_receipts_preserves_missing_file_contract(self):
        # No active or historical OS approval means an absent authorization.
        # This must remain distinguishable from a different active approval.
        candidate = self.manager.fingerprint(self.name)
        with self.assertRaises(FileNotFoundError):
            self.manager.approved_fingerprint(self.name, candidate)
        self.assertFalse(self.request(candidate)['ok'])
        self.assertEqual(list(self.manager.approval_root.glob('*.json')), [])

    def test_absent_historical_approval_is_explicit_denial(self):
        # A valid but different active receipt cannot authorize new bytes.
        original = self.manager.approve(self.name)
        unknown = dict(original)
        unknown['helper_sha256'] = 'f' * 64
        self.assertNotEqual(unknown, original)
        with self.assertRaisesRegex(ValueError, 'OS approval missing'):
            self.manager.approved_fingerprint(self.name, unknown)
        self.assertFalse(self.request(unknown)['ok'])
        self.assertFalse(self.manager.archived_approval_path(self.name, unknown).exists())
        # A missing historical receipt does not alter the approved current one.
        self.assertTrue(self.request(original)['ok'])

    def test_removal_preflight_requires_no_new_approval(self):
        self.assertTrue(self.request(None)['ok'])
        self.assertFalse(self.manager.approval_path(self.name).exists())

    def test_previous_approved_bytes_survive_new_version_approval(self):
        original = self.manager.approve(self.name)
        self.manager.reconcile(self.name)
        helper = self.roots[0] / self.name / 'runtime' / (self.name + '-platform')
        helper.write_bytes(b'\x7fELFnew-version')
        newer = self.manager.approve(self.name)
        self.assertNotEqual(original, newer)
        self.assertTrue(self.manager.approved_fingerprint(self.name, original))
        self.assertTrue(self.request(original)['ok'])
        self.assertTrue(self.request(newer)['ok'])
        self.manager.reconcile(self.name)
        helper.write_bytes(b'\x7fELFtests')
        self.assertTrue(self.manager.approval(self.name))
        self.manager.reconcile(self.name)

    def test_revoke_removes_old_and_current_approvals(self):
        self.manager.approve(self.name)
        helper = self.roots[0] / self.name / 'runtime' / (self.name + '-platform')
        helper.write_bytes(b'\x7fELFnew-version')
        self.manager.approve(self.name)
        self.manager.revoke_approval(self.name)
        self.assertEqual(list(self.manager.approval_root.glob('*.json')), [])
        self.assertFalse(self.request(self.manager.fingerprint(self.name))['ok'])


if __name__ == '__main__':
    unittest.main(verbosity=2)
