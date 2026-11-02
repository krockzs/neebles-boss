"""F23: compatibility checks for modules that do not request root authority.

These tests do not install modules, invoke network providers or access host OS paths.
"""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

provider_path = Path(__file__).with_name('platform_provider.py')
spec = importlib.util.spec_from_file_location('neebles_f23_provider', provider_path)
provider = importlib.util.module_from_spec(spec)
spec.loader.exec_module(provider)


class LegacyCompatibility(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.dirs = [self.root / x for x in ('modules', 'helpers', 'policies', 'journal')]
        for folder in self.dirs:
            folder.mkdir()
        self.manager = provider.Manager(*self.dirs, strict=False)
        self.manager.approval_root.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def test_connect_without_privilege_declared(self):
        self._ordinary_module('neebles-connect', '1.0.2')
        self.assertEqual(self.manager.reconcile('neebles-connect'), 'unchanged')
        self.assertFalse(self.manager.approval_path('neebles-connect').exists())
        self.assertFalse((self.dirs[2] / 'neebles-connect.json').exists())

    def test_secrets_without_privilege_declared(self):
        self._ordinary_module('neebles-secrets', '1.0.7')
        self.assertEqual(self.manager.reconcile('neebles-secrets'), 'unchanged')
        self.assertFalse(self.manager.approval_path('neebles-secrets').exists())
        self.assertFalse((self.dirs[2] / 'neebles-secrets.json').exists())

    def test_no_root_provider_for_ordinary_module(self):
        self._ordinary_module('other-simple-module', '1.0.0')
        self.assertEqual(self.manager.reconcile('other-simple-module'), 'unchanged')
        self.assertEqual(list(self.manager.approval_root.iterdir()), [])

    def _ordinary_module(self, name, version):
        module_dir = self.dirs[0] / name
        module_dir.mkdir()
        (module_dir / 'manifest.json').write_text(json.dumps({
            'schema': 4, 'name': name, 'version': version,
            'entrypoint': 'runtime/application',
        }))


if __name__ == '__main__':
    unittest.main(verbosity=2)
