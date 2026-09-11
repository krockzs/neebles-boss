# N.E.E.B.L.E.S. Boss

Current stable release: `1.0.0`.

## Purpose

N.E.E.B.L.E.S. Boss is the central executable responsible for the N.E.E.B.L.E.S. runtime and its modular architecture.

The Boss is distributed as a stable release and is installed under `/opt/neebles/`. The global `neebles` command is provided by the installed client runtime.

## Stable 1.0.0

Boss `1.0.0` is the current stable release referenced by the N.E.E.B.L.E.S. bootstrap registry.

The release provides the client, backend, UI and installer assets required by the bootstrap/install path.

## Runtime layout

```text
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
```

Boss and modules remain physically separated. Modules are installed independently under `/opt/neebles/modules/<module>/`.

## Release assets

The stable release publishes the runtime assets consumed by the registry bootstrap:

- `neebles-backend`
- `neebles-ui`
- `neebles-installer`
- `install.sh`

The registry records the release version and SHA-256 values for these assets before they are consumed by the bootstrap process.
