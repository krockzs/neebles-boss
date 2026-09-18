#!/usr/bin/env python3

from pathlib import Path
import sys


ROOT = Path(__file__).resolve().parent.parent


def read(relative):
    return (ROOT / relative).read_text()


def require(relative, fragment, description):
    text = read(relative)

    if fragment not in text:
        print(
            "FAIL:",
            description,
            "missing in",
            relative,
        )
        return False

    print(
        "PASS:",
        description,
    )

    return True


def forbid(relative, fragment, description):
    text = read(relative)

    if fragment in text:
        print(
            "FAIL:",
            description,
            "present in",
            relative,
        )
        return False

    print(
        "PASS:",
        description,
    )

    return True


checks = [
    require(
        "client/launcher/contents/ui/main.qml",
        "bossEvents.launcherEnabled",
        "Launcher visibility comes from live Boss events",
    ),

    require(
        "client/spacer/contents/ui/main.qml",
        "bossEvents.launcherEnabled",
        "Spacer visibility comes from live Boss events",
    ),

    forbid(
        "ui/client/bosscontroller.cpp",
        "applyLauncherPanelState",
        "Boss runtime cannot destructively recreate Launcher",
    ),

    forbid(
        "ui/client/bosscontroller.cpp",
        "applyTrayState",
        "Boss runtime cannot stop Tray infrastructure",
    ),

    forbid(
        "ui/client/bosscontroller.cpp",
        "applyTrayServiceState",
        "Boss runtime cannot control Tray services from visual switch",
    ),

    require(
        "src/tray/protocol.rs",
        "SetRootVisibility",
        "Tray protocol has explicit root visibility command",
    ),

    require(
        "src/tray/protocol.rs",
        "RootVisibility",
        "Tray protocol has explicit root visibility event",
    ),

    require(
        "src/tray/protocol.rs",
        "root_visible: bool",
        "Tray snapshot carries root visibility",
    ),

    require(
        "src/tray/host.rs",
        "Option<HostedItem>",
        "Root SNI lifetime is reversible",
    ),

    require(
        "src/tray/host.rs",
        "reconcile_root_item",
        "Root SNI has dedicated reconciliation",
    ),

    require(
        "client/tray-host/main.cpp",
        "BossEventClient::trayEnabledChanged",
        "Qt Tray Host reacts live to global visibility",
    ),

    require(
        "src/ipc.rs",
        '"settings.boss"',
        "Boss publishes settings event stream",
    ),

    require(
        "client/shared/bosseventclient.cpp",
        'QStringLiteral("settings.boss")',
        "Shared Boss listener subscribes to settings stream",
    ),

    require(
        "client/shared/bosseventclient.h",
        "hiddenLauncherModules",
        "Shared Boss listener exposes module Launcher visibility",
    ),

    require(
        "client/shared/bosseventclient.cpp",
        '"hidden_launcher_modules"',
        "Shared Boss listener consumes hidden Launcher modules",
    ),

    require(
        "client/launcher/contents/ui/main.qml",
        "bossEvents.hiddenLauncherModules",
        "Launcher module visibility comes from live Boss events",
    ),

    require(
        "client/launcher/contents/ui/main.qml",
        "onHiddenLauncherModulesChanged",
        "Launcher reacts immediately to module visibility events",
    ),

    forbid(
        "client/launcher/contents/ui/main.qml",
        "neebles config show",
        "Launcher does not poll Boss config for visibility",
    ),

    require(
        "ui/installer/installercontroller.cpp",
        "Launcher and spacer are always installed in Plasma.",
        "Installer always provisions Launcher and Spacer",
    ),

    forbid(
        "ui/installer/installercontroller.h",
        "installLauncherIntoPanel(bool",
        "Installer Launcher provisioning is independent of visual state",
    ),

    forbid(
        "ui/installer/installercontroller.cpp",
        "var enabled = %1;",
        "Installer Launcher integration has no unresolved visibility placeholder",
    ),

    forbid(
        "ui/installer/installercontroller.cpp",
        'QStringLiteral("launcher_enabled")',
        "Installer does not duplicate Launcher visibility configuration",
    ),

    require(
        "ui/installer/installercontroller.cpp",
        "Tray infrastructure is permanent desktop runtime.",
        "Installer treats Tray services as permanent infrastructure",
    ),

    forbid(
        "ui/installer/installercontroller.cpp",
        'QStringLiteral("disable")',
        "Installer integration does not disable Tray services",
    ),
]


if not all(checks):
    print()
    print("SURFACE CONTRACT: FAILED")
    sys.exit(1)


print()
print("SURFACE CONTRACT: VALID")
