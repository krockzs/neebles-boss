# N.E.E.B.L.E.S. Boss

Current stable release: 1.0.0.

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
    │   └── ui/
    │       └── neebles-ui
    ├── modules/
    └── shared/

Boss and modules remain physically separated.

Modules are installed independently under:

    /opt/neebles/modules/<module>/

## Release

Boss owns its own release composition.

The current stable release publishes:

- neebles-backend
- neebles-ui
- neebles-installer
- neebles-tray
- client-data.tar.gz
- install.sh

The release manifest defines the version, base URL, assets, SHA-256 values, executable flags and launch contract required by the N.E.E.B.L.E.S. injector.

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

OS-specific bootstrap, Calamares resources and OS assets belong to neebles-os.
