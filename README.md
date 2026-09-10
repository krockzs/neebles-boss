# N.E.E.B.L.E.S. Boss

Initial development target: `0.0.1`.

## First milestone

The first milestone proves the N.E.E.B.L.E.S. injection path on a client system:

1. `Start NEEBLES` reaches the bootstrap layer.
2. The bootstrap launches the minimal NEEBLES installer UI.
3. The installer places Boss under `/opt/neebles/`.
4. The global `neebles` command becomes available.
5. `neebles --version` returns the installed Boss version.

## Runtime layout

```text
/opt/neebles/
├── boss/
│   ├── bin/
│   ├── backend/
│   └── ui/
├── modules/
└── shared/
```

Boss and modules remain physically separated. Modules will be added later under `/opt/neebles/modules/<module>/`.
