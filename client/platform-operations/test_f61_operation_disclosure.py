"""F61: OS review content must be shown and fingerprinted before approval."""
from pathlib import Path
import json
import unittest

ROOT = Path(__file__).resolve().parents[2]

class ReviewDisclosure(unittest.TestCase):
    def setUp(self):
        self.cpp = (ROOT/'ui/client/bosscontroller.cpp').read_text()
        self.qml = (ROOT/'ui/client/qml/Main.qml').read_text()
        self.head = (ROOT/'ui/client/bosscontroller.h').read_text()

    def test_review_is_distinct_read_only_root_action(self):
        self.assertIn('Q_INVOKABLE void reviewPendingPlatformOperation();', self.head)
        review = self.cpp.split('void BossController::reviewPendingPlatformOperation()',1)[1].split('void BossController::approvePendingPlatformOperation()',1)[0]
        self.assertIn('QStringLiteral("review")', review)
        self.assertIn('QStringLiteral("--candidate-dir")', review)
        self.assertNotIn('--confirm-root-authority', review)
        self.assertNotIn('approvePendingPlatformOperation(', review)

    def test_review_rejects_mismatched_sha_and_wrong_module(self):
        review = self.cpp.split('void BossController::reviewPendingPlatformOperation()',1)[1].split('void BossController::approvePendingPlatformOperation()',1)[0]
        for sha in ('manifest_sha256','contract_sha256','helper_sha256'):
            self.assertIn(sha, review)
        self.assertIn('fingerprint.value(QStringLiteral("module"))', review)
        self.assertIn('output.size() > 65536', review)
        self.assertIn('!doc.isObject()', review)
        self.assertIn('QJsonDocument(operations).toJson', review)

    def test_approval_requires_completed_review(self):
        approval = self.cpp.split('void BossController::approvePendingPlatformOperation()',1)[1].split('void BossController::uninstallModule(',1)[0]
        self.assertIn('approvalOperationsReviewed', approval)
        self.assertIn('if (!approved)', approval)
        self.assertIn('approvalOperationsReviewed === true', self.qml)
        self.assertIn('boss.reviewPendingPlatformOperation()', self.qml)
        self.assertIn('boss.approvePendingPlatformOperation()', self.qml)

    def test_shows_operations_and_translates(self):
        self.assertIn('approvalOperations || ""', self.qml)
        for language in ('es_CL','en_US'):
            data=json.loads((ROOT/'client/languages'/f'{language}.json').read_text())
            self.assertTrue(data['modules.transaction.os_reviewing'])
            self.assertTrue(data['modules.transaction.os_operations'])

if __name__ == '__main__':
    unittest.main()
