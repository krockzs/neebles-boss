"""F56 static UI/contract gates; runtime Qt requires controlled Esbirro build."""
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]

class ApprovalPresentation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.cpp=(ROOT/'ui/client/bosscontroller.cpp').read_text()
        cls.qml=(ROOT/'ui/client/qml/Main.qml').read_text()
        cls.es=(ROOT/'client/languages/es_CL.json').read_text()
        cls.en=(ROOT/'client/languages/en_US.json').read_text()
        cls.rust=(ROOT/'src/platform_policy_lifecycle.rs').read_text()

    def test_ui_tracks_only_exact_format_without_grant(self):
        self.assertIn('OS_ADMIN_APPROVAL_REQUIRED module=',self.cpp)
        self.assertIn('approvalRequired',self.cpp)
        self.assertIn('match.captured(1) == m_activeModuleName',self.cpp)
        self.assertIn('m_activeModuleOperation == QStringLiteral("install")',self.cpp)
        self.assertIn('m_activeModuleOperation == QStringLiteral("update")',self.cpp)
        self.assertNotIn('platform.policy.approve',self.cpp)

    def test_transaction_dialog_has_localized_pending_state(self):
        self.assertIn('transaction.approvalRequired === true',self.qml)
        self.assertIn('root.t("modules.transaction.os_approval_required")',self.qml)
        for data in (self.es,self.en):
            self.assertIn('"modules.transaction.os_approval_required"',data)

    def test_rust_challenge_is_bound_to_original_request(self):
        self.assertIn('request.get("candidate") == Some(expected)',self.rust)
        self.assertIn('Some("platform.policy.preflight")',self.rust)
        self.assertIn('review_challenge_must_match_exact_sent_candidate_and_operation',self.rust)

    def test_no_automatic_os_admin_approval_from_qt(self):
        marker=self.cpp.split('An approval challenge is a PRESENTATION state',1)[1].split('Everything not belonging to the reserved',1)[0]
        self.assertNotIn('process->start',marker)
        self.assertNotIn('run(',marker)
        self.assertNotIn('pkexec',marker.split('*/')[-1])

if __name__ == '__main__':
    unittest.main()
