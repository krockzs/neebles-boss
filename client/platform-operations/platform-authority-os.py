#!/usr/bin/env python3
"""OS-owned generic platform authority administration.

Never executes module operations, never loads an ELF into a new runtime,
and never edits the Boss executable. This utility is a privileged *installer*
requiring explicit root action, not a Lifecycle artillery.
"""
import argparse
import json
import os
from pathlib import Path
import re
import stat
import sys
import tempfile

AUTHORITY = 'neebles.platform.operation'
SUPPLY_PATH = Path('/usr/lib/neebles/platform/authority/authority-supply.json')
DESCRIPTOR_PATH = Path('/usr/lib/neebles/platform/authority/neebles-platform-operation.json')
POLICY_DIR = Path('/etc/neebles/platform-operations')
SOCKET_PATH = '/run/neebles/platform-operations.sock'
MODULE_RE = re.compile(r'[a-z0-9][a-z0-9-]{0,127}\Z')
ACTION_RE = re.compile(r'[a-z0-9][a-z0-9._-]{0,127}\Z')


def pairs_unique(pairs):
    value = {}
    for k, v in pairs:
        if k in value:
            raise ValueError('duplicate JSON key')
        value[k] = v
    return value


def load_json(path):
    data = path.read_bytes()
    if not data or len(data) > 16384:
        raise ValueError('empty or oversized JSON')
    return json.loads(data, object_pairs_hook=pairs_unique)


def strict_root_path(path, kind='file'):
    p = Path(path)
    if not p.is_absolute() or '..' in p.parts:
        raise ValueError('absolute confined path required')
    current = Path('/')
    for part in p.parts[1:]:
        current /= part
        s = current.lstat()
        if stat.S_ISLNK(s.st_mode) or s.st_uid != 0 or s.st_mode & 0o022:
            raise ValueError('untrusted ownership or mode: ' + str(current))
        if current != p and not stat.S_ISDIR(s.st_mode):
            raise ValueError('non-directory ancestor: ' + str(current))
    s = p.lstat()
    if kind == 'file' and not stat.S_ISREG(s.st_mode):
        raise ValueError('not a regular file: ' + str(p))
    if kind == 'directory' and not stat.S_ISDIR(s.st_mode):
        raise ValueError('not a directory: ' + str(p))
    return s


def atomic_json(path, data, mode):
    strict_root_path(path.parent, 'directory')
    target = path
    if target.exists() or target.is_symlink():
        strict_root_path(target)
    fd, tmp = tempfile.mkstemp(prefix='.platform-ops-', dir=target.parent)
    try:
        os.fchmod(fd, mode)
        with os.fdopen(fd, 'wb') as stream:
            stream.write(json.dumps(data, indent=2, ensure_ascii=False).encode() + b'\n')
            stream.flush()
            os.fsync(stream.fileno())
        os.replace(tmp, target)
        parent_fd = os.open(target.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(parent_fd)
        finally:
            os.close(parent_fd)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def check_supply(data):
    if not isinstance(data, dict) or data.get('schema') != '1' or data.get('name') != 'neebles-authority-supply' or not isinstance(data.get('entries'), list):
        raise ValueError('invalid Boss AuthoritySupply shape')
    seen = set()
    for entry in data['entries']:
        if not isinstance(entry, dict) or set(entry) != {'authority', 'location'}:
            raise ValueError('unexpected AuthoritySupply entry shape')
        name, location = entry['authority'], entry['location']
        if not isinstance(name, str) or not name or name in seen or not isinstance(location, str) or not location.startswith('/'):
            raise ValueError('invalid or duplicate authority')
        strict_root_path(Path(location))
        seen.add(name)
    return seen


def validate_global_descriptor(data):
    return isinstance(data, dict) and data == {
        'schema': '2',
        'authority': AUTHORITY,
        'authorization': 'root-policy-per-module',
    }


def validate_policy(module, data):
    if not MODULE_RE.fullmatch(module):
        raise ValueError('invalid module identity')
    if not isinstance(data, dict) or set(data) != {'schema', 'module', 'operations'} or data['schema'] != '1' or data['module'] != module:
        raise ValueError('policy identity mismatch')
    operations = data['operations']
    if not isinstance(operations, dict) or not (1 <= len(operations) <= 64):
        raise ValueError('policy operations invalid')
    for name, entry in operations.items():
        if not isinstance(name, str) or not ACTION_RE.fullmatch(name):
            raise ValueError('invalid operation name')
        if not isinstance(entry, dict) or set(entry) != {'executable', 'argv'}:
            raise ValueError('invalid policy entry')
        exe, args = entry['executable'], entry['argv']
        if not isinstance(exe, str) or not exe.startswith('/usr/lib/neebles/module-operations/'):
            raise ValueError('module helper outside OS-owned module operations territory')
        flat = '/usr/lib/neebles/module-operations/' + module + '-platform'
        nested = '/usr/lib/neebles/module-operations/' + module + '/'
        if exe != flat and not exe.startswith(nested):
            raise ValueError('module helper executable identity mismatch')
        s = strict_root_path(Path(exe))
        if not s.st_mode & 0o111:
            raise ValueError('module helper not executable')
        with open(exe, 'rb') as stream:
            if stream.read(4) != b'\x7fELF':
                raise ValueError('helper must be a native ELF')
        if not isinstance(args, list) or len(args) > 32 or not all(isinstance(a, str) and len(a) <= 512 and '\x00' not in a for a in args):
            raise ValueError('invalid root-owned fixed arguments')


def audit_global():
    strict_root_path(SUPPLY_PATH)
    supply = load_json(SUPPLY_PATH)
    entries = check_supply(supply)
    if AUTHORITY in entries:
        if len([x for x in supply['entries'] if x['authority'] == AUTHORITY]) != 1:
            raise ValueError('duplicate platform authority')
        record = next(x for x in supply['entries'] if x['authority'] == AUTHORITY)
        if record['location'] != str(DESCRIPTOR_PATH):
            raise ValueError('conflicting platform authority registration')
        strict_root_path(DESCRIPTOR_PATH)
        if not validate_global_descriptor(load_json(DESCRIPTOR_PATH)):
            raise ValueError('invalid global platform authority descriptor')
    print('PASS AuthoritySupply entries:', len(entries))
    print('PASS global platform authority:', 'present' if AUTHORITY in entries else 'absent')
    return supply, AUTHORITY in entries


def register_global():
    if os.geteuid() != 0:
        raise ValueError('root required for OS authority registration')
    supply, present = audit_global()
    if present:
        print('PASS authority already registered; no changes')
        return
    if not DESCRIPTOR_PATH.is_file():
        raise ValueError('OS-owned descriptor not installed')
    strict_root_path(DESCRIPTOR_PATH)
    if not validate_global_descriptor(load_json(DESCRIPTOR_PATH)):
        raise ValueError('global authority descriptor invalid')
    supply['entries'].append({'authority': AUTHORITY, 'location': str(DESCRIPTOR_PATH)})
    # The existing 10+ platform authorities must be preserved exactly.
    atomic_json(SUPPLY_PATH, supply, 0o644)
    _, yes = audit_global()
    if not yes:
        raise ValueError('registration verification failed')
    print('PASS global authority registered; Boss runtime restart is a SEPARATE approved operation')


def audit_policy(module, source):
    strict_root_path(source)
    data = load_json(source)
    validate_policy(module, data)
    print('PASS policy module:', module)
    print('PASS allowed operations:', ','.join(sorted(data['operations'])))
    return data


def install_policy(module, source, *, update=False):
    if os.geteuid() != 0:
        raise ValueError('root required for OS policy installation')
    data = audit_policy(module, source)
    strict_root_path(POLICY_DIR, 'directory')
    target = POLICY_DIR / (module + '.json')
    if target.exists() or target.is_symlink():
        strict_root_path(target)
        current = load_json(target)
        validate_policy(module, current)
        if current == data:
            print('PASS existing policy identical; no changes')
            return
        if not update:
            raise ValueError('existing policy differs; explicit update-policy authorization required')
    elif update:
        raise ValueError('update-policy requires an existing authenticated policy')
    atomic_json(target, data, 0o600)
    validate_policy(module, load_json(target))
    print('PASS OS policy '+('updated' if update else 'installed')+':', str(target))


def main():
    parser = argparse.ArgumentParser()
    subs = parser.add_subparsers(dest='command', required=True)
    subs.add_parser('audit-global')
    subs.add_parser('register-global')
    for cmd in ('audit-policy', 'install-policy', 'update-policy'):
        p = subs.add_parser(cmd)
        p.add_argument('--module', required=True)
        p.add_argument('--source', required=True, type=Path)
    args = parser.parse_args()
    try:
        if args.command == 'audit-global':
            audit_global()
        elif args.command == 'register-global':
            register_global()
        elif args.command == 'audit-policy':
            audit_policy(args.module, args.source)
        else:
            install_policy(args.module, args.source, update=args.command == 'update-policy')
    except (ValueError, OSError, KeyError, TypeError, json.JSONDecodeError) as error:
        print('BLOCK:', error)
        return 3
    return 0


if __name__ == '__main__':
    sys.exit(main())
