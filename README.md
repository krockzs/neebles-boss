# N.E.E.B.L.E.S. Boss

Current stable release: 1.0.7.

## Purpose

N.E.E.B.L.E.S. Boss is the central executable responsible for coordinating the N.E.E.B.L.E.S. runtime and its modular architecture.

Boss directs the ecosystem while modules remain independent components responsible for their own functionality.

The installed runtime lives under:

    /opt/neebles/

The global command is:

    neebles

## Runtime layout

    /opt/neebles/
    ├── client/
    │   ├── bin/
    │   │   └── neebles
    │   ├── backend/
    │   │   └── neebles-backend
    │   ├── ui/
    │   │   └── neebles-ui
    │   ├── auth/
    │   │   └── neebles-auth-agent
    │   ├── config/
    │   ├── languages/
    │   ├── assets/
    │   └── systemd/
    ├── modules/
    └── shared/
        ├── settings/
        └── tmp/

Boss and modules remain physically separated.

Modules are installed independently under:

    /opt/neebles/modules/<module>/

Shared user state is stored under:

    /opt/neebles/shared/settings/

## Runtime services

Boss uses a persistent privileged runtime for system-level coordination.

The system runtime is provided by:

    neebles-runtime.service

Its administrative socket is:

    /run/neebles/neebles.sock

Module runtime IPC is exposed through:

    /run/neebles/modules.sock

Desktop tray integration is split into two user-session services:

    neebles-tray-manager.service
    neebles-tray-host.service

The Tray Manager owns the desktop tray runtime, module provider lifecycle and tray IPC.

The Tray Host subscribes to the Tray Manager and exposes visible tray items to KDE Plasma through the StatusNotifierItem D-Bus protocol.

The privileged runtime itself remains owned by root.

Resources that must be consumed by the graphical desktop session, including Boss runtime sockets and shared settings, are assigned to the real desktop user detected during installation.

This keeps privileged execution separated from normal desktop access without exposing Boss runtime resources globally.

## Desktop ownership

During installation, Boss resolves the UID and primary GID of the desktop user that authorized the installation.

The runtime identity is persisted in:

    /etc/neebles/runtime.env

Example:

    NEEBLES_DESKTOP_UID=1000
    NEEBLES_DESKTOP_GID=1000

Boss uses that identity when creating resources that must remain accessible from the desktop session.

The runtime sockets:

    /run/neebles/neebles.sock
    /run/neebles/modules.sock

are private to the desktop user and use mode:

    0600

Shared settings are owned by the desktop user.

The shared settings directory uses private permissions:

    /opt/neebles/shared/settings    0700

Local settings files use:

    local_settings_*.json          0600

Settings files preserve desktop-user ownership even when they are created or updated through a privileged Boss operation.

Privileged writes use an atomic temporary-file-and-rename flow while explicitly preserving the intended owner and permissions.

This prevents runtime and settings state from being unintentionally left owned by root.

## Desktop integration

Boss provides a native desktop identity for KDE Plasma.

The installed desktop entry is:

    /usr/share/applications/org.neebles.Boss.desktop

Its application identity is:

    org.neebles.Boss

The desktop integration defines:

    Exec=/opt/neebles/client/ui/neebles-ui
    Icon=neebles-boss-icon
    StartupWMClass=org.neebles.Boss

The Qt UI also declares the same desktop identity through:

    app.setDesktopFileName("org.neebles.Boss")

The Boss icon is installed globally so Plasma can resolve the application icon consistently for the launcher, taskbar and application window.

The installer treats the desktop entry and icon as managed Boss resources and includes them in install, reinstall and rollback handling.

## Tray architecture

Boss provides generic tray infrastructure without embedding module-specific tray behavior into the core.

The tray flow is:

    module tray provider
            ↓
    Tray Manager
            ↓
    tray.sock
            ↓
    Subscribe / Snapshot / Event
            ↓
    Tray Host
            ↓
    org.kde.StatusNotifierItem
            ↓
    org.kde.StatusNotifierWatcher
            ↓
    KDE Plasma

The user-session tray socket lives under:

    /run/user/<uid>/neebles/tray.sock

The Tray Manager launches and reconciles module tray providers according to the module lifecycle.

Install, update, enable, disable and uninstall operations reconcile tray state after changing module state.

The tray protocol supports:

    Subscribe
    Snapshot
    Event

Tray events include:

    Registered
    Updated
    Unregistered

The Tray Host maintains a persistent subscription to the Tray Manager.

For each visible tray record, the host exports an `org.kde.StatusNotifierItem` object over D-Bus and registers it with:

    org.kde.StatusNotifierWatcher

Tray item actions such as activation, secondary activation and context-menu requests are delegated back to Boss rather than implemented inside KDE-specific module code.

Module providers therefore remain independent from Plasma and D-Bus implementation details.

The Tray Host uses the Rust `zbus` implementation for D-Bus integration.

## Settings contract

Boss treats module and Boss settings as sparse local overrides over declared defaults.

Settings leaves are represented as strings.

Examples include:

    "true"
    "compact"
    "[\"alpha\",\"beta\"]"

The effective value is the local override when present; otherwise the declared default is used.

Writing the same raw string as the default removes the local override.

Updates reconcile local state against the current defaults while preserving valid user overrides and removing stale or default-equivalent values.

Boss also supports a top-level `hardcoded` namespace whose string values may be referenced textually through placeholders such as:

    ${key}

Unknown placeholders remain literal.

## Module contracts

Modules remain independent repositories and declare their own behavior and contracts.

Boss owns the generic mechanisms used to install, enable, disable, update, launch and reconcile modules without hardcoding module-specific semantics.

Installed module state is discovered from:

    /opt/neebles/modules/

Each installed module owns its own directory and manifest.

Boss merges installed state with the remote module registry to present the effective module catalog.

Boss owns lifecycle mechanics.

Modules own their own commands, settings, translations, assets and tray provider behavior.

## Release

Boss owns its own release composition.

The current stable release publishes the Boss runtime and bootstrap assets required by the N.E.E.B.L.E.S. injector, including:

- neebles-backend
- neebles-ui
- neebles-installer
- neebles-auth-agent
- client-data.tar.gz
- install.sh
- bootstrap.json
- SHA-256 verification data

The release manifest defines the version, base URL, assets, SHA-256 values, executable flags and launch contract required by the N.E.E.B.L.E.S. injector.

A rebuilt asset may retain the same release version, but its SHA-256 must always match the exact published artifact.

## 1.0.7 integration patch

The 1.0.7 stable line includes an integration patch that preserves the release version while rebuilding the published runtime assets.

The patch consolidates three related desktop/runtime fixes.

### Desktop-user ownership

Boss runtime resources consumed by the graphical session no longer depend on root ownership.

The installer records the real desktop UID/GID and uses that identity for runtime sockets and shared settings.

This keeps privileged execution privileged while allowing the desktop session to safely consume the Boss resources it owns.

### Boss UI identity and icon

Boss now installs a canonical desktop entry and global application icon.

Qt, the `.desktop` file and Plasma use the same application identity:

    org.neebles.Boss

This prevents Boss from falling back to a generic window/taskbar identity when Plasma resolves the application.

### Tray Host and lifecycle reconciliation

The tray lifecycle now reconciles module state consistently after install, update, enable, disable and uninstall operations.

Boss also includes the Tray Host implementation that bridges the existing Tray Manager protocol to KDE Plasma using StatusNotifierItem over D-Bus.

This completes the intended separation:

    module provider
        ↓
    generic Boss tray runtime
        ↓
    desktop-specific Tray Host
        ↓
    Plasma

The release remains version 1.0.7.

Because the runtime artifacts are rebuilt, every published asset must use the SHA-256 generated from that exact rebuilt artifact.

## Registry

Boss owns its own runtime registry under:

    registry/
    ├── checking.json
    ├── critical-update.json
    └── modules.json

### checking.json

Defines the expected structural state of the current stable Boss runtime.

Checks may describe:

- existence
- type
- path
- permissions
- SHA-256
- symlink target
- restoration information

The checking contract belongs to Boss and does not describe the internal health of independent modules.

### critical-update.json

Describes critical structural migrations of Boss.

When no critical migration is pending, update is null.

### modules.json

Contains the catalog of N.E.E.B.L.E.S. modules visible to Boss.

Modules remain separate repositories and are installed independently from Boss.

Boss is responsible for deciding which compatible stable module release should be used.

## Ownership

N.E.E.B.L.E.S. Boss owns:

- Boss runtime
- Boss releases
- Boss release manifest
- Boss checking contract
- critical Boss migrations
- module catalog
- module compatibility and selection logic
- runtime IPC mechanics
- shared settings persistence and reconciliation
- desktop ownership policy for Boss-managed runtime resources
- Boss desktop identity and application integration
- Tray Manager lifecycle mechanics
- Tray Host and desktop tray integration

OS-specific bootstrap, Calamares resources and OS assets belong to neebles-os.
