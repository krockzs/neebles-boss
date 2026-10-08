# N.E.E.B.L.E.S. Boss v1.0.33 — CAST30 Active, Open/Close and Launcher integration

This is a source candidate, **not** Fresh Live/installed-system acceptance. v1.0.32 remains immutable and published.

## Fresh Live defects addressed in source

- Open was governed with single-flight RuntimeRegistry, but Test Module 1.2.2 kept its IPC session alive after closing its graphical child with X. Updated Test Module 1.2.3 ends the graphical Open session, cooperatively unregisters and disconnects; Boss releases the session and emits `runtime_dead`.
- Active/Inactive was eventually consistent, sometimes taking more than 4 seconds between canonical state write and notification. Boss now broadcasts changed module state as soon as it commits and again after completing the transaction, with compensation notifications as needed; it does not create local GUI authorities.
- Launcher QML formerly lost errors, reused text-command callbacks and could refresh out of order. This release hardens command callbacks, serializes module operations per module, and uses canonical model recovery while visible.
- Boss UI Config > Features places its small Notify button at the right; Launcher gains a larger scrollable five-column module list (icon, two-line name, Active, optional Open and extensible optional controls), plus a compact Open Boss button.

## Ownership and immutable revisions

- Test Module 1.2.3 module source: `308fbf186aa9dc2a2201862f993809799a987fd0`.
- CUSTOM V2 material/Construction source: `d7fa0452fb3e16db85de71e2c7fc6d041fa0a3e5` (2,242 unchanged material entries, no DEB or Python/Tk world changes); the Boss Preinstall transaction resolves the current CUSTOM `main` revision and **records its immutable SHA** in MaterialBinding. This is not a compile-time custom-v2 pin in Boss CI.
- Boss Registry installs Test Module 1.2.3 from its explicit commit above.
- CUSTOM Classic/Esbirro build authority remains pinned at `20488f6818d5e227f043425a682115f614f4bf79` for this workflow.

## Publication and acceptance gates

- Ryzen source gate: version consistency, Rust `cargo test --locked`, `cargo check --locked`, Rust format, diff and exact source scope must pass. QML code changes require controlled Esbirro Qt build in GitHub Actions; static source checks alone do **not** certify QML visual/runtime operation.
- Publish only after explicit source authorization and CI verification of exactly 11 artifacts. This source preparation **does not create `release/trigger-v1.0.33`**.
- Fresh Live: verify Active/Inactive propagation across Boss UI, Launcher and Tray in all 12 source/target combinations, one-click behavior, no stale reversals, hidden/reshown surfaces, and persisted Settings. Verify Open from each **declared** surface, single-flight state `closed → opening → open`, closing app with X, `runtime_dead`, `open_available` recovery, and repeated Open/Close without orphan sessions.
- Boss UI Modules always offers Open as a button (disabled when unavailable); Launcher/Tray Open Surface controls are optional and module-owned. Different modules remain independent.
- Explicitly verify Notify right alignment, Launcher five-column presentation, scroll and larger geometry on the genuine desktop. No Fresh Live approval is implied by Rust/CI success.
