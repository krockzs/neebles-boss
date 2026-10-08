#!/usr/bin/env python3
"""Fail-closed static product version and CAST30 packaging source gate."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser()
parser.add_argument('--version', required=True)
args = parser.parse_args()
v = args.version

checks = {
    'Cargo.toml': 'name = "neebles-backend"\nversion = "' + v + '"',
    'Cargo.lock': 'name = "neebles-backend"\nversion = "' + v + '"',
    'ui/client/CMakeLists.txt': 'VERSION ' + v,
    'ui/installer/CMakeLists.txt': 'VERSION ' + v,
    'ui/auth-agent/CMakeLists.txt': 'VERSION ' + v,
    'client/launcher-plugin/CMakeLists.txt': 'VERSION ' + v,
    'client/tray-host/CMakeLists.txt': 'VERSION ' + v,
    'ui/client/main.cpp': 'QStringLiteral("' + v + '")',
    'ui/installer/main.cpp': 'QStringLiteral("' + v + '")',
    'client/launcher/contents/ui/main.qml': 'property string bossVersion: "' + v + '"',
    'client/launcher/metadata.json': '"Version": "' + v + '"',
    'client/spacer/metadata.json': '"Version": "' + v + '"',
    '.github/workflows/release-' + v + '.yml': 'release/trigger-v' + v,
    'release/notes-v' + v + '.md': '# N.E.E.B.L.E.S. Boss v' + v,
}
for path, value in checks.items():
    content = (ROOT / path).read_text(encoding='utf-8')
    if content.count(value) != 1:
        raise SystemExit('VERSION GATE FAILED: ' + path + ': ' + value)

for path in ('client/launcher/metadata.json', 'client/spacer/metadata.json'):
    obj = json.loads((ROOT / path).read_text(encoding='utf-8'))
    # Check literal form in addition to parseability to protect published Plasma metadata.
    if not isinstance(obj, dict):
        raise SystemExit('INVALID METADATA: ' + path)

presenter = ROOT / 'client/systemd/neebles-notification-presenter.service'
if 'neebles-backend notifications serve' not in presenter.read_text(encoding='utf-8'):
    raise SystemExit('PRESENTER UNIT FAIL: wrong ExecStart')
if not (ROOT / 'src/notification_presenter.rs').is_file():
    raise SystemExit('PRESENTER SOURCE MISSING')
for path in ('scripts/install.sh', 'scripts/test-install-layout.sh',
             'scripts/verify-release.py', 'ui/installer/installercontroller.cpp'):
    if 'neebles-notification-presenter.service' not in (ROOT / path).read_text(encoding='utf-8'):
        raise SystemExit('PRESENTER WIRING MISSING: ' + path)
workflow_source = (ROOT / ('.github/workflows/release-' + v + '.yml')).read_text(encoding='utf-8')
if 'Stage and certify Boss Qt executables under controlled Esbirro DESTDIR' not in workflow_source:
    raise SystemExit('QT STAGING GATE MISSING')
for build_path in ('--ui neebles-custom/build_sysroot_6.8.2/work/neebles-boss-ui-build/neebles-ui',
                   '--installer neebles-custom/build_sysroot_6.8.2/work/neebles-installer-build/neebles-installer',
                   '--auth-agent neebles-custom/build_sysroot_6.8.2/work/neebles-auth-agent-build/neebles-auth-agent'):
    if build_path in workflow_source:
        raise SystemExit('QT RELEASE MUST NOT CONSUME BUILD TREE: ' + build_path)
for staged_path in ('--ui neebles-custom/build_sysroot_6.8.2/work/neebles-boss-ui-stage/usr/bin/neebles-ui',
                    '--installer neebles-custom/build_sysroot_6.8.2/work/neebles-installer-stage/usr/bin/neebles-installer',
                    '--auth-agent neebles-custom/build_sysroot_6.8.2/work/neebles-auth-agent-stage/usr/bin/neebles-auth-agent'):
    if workflow_source.count(staged_path) != 1:
        raise SystemExit('QT RELEASE CERTIFIED STAGE NOT USED: ' + staged_path)
if not (ROOT / 'scripts/certify-qt-executable-stage.py').is_file():
    raise SystemExit('QT STAGE CERTIFIER MISSING')
installer_script = (ROOT / 'scripts/install.sh').read_text(encoding='utf-8')
for marker in ('PRESENTER_USER_MANAGER_AUTHORITY',
               'activate_desktop_notification_presenter ||',
               '--world boss.setpriv --category executable',
               '--world boss.systemctl --category executable',
               'Refusing to install desktop Presenter for root'):
    if marker not in installer_script:
        raise SystemExit('PRESENTER CRITICAL UPDATE BRIDGE MISSING: ' + marker)
if (ROOT / ('release/trigger-v' + v)).exists():
    print('NOTE: release trigger exists; this checker alone cannot certify runtime readiness')
print('PASS: product version', v, ', presenter packaging and installer source wiring')
