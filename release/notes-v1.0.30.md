# N.E.E.B.L.E.S. Boss v1.0.30 — CAST30

CAST30 integrates the persistent module Governor, canonical Open runtime ownership,
module-wide Launcher/Tray visibility, and the user-session Notification Presenter.

## B3A — Persistent Governor

- Module Governor actions execute through the persistent Boss IPC authority.
- Governed action results carry batched Lifecycle events back to callers.
- Installed contract and module identity remain the authority; callers do not create transitions.

## B3B — Canonical Open

- Closed -> opening -> open -> closed is the canonical authenticated runtime lifecycle.
- Atomic single-flight semantics prevent concurrent duplicate Open runtime births.
- Runtime generation, execution identity and PID-incarnation evidence protect ownership.
- Administrative Open remains available for an installed, enabled module even without optional Launcher or Tray Open surfaces.
- Launcher, Tray and Boss UI converge on the same persistent backend authority.

## B4 — Module-wide visibility

- Launcher and Tray visibility preferences are per-module rather than per-item.
- Explicit per-module visibility overrides legacy item settings.
- The 1.0.29 item representation migrates deterministically, and uninstall cleans module state.
- Presentation visibility does not become a second functional state authority.

## B5 — Desktop Notification Presenter

- Desktop-session environment is resolved from authenticated session authority.
- A dedicated systemd user service runs the Presenter as the actual desktop UID/GID.
- A confined user-runtime socket authenticates the persistent root Boss through kernel SO_PEERCRED.
- Boss retains notification policy, ownership, replacement and return routing.
- Present -> Presented -> Boss ownership registration -> Activate stages return events safely.
- Closed and ActionInvoked signals route back to the authenticated owning runtime.
- Client packaging, installer unit installation, rollback and release source verification include the Presenter.

## Preserved boundaries

- CUSTOM classic remains authoritative for Boss/Calamares certified domestic material.
- CUSTOM V2 owns Essential/module delta material, worlds and Construction declarations.
- MaterialBinding schema 2 and RuntimeLease remain unchanged by CAST30.
- No module technology or Test Module identity is hardcoded into Boss.
- Existing ISO material is not rebuilt merely because Boss is updated.

## Source gate recorded before release preparation

- Boss main: 665 passed.
- Boss library: 120 passed.
- Presenter focused: 3 passed.
- Runtime ownership focused: 15 passed.
- Domesticacion: 13 passed, 1 intentionally ignored.
- Cargo check, Bash syntax, AST and diff integrity gates: passed.

These are local source results, not release-build certification or Fresh Live acceptance.

## Publication and acceptance

The workflow builds Rust with the pinned toolchain and Qt binaries in the controlled
Esbirro Qt 6.8.2 laboratory, packages the 11 Boss release assets, verifies hashes,
and publishes only after its own materialization gates pass.

The 1.0.29 -> 1.0.30 Critical Update path must separately prove that the user-session
Notification Presenter service is enabled and running after a non-graphical update;
merely installing its unit is insufficient. Until that gate is certified, do not
trigger publication.

After publication: Fresh Live and installed-system acceptance must exercise runtime
Open/close/reopen, Launcher/Tray visibility, notification delivery and return events,
service lifecycle, uninstall/reinstall and persistent ownership.
