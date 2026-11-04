"""F57: structural snapshot integration gates, plus real Rust unit suites."""
import pathlib
import unittest
ROOT=pathlib.Path(__file__).resolve().parents[2]
class ReviewSnapshotWiring(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.rust=(ROOT/'src/platform_policy_lifecycle.rs').read_text()
        cls.modules=(ROOT/'src/modules.rs').read_text()
    def test_root_owned_os_review_store_is_separate_from_installed_module(self):
        self.assertIn('const OS_REVIEW_ROOT: &str = "/var/lib/neebles/platform-operations/reviews";',self.rust)
        self.assertIn('preserve_candidate_for_os_review',self.rust)
        self.assertIn('review_root_ancestors',self.rust)
        self.assertIn('only root Governor may prepare OS review material',self.rust)
        self.assertNotIn('approve_exact_candidate',self.rust)
    def test_only_exact_challenge_enters_snapshot(self):
        self.assertIn('if !preflight_error.starts_with(&expected_prefix)',self.rust)
        self.assertEqual(self.modules.count('preserve_candidate_for_os_review('),2)
        self.assertIn('OS_ADMIN_REVIEW_DIR=',self.modules)
    def test_snapshot_does_not_touch_live_module_or_approval(self):
        marker=self.rust.split('const OS_REVIEW_ROOT:',1)[1].split('/// Preserve a verified root-only review copy',1)[0]
        self.assertNotIn('/opt/neebles/modules/',marker)
        self.assertNotIn('platform.policy.sync',marker)
        self.assertNotIn('approval_path(',marker)
        self.assertIn('fs::rename(&temporary, &final_path)',marker)
if __name__=='__main__':unittest.main()
