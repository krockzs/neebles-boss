import json
import os
import socket
import threading
import unittest
from types import SimpleNamespace
from platform_provider import serve_one, MAX_FRAME

POLICY={'generic.operation':{'executable':'/usr/bin/true','argv':[]}}

class WireContract(unittest.TestCase):
    def exchange(self, data, expected_uid=None):
        a,b=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
        expected_uid = os.getuid() if expected_uid is None else expected_uid
        ran=[]
        def execute(*args, **kwargs):
            ran.append(args); return SimpleNamespace(returncode=0)
        def worker():
            with b:
                serve_one(b, expected_uid, policy_loader=lambda _:POLICY,executor=execute,
                          trusted_executable='/usr/bin/true', peer_verifier=lambda pid, path: True)
        t=threading.Thread(target=worker,daemon=True); t.start()
        with a:
            a.settimeout(3)
            a.sendall(data)
            a.shutdown(socket.SHUT_WR)
            out=b''
            while not out.endswith(b'\n'):
                chunk=a.recv(4096)
                if not chunk: break
                out+=chunk
        t.join(3)
        self.assertFalse(t.is_alive())
        self.assertLessEqual(len(out),MAX_FRAME)
        return json.loads(out),ran
    def test_same_uid_forged_executable_denied(self):
        a,b=socket.socketpair(socket.AF_UNIX,socket.SOCK_STREAM)
        ran=[]
        def worker():
            with b:
                serve_one(b, os.getuid(), policy_loader=lambda _: POLICY,
                          executor=lambda *args, **kwargs: ran.append(args),
                          trusted_executable='/usr/bin/true', peer_verifier=lambda pid,path: False)
        t=threading.Thread(target=worker,daemon=True);t.start()
        with a:
            a.sendall(b'{"schema":"1","module":"test-module","operation":"generic.operation"}\n')
            r=json.loads(a.recv(4096))
        t.join(3)
        self.assertFalse(r['ok']); self.assertFalse(ran)
        self.assertEqual(r['error'],'caller executable not authorized')
    def test_real_unix_socket_round_trip(self):
        r,ran=self.exchange(b'{"schema":"1","module":"test-module","operation":"generic.operation"}\n')
        self.assertTrue(r['ok']);self.assertEqual(r['result']['exit_code'],0);self.assertEqual(len(ran),1)
    def test_kernel_uid_mismatch_denies_without_exec(self):
        r,ran=self.exchange(b'{"schema":"1","module":"test-module","operation":"generic.operation"}\n',expected_uid=os.getuid()+1)
        self.assertFalse(r['ok']);self.assertEqual(r['error'],'caller not authorized');self.assertFalse(ran)
    def test_unauthorized_operation_denies(self):
        r,ran=self.exchange(b'{"schema":"1","module":"test-module","operation":"other"}\n')
        self.assertFalse(r['ok']);self.assertFalse(ran)
    def test_extra_request_keys_denied(self):
        r,ran=self.exchange(b'{"schema":"1","module":"test-module","operation":"generic.operation","argv":["--x"]}\n')
        self.assertFalse(r['ok']);self.assertFalse(ran)
    def test_invalid_json_denied(self):
        r,ran=self.exchange(b'not json\n')
        self.assertFalse(r['ok']);self.assertFalse(ran)
    def test_missing_terminator_denied(self):
        r,ran=self.exchange(b'{"schema":"1","module":"test-module","operation":"generic.operation"}')
        self.assertFalse(r['ok']);self.assertFalse(ran)
    def test_oversized_frame_denied(self):
        r,ran=self.exchange(b'x'*(MAX_FRAME+1)+b'\n')
        self.assertFalse(r['ok']);self.assertFalse(ran)

if __name__=='__main__':unittest.main()
