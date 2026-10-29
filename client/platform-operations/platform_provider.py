#!/usr/bin/env python3
"""NEEBLES OS generic platform operation provider candidate; not auto-installed.

Root-owned policy is the sole authority for executable and fixed arguments.
"""
import json
import os
from pathlib import Path
import socket
import stat
import struct
import subprocess
import sys

POLICY_DIR = Path('/etc/neebles/platform-operations')
SOCKET_FD = 3
MAX_FRAME = 16384


def validate_identity(s):
    return isinstance(s, str) and 0 < len(s) <= 128 and all(c in 'abcdefghijklmnopqrstuvwxyz0123456789-._' for c in s)


def secure_regular(path, *, root_uid=0):
    """Obtain policy bytes through verified O_NOFOLLOW directory/file descriptors."""
    path = Path(path)
    if not path.is_absolute() or len(path.parts) < 2:
        raise ValueError('policy path must be an absolute file')
    fds = []
    try:
        fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        fds.append(fd)
        for component in path.parts[1:-1]:
            parent = os.fstat(fd)
            if not stat.S_ISDIR(parent.st_mode) or parent.st_uid != root_uid or parent.st_mode & 0o022:
                raise ValueError('untrusted policy directory')
            fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=fd)
            fds.append(fd)
        parent = os.fstat(fd)
        if not stat.S_ISDIR(parent.st_mode) or parent.st_uid != root_uid or parent.st_mode & 0o022:
            raise ValueError('untrusted policy directory')
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK, dir_fd=fd)
        fds.append(fd)
        info = os.fstat(fd)
        if not stat.S_ISREG(info.st_mode) or info.st_uid != root_uid or info.st_mode & 0o022 or info.st_nlink != 1 or info.st_size > MAX_FRAME:
            raise ValueError('untrusted policy file')
        data = os.read(fd, MAX_FRAME + 1)
        if len(data) > MAX_FRAME:
            raise ValueError('oversized policy')
        return data
    finally:
        for fd in reversed(fds):
            os.close(fd)



def secure_executable(path, *, root_uid=0):
    """Keep the verified ELF inode open until exec via /proc/self/fd."""
    path = Path(path)
    if not path.is_absolute() or len(path.parts) < 2:
        raise ValueError('absolute executable path required')
    fds = []
    try:
        parent = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_CLOEXEC)
        fds.append(parent)
        for component in path.parts[1:-1]:
            info = os.fstat(parent)
            if not stat.S_ISDIR(info.st_mode) or info.st_uid != root_uid or info.st_mode & 0o022:
                raise ValueError('untrusted executable directory')
            parent = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW | os.O_CLOEXEC, dir_fd=parent)
            fds.append(parent)
        info = os.fstat(parent)
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != root_uid or info.st_mode & 0o022:
            raise ValueError('untrusted executable directory')
        fd = os.open(path.name, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK, dir_fd=parent)
        info = os.fstat(fd)
        if (not stat.S_ISREG(info.st_mode) or info.st_uid != root_uid
                or info.st_mode & 0o022 or not info.st_mode & 0o111 or info.st_nlink != 1):
            os.close(fd)
            raise ValueError('untrusted executable inode')
        if os.read(fd, 4) != b'\x7fELF':
            os.close(fd)
            raise ValueError('executable must be native ELF')
        os.lseek(fd, 0, os.SEEK_SET)
        return fd
    finally:
        for item in reversed(fds):
            os.close(item)


def reject_duplicate_keys(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate policy key')
        result[key] = value
    return result


def load_policy(module, * , policy_dir=POLICY_DIR, verifier=secure_regular):
    if not validate_identity(module) or '/' in module or '..' in module:
        raise ValueError('invalid module identity')
    path = policy_dir / (module + '.json')
    data = verifier(path)
    if not isinstance(data, bytes):
        raise ValueError("verifier must return authenticated bytes")
    if len(data) > MAX_FRAME:
        raise ValueError('oversized policy')
    obj = json.loads(data, object_pairs_hook=reject_duplicate_keys)
    if not isinstance(obj, dict) or set(obj) != {'schema', 'module', 'operations'} or obj['schema'] != '1' or obj['module'] != module:
        raise ValueError('invalid policy identity')
    entries = obj['operations']
    if not isinstance(entries, dict) or not entries or len(entries) > 64:
        raise ValueError('invalid operations')
    for action, instruction in entries.items():
        if not validate_identity(action) or not isinstance(instruction, dict) or set(instruction) != {'executable', 'argv'}:
            raise ValueError('invalid operation entry')
        exe, argv = instruction['executable'], instruction['argv']
        if not isinstance(exe, str) or not exe.startswith('/') or '\x00' in exe or not isinstance(argv, list) or len(argv) > 32 or not all(isinstance(a, str) and len(a) <= 512 and '\x00' not in a for a in argv):
            raise ValueError('invalid execution contract')
    return entries


def handle(req, peer_uid, expected_uid, *, policy_loader=load_policy, executor=subprocess.run, executable_opener=secure_executable):
    if peer_uid != expected_uid:
        return {'schema':'1', 'ok':False, 'error':'caller not authorized'}
    if not isinstance(req, dict) or set(req) != {'schema','module','operation'} or req.get('schema') != '1':
        return {'schema':'1', 'ok':False, 'error':'invalid request shape'}
    module, operation = req['module'], req['operation']
    if not validate_identity(module) or not validate_identity(operation):
        return {'schema':'1', 'ok':False, 'error':'invalid operation identity'}
    try:
        entries = policy_loader(module)
        if operation not in entries:
            raise ValueError('operation not authorized')
        cmd = entries[operation]
        # All arguments originate from root-controlled policy; no shell or request-supplied argv.
        fd = executable_opener(cmd['executable'])
        try:
            result = executor([f'/proc/self/fd/{fd}', *cmd['argv']], pass_fds=(fd,), timeout=10,
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False,
                              env={'PATH':'/usr/sbin:/usr/bin:/sbin:/bin','LANG':'C'})
        finally:
            os.close(fd)
        if result.returncode != 0:
            raise ValueError('operation failed')
        return {'schema':'1','ok':True,'result':{'exit_code':0}}
    except (OSError, ValueError, subprocess.TimeoutExpired, json.JSONDecodeError):
        return {'schema':'1','ok':False,'error':'operation denied or failed'}


def verify_peer_executable(peer_pid, trusted_executable):
    """Bind the kernel peer PID to an immutable, OS-controlled Boss ELF inode."""
    if not isinstance(peer_pid, int) or peer_pid <= 1:
        return False
    trusted_fd = secure_executable(trusted_executable)
    try:
        expected = os.fstat(trusted_fd)
        peer_fd = os.open(f'/proc/{peer_pid}/exe', os.O_RDONLY | os.O_CLOEXEC)
        try:
            observed = os.fstat(peer_fd)
            return (expected.st_dev, expected.st_ino) == (observed.st_dev, observed.st_ino)
        finally:
            os.close(peer_fd)
    except OSError:
        return False
    finally:
        os.close(trusted_fd)


def is_boss_runtime_process(peer_pid):
    """Require exact kernel cgroup membership of the system Boss runtime."""
    if peer_pid <= 1:
        return False
    try:
        lines = Path(f'/proc/{peer_pid}/cgroup').read_text().splitlines()
    except OSError:
        return False
    # Boss system service has its own cgroup. Merely executing its ELF is insufficient.
    return any(line.startswith('0::') and line[3:] == '/system.slice/neebles-runtime.service'
               for line in lines)


def serve_one(client, expected_uid, *, policy_loader=load_policy,
              executor=subprocess.run, executable_opener=secure_executable,
              peer_verifier=verify_peer_executable, trusted_executable=None):
    """One authenticated AF_UNIX request; also used by integration tests."""
    client.settimeout(10)
    raw = client.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, 12)
    peer_pid, peer_uid, _ = struct.unpack('3i', raw)
    line = bytearray()
    complete = False
    try:
        while len(line) <= MAX_FRAME:
            b = client.recv(1)
            if not b:
                break
            if b == b'\n':
                complete = True
                break
            line.extend(b)
        if not complete or len(line) > MAX_FRAME:
            raise ValueError('invalid frame')
        req = json.loads(line, object_pairs_hook=reject_duplicate_keys)
        if not trusted_executable or not peer_verifier(peer_pid, trusted_executable) or not is_boss_runtime_process(peer_pid):
            response = {'schema':'1','ok':False,'error':'caller executable not authorized'}
        else:
            response = handle(req, peer_uid, expected_uid, policy_loader=policy_loader,
                          executor=executor, executable_opener=executable_opener)
    except (ValueError, UnicodeDecodeError, socket.timeout):
        response = {'schema':'1','ok':False,'error':'invalid frame'}
    client.sendall(json.dumps(response, separators=(',',':')).encode()+b'\n')


def serve():
    if os.geteuid() != 0:
        raise RuntimeError('root service required')
    if os.environ.get('LISTEN_PID') != str(os.getpid()) or os.environ.get('LISTEN_FDS') != '1':
        raise RuntimeError('systemd socket activation required')
    uid_text = os.environ.get('NEEBLES_BOSS_UID', '')
    if not uid_text.isdecimal():
        raise RuntimeError('OS-controlled UID required')
    uid = int(uid_text)
    executable = os.environ.get('NEEBLES_BOSS_EXE', '')
    if not executable or not Path(executable).is_absolute():
        raise RuntimeError('OS-controlled Boss executable required')
    # Refuse startup unless the configured ELF is root-controlled.
    verified_fd = secure_executable(executable)
    os.close(verified_fd)
    listener = socket.socket(fileno=SOCKET_FD)
    if listener.family != socket.AF_UNIX or listener.type & socket.SOCK_STREAM != socket.SOCK_STREAM:
        raise RuntimeError('invalid systemd socket')
    while True:
        client, _ = listener.accept()
        with client:
            serve_one(client, uid, trusted_executable=executable)



if __name__ == '__main__':
    if sys.argv[1:] != ['--serve']:
        raise SystemExit('usage: platform_provider.py --serve')
    serve()
