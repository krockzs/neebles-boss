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
    │   └── assets/
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

The desktop Tray Manager runs as a user service:

    neebles-tray-manager.service

The privileged runtime remains owned by root, while its runtime sockets are assigned to the desktop user that owns the N.E.E.B.L.E.S. installation.

This keeps privileged execution separated from normal desktop access without exposing the sockets globally.

## Desktop ownership

During installation, Boss records the UID and primary GID of the desktop user that authorized the installation.

The runtime identity is persisted in:

    /etc/neebles/runtime.env

The runtime uses that identity to assign access to N.E.E.B.L.E.S. runtime sockets.

Shared settings are owned by the desktop user.

Settings files preserve that ownership even when they are created or updated by a privileged Boss operation.

Local settings files use private permissions and are written atomically through a temporary file followed by rename.

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

OS-specific bootstrap, Calamares resources and OS assets belong to neebles-os.
