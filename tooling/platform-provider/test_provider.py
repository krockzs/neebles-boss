import unittest
import tempfile
import os
from pathlib import Path
import tempfile
import os
from pathlib import Path
from types import SimpleNamespace
from platform_provider import handle, validate_identity, secure_regular, load_policy, secure_executable, verify_peer_executable

POLICY={'generic.operation':{'executable':'/usr/bin/true','argv':[]}}
REQ={'schema':'1','module':'test-module','operation':'generic.operation'}

class ProviderTests(unittest.TestCase):
    def test_good(self):
        calls=[]
        def run(cmd, **kwargs):
            calls.append((cmd,kwargs)); return SimpleNamespace(returncode=0)
        r=handle(REQ,1000,1000,policy_loader=lambda _:POLICY,executor=run)
        self.assertTrue(r['ok']); self.assertEqual(calls[0][0][1:], [])
        self.assertTrue(calls[0][0][0].startswith('/proc/self/fd/'))
        self.assertEqual(len(calls[0][1]['pass_fds']), 1)
        self.assertNotIn('shell', calls[0][1])
    def test_output_is_discarded_and_not_buffered_by_root_provider(self):
        import subprocess
        seen=[]
        def run(cmd, **kwargs):
            seen.append(kwargs)
            return SimpleNamespace(returncode=0)
        result=handle(REQ, 1000, 1000, policy_loader=lambda _: POLICY, executor=run)
        self.assertTrue(result['ok'])
        self.assertEqual(len(seen), 1)
        self.assertIs(seen[0]['stdout'], subprocess.DEVNULL)
        self.assertIs(seen[0]['stderr'], subprocess.DEVNULL)
        self.assertNotIn('capture_output', seen[0])
        self.assertNotIn('text', seen[0])

    def test_wrong_uid(self):
        self.assertFalse(handle(REQ,1001,1000,policy_loader=lambda _:POLICY)['ok'])
    def test_missing_operation(self):
        r=handle({**REQ,'operation':'other'},1000,1000,policy_loader=lambda _:POLICY)
        self.assertFalse(r['ok'])
    def test_extra_arguments_refused(self):
        self.assertFalse(handle({**REQ,'argv':['--bad']},1000,1000,policy_loader=lambda _:POLICY)['ok'])
    def test_bogus_module(self):
        self.assertFalse(handle({**REQ,'module':'../evil'},1000,1000,policy_loader=lambda _:POLICY)['ok'])
    def test_failed_command(self):
        r=handle(REQ,1000,1000,policy_loader=lambda _:POLICY,executor=lambda *a,**kw:SimpleNamespace(returncode=1))
        self.assertFalse(r['ok'])
    def test_verified_file_fd_reads_exact_bytes(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'policy.json'; p.write_bytes(b'hello')
            # /tmp may be sticky, but still world-writable; explicitly reject it.
            with self.assertRaises(ValueError):
                secure_regular(p, root_uid=os.getuid())

    def test_verified_file_symlink_denied(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/'data'; p.write_text('x')
            link=Path(d)/'link'; link.symlink_to(p)
            with self.assertRaises((OSError,ValueError)):
                secure_regular(link, root_uid=os.getuid())

    def test_verified_loader_consumes_bytes_not_reopen(self):
        import json
        with tempfile.TemporaryDirectory() as d:
            obj={'schema':'1','module':'test-module','operations':POLICY}
            def verify(path):
                self.assertEqual(path,Path(d)/'test-module.json')
                return json.dumps(obj).encode()
            self.assertEqual(load_policy('test-module',policy_dir=Path(d),verifier=verify),POLICY)

    def test_real_authorized_elf_executes_via_fd(self):
        self.assertTrue(handle(REQ, 1000, 1000, policy_loader=lambda _: POLICY)['ok'])

    def test_symlink_executable_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            link = Path(d)/'link'
            link.symlink_to('/usr/bin/true')
            with self.assertRaises((ValueError, OSError)):
                secure_executable(link, root_uid=os.getuid())

    def test_untrusted_path_is_rejected(self):
        with tempfile.TemporaryDirectory() as d:
            fake = Path(d)/'fake'
            fake.write_bytes(b'\x7fELFhello')
            fake.chmod(0o755)
            with self.assertRaises((ValueError, OSError)):
                secure_executable(fake, root_uid=os.getuid())

    def test_peer_pid_must_match_root_owned_elf(self):
        self.assertFalse(verify_peer_executable(-1, '/usr/bin/true'))
        self.assertFalse(verify_peer_executable(os.getpid(), '/usr/bin/true'))
    def test_identity(self):
        self.assertTrue(validate_identity('generic.operation'))
        self.assertFalse(validate_identity('A/B'))

if __name__ == '__main__': unittest.main()
