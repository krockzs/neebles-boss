"""F23: source-only regression gates for OS-independent privileged approvals."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

provider = Path(__file__).with_name('platform_provider.py')
spec = importlib.util.spec_from_file_location('neebles_os_policy_provider', provider)
m = importlib.util.module_from_spec(spec)
spec.loader.exec_module(m)


class TestManager(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.dirs = [root / x for x in ('modules', 'helpers', 'policies', 'journal')]
        for directory in self.dirs:
            directory.mkdir()
        self.engine = m.Manager(*self.dirs, strict=False)
        self.engine.approval_root.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def module(self, name='future-hardware', op='reset', code=b'\x7fELF1234'):
        root = self.dirs[0] / name
        (root / 'contracts').mkdir(parents=True, exist_ok=True)
        (root / 'runtime').mkdir(exist_ok=True)
        (root / 'manifest.json').write_text(json.dumps({
            'schema': 4,
            'name': name,
            'platform_operations': 'contracts/platform-operations.json',
        }))
        (root / 'contracts/platform-operations.json').write_text(json.dumps({
            'schema': '1',
            'module': name,
            'operations': {op: {
                'executable': str(self.dirs[1] / (name + '-platform')),
                'argv': ['--' + op],
            }},
        }))
        helper = root / 'runtime' / (name + '-platform')
        helper.write_bytes(code)
        helper.chmod(0o755)
        return root

    def test_request_is_not_approval(self):
        self.module()
        with self.assertRaises(FileNotFoundError):
            self.engine.reconcile('future-hardware')
        self.assertFalse((self.dirs[2] / 'future-hardware.json').exists())
        self.assertFalse((self.dirs[1] / 'future-hardware-platform').exists())

    def test_install_update_uninstall(self):
        self.module()
        self.engine.approve('future-hardware')
        self.assertEqual(self.engine.reconcile('future-hardware'), 'installed')
        self.assertTrue(self.engine.audit('future-hardware'))
        self.assertEqual(self.engine.reconcile('future-hardware'), 'unchanged')
        self.module('future-printer', 'print')
        self.engine.approve('future-printer')
        self.assertEqual(self.engine.reconcile('future-printer'), 'installed')
        self.assertTrue(self.engine.audit('future-hardware'))
        self.assertEqual(self.engine.reconcile('future-hardware', remove=True), 'revoked')
        self.assertFalse((self.dirs[2] / 'future-hardware.json').exists())
        self.assertTrue(self.engine.audit('future-printer'))

    def test_update_and_repair_requires_fresh_approval(self):
        root = self.module()
        self.engine.approve('future-hardware')
        self.engine.reconcile('future-hardware')
        (root / 'runtime/future-hardware-platform').write_bytes(b'\x7fELFnew')
        with self.assertRaises(ValueError):
            self.engine.audit('future-hardware')
        with self.assertRaises(ValueError):
            self.engine.reconcile('future-hardware')
        self.engine.approve('future-hardware')
        self.assertEqual(self.engine.reconcile('future-hardware'), 'installed')
        self.assertTrue(self.engine.audit('future-hardware'))

    def test_contract_change_requires_reapproval(self):
        root = self.module()
        self.engine.approve('future-hardware')
        self.engine.reconcile('future-hardware')
        path = root / 'contracts/platform-operations.json'
        data = json.loads(path.read_text())
        data['operations']['reset']['argv'] = ['--changed']
        path.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            self.engine.reconcile('future-hardware')
        self.engine.approve('future-hardware')
        self.assertEqual(self.engine.reconcile('future-hardware'), 'installed')

    def test_manifest_change_requires_reapproval(self):
        root = self.module()
        self.engine.approve('future-hardware')
        self.engine.reconcile('future-hardware')
        manifest = root / 'manifest.json'
        manifest.write_text(manifest.read_text() + '\n')
        with self.assertRaises(ValueError):
            self.engine.audit('future-hardware')

    def test_revoke_approval_invalidates_existing_policy(self):
        self.module()
        self.engine.approve('future-hardware')
        self.engine.reconcile('future-hardware')
        self.engine.revoke_approval('future-hardware')
        self.assertFalse(self.engine.approval_path('future-hardware').exists())
        self.assertFalse((self.dirs[2] / 'future-hardware.json').exists())
        self.assertFalse((self.dirs[1] / 'future-hardware-platform').exists())
        with self.assertRaises(FileNotFoundError):
            self.engine.audit('future-hardware')

    def test_approval_is_per_module(self):
        self.module('future-printer', 'print')
        self.module('future-hardware', 'reset')
        self.engine.approve('future-printer')
        with self.assertRaises(FileNotFoundError):
            self.engine.reconcile('future-hardware')
        self.assertEqual(self.engine.reconcile('future-printer'), 'installed')

    def test_approval_not_accepted_for_tampered_receipt(self):
        self.module()
        self.engine.approve('future-hardware')
        path = self.engine.approval_path('future-hardware')
        receipt = json.loads(path.read_text())
        receipt['helper_sha256'] = '0' * 64
        path.write_text(json.dumps(receipt))
        with self.assertRaises(ValueError):
            self.engine.reconcile('future-hardware')

    def test_module_ownership(self):
        root = self.module()
        contract = root / 'contracts/platform-operations.json'
        data = json.loads(contract.read_text())
        data['operations']['reset']['executable'] = str(self.dirs[1] / 'future-printer-platform')
        contract.write_text(json.dumps(data))
        with self.assertRaises(ValueError):
            self.engine.approve('future-hardware')

    def test_journal_blocks_execution_then_recovers(self):
        self.module()
        self.engine.approve('future-hardware')
        self.engine.reconcile('future-hardware')
        _, helper, policy, journal = self.engine.paths('future-hardware')
        previous = self.engine._snapshot(helper, policy)
        journal.write_text(json.dumps({'schema': '1', 'module': 'future-hardware', 'previous': previous}))
        policy.write_text('{}')
        with self.assertRaises(ValueError):
            self.engine.audit('future-hardware')
        self.assertTrue(self.engine.recover('future-hardware'))
        self.assertTrue(self.engine.audit('future-hardware'))

    def test_duplicate_keys(self):
        with self.assertRaises(ValueError):
            m.parse(b'{"schema":"1","schema":"1"}')

    def test_missing_module_does_not_grant(self):
        with self.assertRaises((FileNotFoundError, ValueError)):
            self.engine.installed_contract('ghost')

    def test_no_unexpected_ops(self):
        self.module()
        reply = m.handle_control({
            'schema': '1', 'module': 'future-hardware', 'operation': 'platform.policy.shell',
        }, self.engine)
        self.assertFalse(reply['ok'])
        self.assertFalse(self.engine.approval_path('future-hardware').exists())

    def test_control_sync_cannot_grant_without_approval(self):
        self.module()
        reply = m.handle_control({
            'schema': '1', 'module': 'future-hardware', 'operation': 'platform.policy.sync',
        }, self.engine)
        self.assertFalse(reply['ok'])
        self.assertFalse(self.engine.approval_path('future-hardware').exists())

    def test_admin_rejects_missing_root_confirmation(self):
        self.module()
        with patch.object(m.os, 'geteuid', return_value=0):
            with self.assertRaises(ValueError):
                m.administer(['approve', '--module', 'future-hardware',
                              '--candidate-dir', str(self.dirs[0] / 'future-hardware')])


if __name__ == '__main__':
    unittest.main(verbosity=2)
