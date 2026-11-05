#!/usr/bin/env python3
"""Regresión estática del sandbox OS para sockets Bridge bajo /run/user.

Esta prueba no demuestra una conexión real: esa aceptación corresponde al equipo
instalado y al proveedor bajo el sandbox completo de systemd.
"""
import pathlib
import unittest

DIR = pathlib.Path(__file__).resolve().parent


def read_properties(raw):
    result = {}
    for line in raw.splitlines():
        line = line.strip()
        if not line or line.startswith(("#", ";", "[")):
            continue
        if "=" not in line:
            raise AssertionError("Invalid systemd unit property")
        key, val = line.split("=", 1)
        result.setdefault(key, []).append(val)
    return result


def inspect(service_raw, socket_raw):
    service = read_properties(service_raw)
    sock = read_properties(socket_raw)
    expected = {
        "ProtectHome": ["read-only"],
        "InaccessiblePaths": ["/home /root"],
        "ProtectSystem": ["strict"],
        "NoNewPrivileges": ["yes"],
        "RestrictAddressFamilies": ["AF_UNIX"],
        "PrivateTmp": ["yes"],
        "LockPersonality": ["yes"],
        "MemoryDenyWriteExecute": ["yes"],
        "RestrictSUIDSGID": ["yes"],
        "User": ["root"],
        "Group": ["root"],
        "ExecStart": ["/usr/bin/python3 -I /usr/lib/neebles/platform-operations/provider.py --serve"],
    }
    socket_expected = {
        "ListenStream": ["/run/neebles/platform-operations.sock"],
        "SocketUser": ["root"],
        "SocketGroup": ["root"],
        "SocketMode": ["0600"],
    }
    return all(service.get(key) == value for key, value in expected.items()) and \
        all(sock.get(key) == value for key, value in socket_expected.items())


class ProviderSandboxVisibility(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.service = (DIR / "neebles-platform-operations.service").read_text(encoding="utf-8")
        cls.sock = (DIR / "neebles-platform-operations.socket").read_text(encoding="utf-8")

    def test_installed_source_preserves_private_homes_but_exposes_run_user(self):
        self.assertTrue(inspect(self.service, self.sock))

    def test_protecthome_yes_regression_is_rejected(self):
        replacement = self.service.replace("ProtectHome=read-only", "ProtectHome=yes")
        self.assertNotEqual(replacement, self.service)
        self.assertFalse(inspect(replacement, self.sock))

    def test_home_or_root_exposure_is_rejected(self):
        replacement = self.service.replace("InaccessiblePaths=/home /root", "InaccessiblePaths=/home")
        self.assertNotEqual(replacement, self.service)
        self.assertFalse(inspect(replacement, self.sock))

    def test_socket_permission_broadening_is_rejected(self):
        replacement = self.sock.replace("SocketMode=0600", "SocketMode=0660")
        self.assertNotEqual(replacement, self.sock)
        self.assertFalse(inspect(self.service, replacement))


if __name__ == "__main__":
    unittest.main()
