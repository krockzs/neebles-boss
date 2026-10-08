# N.E.E.B.L.E.S. Boss v1.0.31 — CAST30 post-Fresh-Live repairs

This is a patch release prepared after Fresh Live audits of v1.0.30. It does not change the Boss/Module/CUSTOM responsibility boundaries.

## Source fixes

- Synchronize asynchronous module runtime unregistration with shutdown before completing an administrative disable, with bounded fail-closed waiting.
- Preserve actual Boss CLI diagnostics when a UI Active operation fails.
- Fix Tray Host selected-module scope and filesystem icon URL composition.
- Permit the Installed switch to initiate governed uninstall while a runtime remains open; Boss still enforces dependency legality, Lifecycle, deactivation and transactional removal.
- Consume Test Module 1.2.2 notification-closed IPC handling through immutable registry selection.

## Immutable module and material selection

- Registry Test Module 1.2.2 source commit: `ee91d94b1026e7890379c656dbdee2896d00e32b`.
- CUSTOM V2 module-material/Construction commit: `c0afe5baa6a58f50166ed12f17a1637a667b4d88`.
- CUSTOM V2 Construction `fetch` and `checkout` select the same Test Module source commit.
- Module-material version is 1.2.2. The 59 Essential and 32 module-delta DEB payloads are unchanged.
- CUSTOM classic/Esbirro release build snapshot remains pinned at `20488f6818d5e227f043425a682115f614f4bf79`.

## Validation evidence available before this release build

- Gate112 Rust source tests: 805 passed, one intentionally ignored; `cargo check --locked`, `cargo fmt --all -- --check` and source contracts passed.
- Test Module 1.2.2: 12 Python tests passed.
- Gates115/117: controlled Qt 6.8.2/G++ 14 Boss UI and Tray Host stage builds passed; Boss UI stage certifier passed.

All final v1.0.31 release assets must be freshly built and verified in the pinned CI workflow. Local source/stage results do not certify the v1.0.31 artifact or Fresh Live.

## Release and acceptance boundary

The new workflow must generate and verify exactly 11 release assets, reject mutating existing published versions, and publish v1.0.31 only when authorized by its own release trigger. The previous v1.0.30 release and historical files remain immutable.

Fresh Live and installed-system acceptance must still validate Active OFF/ON persistence, Installed uninstall/reinstall while runtime is open, runtime unregister ordering, Launcher/Tray surface visibility, notification delivery/close routing and genuine UI diagnostic reporting.
