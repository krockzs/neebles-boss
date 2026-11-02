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


import hashlib
import json
import os
from pathlib import Path
import re
import stat
import tempfile

MODULES_ROOT = Path('/opt/neebles/modules')
HELPERS_ROOT = Path('/usr/lib/neebles/module-operations')
POLICY_ROOT = Path('/etc/neebles/platform-operations')
JOURNAL_ROOT = Path('/var/lib/neebles/platform-operations/journal')
APPROVAL_ROOT = POLICY_ROOT / 'approvals'
MODULE = re.compile(r'[a-z0-9][a-z0-9-]{0,127}\Z')
ACTION = re.compile(r'[a-z0-9][a-z0-9._-]{0,127}\Z')
SHA = re.compile(r'[a-f0-9]{64}\Z')
MAX_POLICY = 16384
MAX_HELPER = 32 * 1024 * 1024


def unique(pairs):
    obj = {}
    for k, v in pairs:
        if k in obj:
            raise ValueError('duplicate JSON key')
        obj[k] = v
    return obj


def parse(raw, cap=MAX_POLICY):
    if not raw or len(raw) > cap:
        raise ValueError('invalid JSON size')
    return json.loads(raw, object_pairs_hook=unique)


def trusted(path, *, kind='file', expected_uid=0, strict=True):
    path = Path(path)
    if not path.is_absolute() or '..' in path.parts:
        raise ValueError('untrusted absolute path')
    cursor = Path('/')
    for part in path.parts[1:]:
        cursor /= part
        info = cursor.lstat()
        if stat.S_ISLNK(info.st_mode):
            raise ValueError('symlink prohibited: ' + str(cursor))
        if strict and (info.st_uid != expected_uid or info.st_mode & 0o022):
            raise ValueError('untrusted path ownership or mode: ' + str(cursor))
        if cursor != path and not stat.S_ISDIR(info.st_mode):
            raise ValueError('non-directory ancestor: ' + str(cursor))
    info = path.lstat()
    if kind == 'file' and (not stat.S_ISREG(info.st_mode) or info.st_nlink != 1):
        raise ValueError('unsafe regular file: ' + str(path))
    if kind == 'directory' and not stat.S_ISDIR(info.st_mode):
        raise ValueError('unsafe directory: ' + str(path))
    return info


def safe_read(path, cap=MAX_POLICY, **kw):
    info = trusted(path, **kw)
    if info.st_size > cap:
        raise ValueError('source exceeds maximum size')
    with Path(path).open('rb') as src:
        raw = src.read(cap + 1)
    if len(raw) > cap:
        raise ValueError('oversized source')
    return raw


def atomic(path, data, mode, *, strict=True):
    path = Path(path)
    trusted(path.parent, kind='directory', strict=strict)
    if path.exists() or path.is_symlink():
        trusted(path, strict=strict)
    fd, tmp = tempfile.mkstemp(prefix='.ops-', dir=path.parent)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, 'wb') as handle:
            handle.write(data)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp, path)
        dfd = os.open(path.parent, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
        try:
            os.fsync(dfd)
        finally:
            os.close(dfd)
    finally:
        if os.path.lexists(tmp):
            os.unlink(tmp)


def sha(data):
    return hashlib.sha256(data).hexdigest()


class Manager:
    def __init__(self, modules_root=MODULES_ROOT, helpers_root=HELPERS_ROOT,
                 policy_root=POLICY_ROOT, journal_root=JOURNAL_ROOT, *,
                 approval_root=None, strict=True):
        self.modules_root = Path(modules_root)
        self.helpers_root = Path(helpers_root)
        self.policy_root = Path(policy_root)
        self.journal_root = Path(journal_root)
        self.approval_root = Path(approval_root) if approval_root is not None else Path(policy_root) / 'approvals'
        self.strict = strict

    def dirs(self):
        for root in (self.modules_root, self.helpers_root, self.policy_root, self.journal_root, self.approval_root):
            trusted(root, kind='directory', strict=self.strict)

    def paths(self, module):
        if not isinstance(module, str) or not MODULE.fullmatch(module):
            raise ValueError('invalid module identity')
        return (self.modules_root / module,
                self.helpers_root / (module + '-platform'),
                self.policy_root / (module + '.json'),
                self.journal_root / (module + '.json'))

    def approval_path(self, module):
        # Same restrictive identity grammar as a module policy filename.
        self.paths(module)
        return self.approval_root / (module + '.json')

    def fingerprint(self, module):
        """Fingerprint exact module source bytes, not a path claimed over IPC."""
        directory, _, _, _ = self.paths(module)
        self.installed_contract(module)  # validates manifest/contract/ELF and operation paths
        return {
            'schema': '1',
            'module': module,
            'manifest_sha256': sha(safe_read(directory / 'manifest.json', strict=self.strict)),
            'contract_sha256': sha(safe_read(directory / 'contracts/platform-operations.json', strict=self.strict)),
            'helper_sha256': sha(safe_read(directory / 'runtime' / (module + '-platform'), MAX_HELPER, strict=self.strict)),
        }

    def approval(self, module):
        path = self.approval_path(module)
        record = parse(safe_read(path, strict=self.strict))
        expected_keys = {'schema', 'module', 'manifest_sha256', 'contract_sha256', 'helper_sha256'}
        if not isinstance(record, dict) or set(record) != expected_keys:
            raise ValueError('invalid root approval receipt')
        if record.get('schema') != '1' or record.get('module') != module:
            raise ValueError('root approval identity mismatch')
        for key in ('manifest_sha256', 'contract_sha256', 'helper_sha256'):
            if not isinstance(record.get(key), str) or not SHA.fullmatch(record[key]):
                raise ValueError('root approval digest invalid')
        expected = self.fingerprint(module)
        if record != expected:
            raise ValueError('OS approval missing for current certified module bytes')
        return True

    def approve(self, module):
        """Explicit OS-administrator action only; never invoked by Boss IPC."""
        if os.geteuid() != 0 and self.strict:
            raise ValueError('root required for OS approval')
        self.dirs()
        data = self.fingerprint(module)
        atomic(self.approval_path(module),
               (json.dumps(data, sort_keys=True, separators=(',', ':')) + '\n').encode(),
               0o600, strict=self.strict)
        self.approval(module)
        return data

    def revoke_approval(self, module):
        """Revoke approval; audit then denies operations even if a policy remains."""
        if os.geteuid() != 0 and self.strict:
            raise ValueError('root required for OS revocation')
        self.dirs()
        path = self.approval_path(module)
        if path.exists() or path.is_symlink():
            trusted(path, strict=self.strict)
            path.unlink()
        self.reconcile(module, remove=True)
        return True

    def pending(self, module):
        _, _, _, journal = self.paths(module)
        return journal.exists() or journal.is_symlink()

    def installed_contract(self, module):
        directory, target_helper, _, _ = self.paths(module)
        trusted(directory, kind='directory', strict=self.strict)
        manifest = parse(safe_read(directory / 'manifest.json', strict=self.strict))
        if not isinstance(manifest, dict) or manifest.get('schema') != 4 or manifest.get('name') != module:
            raise ValueError('installed module identity mismatch')
        relative = manifest.get('platform_operations')
        if relative is None:
            return None
        if relative != 'contracts/platform-operations.json':
            raise ValueError('unsupported platform_operations declaration path')
        contract = parse(safe_read(directory / relative, strict=self.strict))
        if not isinstance(contract, dict) or set(contract) != {'schema', 'module', 'operations'} or contract.get('schema') != '1' or contract.get('module') != module:
            raise ValueError('invalid declared platform contract')
        ops = contract['operations']
        if not isinstance(ops, dict) or not 1 <= len(ops) <= 64:
            raise ValueError('invalid number of privileged operations')
        for action, value in ops.items():
            if not isinstance(action, str) or not ACTION.fullmatch(action):
                raise ValueError('invalid platform action')
            if not isinstance(value, dict) or set(value) != {'executable', 'argv'}:
                raise ValueError('invalid platform operation shape')
            if value['executable'] != str(target_helper):
                raise ValueError('OS helper path must match exact module')
            argv = value['argv']
            if not isinstance(argv, list) or len(argv) > 32 or any(
                not isinstance(x, str) or len(x) > 512 or '\x00' in x for x in argv
            ):
                raise ValueError('invalid fixed argv')
        helper_source = directory / 'runtime' / (module + '-platform')
        raw = safe_read(helper_source, MAX_HELPER, strict=self.strict)
        if raw[:4] != b'\x7fELF':
            raise ValueError('declared helper must be a native ELF')
        if not trusted(helper_source, strict=self.strict).st_mode & 0o111:
            raise ValueError('helper is not executable')
        policy = (json.dumps(contract, sort_keys=True, separators=(',', ':')) + '\n').encode()
        return raw, policy

    def audit(self, module):
        self.dirs()
        if self.pending(module):
            raise ValueError('unreconciled privileged transaction; requests blocked')
        directory, helper, policy, _ = self.paths(module)
        if not directory.exists():
            raise ValueError('module no longer installed')
        expected = self.installed_contract(module)
        if expected is None:
            raise ValueError('module has no platform authority declaration')
        self.approval(module)
        if safe_read(helper, MAX_HELPER, strict=self.strict) != expected[0]:
            raise ValueError('installed helper diverged from module material')
        if safe_read(policy, MAX_POLICY, strict=self.strict) != expected[1]:
            raise ValueError('installed policy diverged from module material')
        return True

    def _snapshot(self, helper, policy):
        out = []
        for path in (helper, policy):
            if path.exists() or path.is_symlink():
                raw = safe_read(path, MAX_HELPER if path == helper else MAX_POLICY, strict=self.strict)
                out.append({'bytes': raw.hex(), 'mode': stat.S_IMODE(path.stat().st_mode)})
            else:
                out.append(None)
        return out

    def _restore(self, helper, policy, previous):
        for path, before in zip((helper, policy), previous):
            if before is None:
                if path.exists():
                    trusted(path, strict=self.strict)
                    path.unlink()
            else:
                atomic(path, bytes.fromhex(before['bytes']), before['mode'], strict=self.strict)

    def recover(self, module):
        _, helper, policy, journal = self.paths(module)
        if not self.pending(module):
            return False
        record = parse(safe_read(journal, MAX_HELPER*2+MAX_POLICY, strict=self.strict), cap=MAX_HELPER*2+MAX_POLICY)
        if record.get('schema') != '1' or record.get('module') != module or not isinstance(record.get('previous'), list) or len(record['previous']) != 2:
            raise ValueError('untrusted journal record')
        for entry in record['previous']:
            if entry is not None and (not isinstance(entry, dict) or set(entry) != {'bytes','mode'} or not isinstance(entry['bytes'],str) or not isinstance(entry['mode'],int)):
                raise ValueError('invalid journal snapshot')
        self._restore(helper, policy, record['previous'])
        journal.unlink()
        return True

    def reconcile(self, module, *, remove=False):
        self.dirs()
        directory, helper, policy, journal = self.paths(module)
        # Recover pre-interruption transaction before starting a new one.
        if self.pending(module):
            self.recover(module)
        if not remove and not directory.exists():
            raise ValueError('cannot grant to an uninstalled module')
        desired = None if remove else self.installed_contract(module)
        if desired is not None:
            self.approval(module)  # a declared request NEVER self-grants a root policy
        new_helper, new_policy = desired if desired is not None else (None, None)
        current = self._snapshot(helper, policy)
        want = [new_helper, new_policy]
        if all((old is None and data is None) or (old is not None and bytes.fromhex(old['bytes']) == data)
               for old, data in zip(current, want)):
            return 'unchanged'
        record = {'schema':'1','module':module,'previous':current}
        atomic(journal, (json.dumps(record, separators=(',', ':'))+'\n').encode(), 0o600, strict=self.strict)
        try:
            for path, data, mode in zip((helper, policy), want, (0o755, 0o600)):
                if data is None:
                    if path.exists():
                        trusted(path, strict=self.strict)
                        path.unlink()
                else:
                    atomic(path, data, mode, strict=self.strict)
            # Validate exact byte identity and restoration even on partial error.
            for path, data in zip((helper, policy), want):
                if data is None and path.exists():
                    raise ValueError('revocation not complete')
                if data is not None and safe_read(path, MAX_HELPER if path == helper else MAX_POLICY,
                                                 strict=self.strict) != data:
                    raise ValueError('installed policy/helper mismatch')
            journal.unlink()
            return 'revoked' if desired is None else 'installed'
        except Exception:
            self.recover(module)
            raise


def handle_control(req, manager=None):
    if not isinstance(req, dict) or set(req) != {'schema', 'module', 'operation'} or req.get('schema') != '1':
        return {'schema':'1','ok':False,'error':'invalid management request'}
    module, operation = req['module'], req['operation']
    if not isinstance(module, str) or not MODULE.fullmatch(module):
        return {'schema':'1','ok':False,'error':'invalid module'}
    if operation not in ('platform.policy.sync', 'platform.policy.revoke'):
        return {'schema':'1','ok':False,'error':'invalid management action'}
    try:
        engine = manager or Manager()
        value = engine.reconcile(module, remove=operation.endswith('.revoke'))
        return {'schema':'1','ok':True,'result':{'state':value}}
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        return {'schema':'1','ok':False,'error':'policy transaction rejected or failed'}


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
        # Reserve policy lifecycle for the physically authenticated Boss runtime.
        # A module can only change its own policy from its pinned installed files.
        # No arbitrary source path, ELF, argv, shell, or module target arrives over IPC.
        if operation in ('platform.policy.sync', 'platform.policy.revoke'):
            return handle_control(req)
        if operation.startswith('platform.policy.'):
            raise ValueError('reserved OS policy operation')
        # A leftover root policy after uninstall or a changed installed ELF
        # NEVER authorizes execution. Rollbacks and crashes fail closed.
        if policy_loader is load_policy:
            Manager().audit(module)
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



def administer(argv):
    # Standalone OS-administrator interface; never exposed as a provider IPC action.
    import argparse
    parser = argparse.ArgumentParser(description='NEEBLES OS module privilege approvals')
    sub = parser.add_subparsers(dest='command', required=True)
    for name in ('approve', 'audit-approval', 'revoke-approval'):
        cmd = sub.add_parser(name)
        cmd.add_argument('--module', required=True)
        if name == 'approve':
            cmd.add_argument('--candidate-dir', required=True, type=Path)
            cmd.add_argument('--confirm-root-authority', action='store_true')
    args = parser.parse_args(argv)
    if os.geteuid() != 0:
        raise ValueError('explicit root administrator authorization required')
    if args.command == 'approve':
        if not args.confirm_root_authority:
            raise ValueError('explicit --confirm-root-authority required')
        candidate = args.candidate_dir
        if not candidate.is_absolute() or candidate.name != args.module:
            raise ValueError('candidate directory must be absolute and named for the module')
        # Privileged approval reads only a separately controlled, root-owned
        # candidate directory. Boss cannot create or modify the approval.
        admin = Manager(modules_root=candidate.parent)
        record = admin.approve(args.module)
        print('APPROVED OS privileged material:', record['module'])
        for key in ('manifest_sha256', 'contract_sha256', 'helper_sha256'):
            print(key, record[key])
    else:
        admin = Manager()
        if args.command == 'audit-approval':
            admin.approval(args.module)
            print('PASS root material approval:', args.module)
        else:
            admin.revoke_approval(args.module)
            print('REVOKED OS material approval:', args.module)


if __name__ == '__main__':
    try:
        if sys.argv[1:] == ['--serve']:
            serve()
        else:
            administer(sys.argv[1:])
    except (OSError, ValueError, TypeError, KeyError, json.JSONDecodeError) as exc:
        print('DENIED:', exc, file=sys.stderr)
        raise SystemExit(3)
