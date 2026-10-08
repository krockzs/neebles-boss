#!/usr/bin/env python3
"""Pre-domestication certification of exact CMake DESTDIR product Qt ELFs."""
import argparse
import re
import stat
import subprocess
from pathlib import Path

ALLOWED = {'neebles-ui', 'neebles-installer', 'neebles-auth-agent'}

def inspected(args):
    result = subprocess.run(args, text=True, stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, check=False)
    if result.returncode:
        raise RuntimeError('ELF INSPECTION FAILED: ' + ' '.join(args) + '\n' + result.stdout)
    return result.stdout

def certify(stage: Path, name: str):
    if name not in ALLOWED:
        raise RuntimeError('UNKNOWN QT PRODUCT: ' + name)
    target = stage / 'usr' / 'bin' / name
    if target.is_symlink() or not target.is_file():
        raise RuntimeError('QT DESTDIR BINARY MISSING OR SYMLINKED: ' + str(target))
    mode = target.stat().st_mode
    if not stat.S_ISREG(mode) or not mode & stat.S_IXUSR:
        raise RuntimeError('QT DESTDIR BINARY NOT EXECUTABLE: ' + str(target))
    if target.read_bytes()[:4] != b'\x7fELF':
        raise RuntimeError('QT DESTDIR NOT ELF: ' + str(target))
    header = inspected(['readelf', '-h', str(target)])
    if 'Advanced Micro Devices X86-64' not in header:
        raise RuntimeError('QT DESTDIR WRONG ARCHITECTURE: ' + name)
    program = inspected(['readelf', '-l', str(target)])
    if 'Requesting program interpreter' not in program:
        raise RuntimeError('QT DESTDIR WITHOUT DYNAMIC LOADER: ' + name)
    dynamic = inspected(['readelf', '-d', str(target)])
    if 'Shared library: [libQt6Core.so.6]' not in dynamic:
        raise RuntimeError('QT DESTDIR MISSING Qt6Core NEEDED: ' + name)
    versions = inspected(['readelf', '--version-info', str(target)])
    forbidden = ('Qt_6.10', 'Qt_6.9')
    if any(item in versions for item in forbidden):
        raise RuntimeError('QT DESTDIR HAS FORBIDDEN QT SYMBOL VERSIONS: ' + name)
    dynamic_paths = '\n'.join(line for line in dynamic.splitlines()
                              if '(RPATH)' in line or '(RUNPATH)' in line)
    for marker in ('/work/neebles-', 'neebles-boss-source', 'build_sysroot_6.8.2'):
        if marker in dynamic_paths:
            raise RuntimeError('QT DESTDIR LEAKS BUILD PATH: ' + name + ': ' + marker)
    print('CERTIFIED QT DESTDIR (pre-domestication):', name, target)

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--stage', required=True)
    parser.add_argument('--binary', required=True)
    values = parser.parse_args()
    certify(Path(values.stage).resolve(), values.binary)
