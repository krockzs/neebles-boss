# N.E.E.B.L.E.S. Boss

Current development target: `1.0.0`.

## First milestone

The first milestone proves the N.E.E.B.L.E.S. injection path on a client system:

1. `Start NEEBLES` reaches the bootstrap layer.
2. The bootstrap resolves the stable Boss release.
3. The bootstrap launches the minimal NEEBLES installer UI when installation or update is required.
4. The installer places Boss under `/opt/neebles/`.
5. The global `neebles` command becomes available.
6. `neebles --version` returns the installed Boss version.

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

Boss and modules remain physically separated. Modules will be added later under `/opt/neebles/modules/<module>/`.
