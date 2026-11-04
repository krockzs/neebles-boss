"""F58: UI/root CLI boundary regressions; source-only, no OS mutations."""
from pathlib import Path
import json
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]
CPP = ROOT / 'ui/client/bosscontroller.cpp'
H = ROOT / 'ui/client/bosscontroller.h'
QML = ROOT / 'ui/client/qml/Main.qml'

class TestIndependentApprovalUi(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cpp = CPP.read_text()
        cls.header = H.read_text()
        cls.qml = QML.read_text()

    def test_approval_is_user_invokable_but_not_silent(self):
        self.assertIn('Q_INVOKABLE void approvePendingPlatformOperation();', self.header)
        self.assertEqual(self.cpp.count('void BossController::approvePendingPlatformOperation()'), 1)
        start = self.cpp.index('void BossController::startModuleProcess(')
        end = self.cpp.index('void BossController::runModuleOperation(')
        self.assertNotIn('approvePendingPlatformOperation(', self.cpp[start:end])
        self.assertIn('onClicked: {\n                        osApprovalDialog.close()\n                        boss.approvePendingPlatformOperation()', self.qml)

    def test_approval_root_cli_requires_exact_digest_and_confirmation(self):
        method = self.cpp.split('void BossController::approvePendingPlatformOperation()', 1)[1].split('void BossController::uninstallModule(', 1)[0]
        for part in ('--confirm-root-authority', '--expect-manifest-sha256',
                     '--expect-contract-sha256', '--expect-helper-sha256',
                     '--candidate-dir', 'm_platformApprovalProcess',
                     '/usr/lib/neebles/platform-operations/provider.py',
                     '/var/lib/neebles/platform-operations/reviews/'):
            self.assertIn(part, method)
        self.assertIn('!tx.value(QStringLiteral("approvalReviewReady")).toBool()', method)
        self.assertIn('QRegularExpression digestPattern', method)
        self.assertIn('process->start(authAgent, args);', method)

    def test_fail_closed_and_restart_after_success_only(self):
        method = self.cpp.split('void BossController::approvePendingPlatformOperation()', 1)[1].split('void BossController::uninstallModule(', 1)[0]
        self.assertIn('if (!approved)', method)
        denied = method.index('if (!approved)')
        retry = method.index('startModuleProcess(operation, module, true,')
        self.assertLess(denied, retry)
        self.assertIn('&& rc == 0', method)
        self.assertIn('QStringLiteral("--close-running")', method)
        self.assertIn('if (m_busy || m_moduleOperationProcess || m_platformApprovalProcess)', method)

    def test_root_snapshot_marker_required(self):
        self.assertIn('OS_ADMIN_REVIEW_DIR=/var/lib/neebles/platform-operations/', self.cpp)
        self.assertIn('approvalReviewReady', self.cpp)
        self.assertIn('&& !boss.busy', self.qml)
        self.assertIn('transactionProcessDialog.contentItem.transaction.running !== true', self.qml)

    def test_i18n_and_ui_show_all_digests(self):
        for language in ('es_CL', 'en_US'):
            data=json.loads((ROOT / f'client/languages/{language}.json').read_text())
            for key in ('modules.transaction.os_review_title', 'modules.transaction.os_review_warning',
                        'modules.transaction.os_cancel', 'modules.transaction.os_approve'):
                self.assertIn(key, data)
                self.assertTrue(data[key].strip())
        for field in ('approvalManifestSha256', 'approvalContractSha256', 'approvalHelperSha256'):
            self.assertIn(field, self.qml)
        self.assertIn('osApprovalDialog', self.qml)

if __name__ == '__main__':
    unittest.main()
