# N.E.E.B.L.E.S. Boss v1.0.32 — CAST30 Active, Open and Settings synchronization

This patch release prepares corrections identified by Fresh Live and Gates128-132. It is not a claim of installed-system acceptance.

## Changes

- Relay committed module changes from short-lived CLI operations to persistent Boss IPC, which broadcasts `module.lifecycle` to the connected UI, Tray and Launcher subscribers.
- Protect the canonical state and its refresh contracts independently of global Launcher/Tray switches and per-module surface visibility.
- Recognize terminated/zombie-only orphaned Tray process groups through guarded `/proc` inspection, without signalling an unauthenticated group or bypassing fail-closed handling of live/unknown members.
- Project canonical Open availability using both the module-owned `governor.open` Lifecycle binding and persistent `RuntimeRegistry` (`closed`/`opening`/`open`). Keep the Boss UI Modules Open button visible, but disable it when opening is unavailable.
- Preserve optional Open SurfaceContent in Launcher and Tray: only module-declared controls appear, and object-bound actions remain module-owned. Prevent duplicate runtime instances via the per-module atomic `begin_opening()` guard, regardless of requesting surface or visibility.

## Source evidence

- Gate131: `cargo test` **810 passed**, 0 failed, 1 ignored; `cargo fmt --check`, `cargo check --locked` and `git diff --check` passed.
- Gate132: 25/25 Active/Settings source contracts passed. 32 visibility/Active matrix rows are specifications only, not run on Live.
- Gate137: 815 Rust tests passed, 0 failed, 1 ignored; 5 new Open tests cover the per-module race and Open projection. This is Rust source certification only; no actual three-GUI synchronization test has run yet.
- Test Module remains **1.2.2** at `ee91d94b1026e7890379c656dbdee2896d00e32b`; CUSTOM V2 remains `c0afe5baa6a58f50166ed12f17a1637a667b4d88`; certified build CUSTOM Classic/Esbirro remains `20488f6818d5e227f043425a682115f614f4bf79`.
- No changes to module packages, Essentials, Test Module, CUSTOM repositories or NEEBLES OS are included.

## Release and final acceptance

The pinned CI workflow must compile, verify and publish exactly 11 assets before the published v1.0.32 release is considered successful. Existing releases, workflows, notes and triggers remain immutable.

Acceptance **after installation in a genuine Live system** must verify all directions of Active/Inactive via Boss UI, Launcher and Tray, with the runtime open and closed, external changes delivered to other surfaces without manual refresh, and settings (both global visual surfaces, per-module visibility, language, notification and telemetry values) preserved/restored. The source-only 32-row matrix does not satisfy Live acceptance.

Open and Active/Inactive are both included in the 1.0.32 source candidate. Live acceptance must prove that opening from Boss UI, Launcher or Tray yields exactly one runtime instance, and that all visible surfaces update after `opening`, `runtime_ready`, failure and `runtime_dead`. If Launcher or Tray is hidden via Boss Settings (globally or per module), it must recover canonical Open state when shown again without changing Active or runtime state. The Open button in Boss UI Modules always exists; optional Launcher/Tray Open controls exist only when declared by module SurfaceContent. These behaviors remain **unverified in Live**, and the release must not be called accepted until they pass.
