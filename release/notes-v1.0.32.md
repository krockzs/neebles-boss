# N.E.E.B.L.E.S. Boss v1.0.32 — CAST30 Active and Settings synchronization

This patch release prepares corrections identified by Fresh Live and Gates128-132. It is not a claim of installed-system acceptance.

## Changes

- Relay committed module changes from short-lived CLI operations to persistent Boss IPC, which broadcasts `module.lifecycle` to the connected UI, Tray and Launcher subscribers.
- Protect the canonical state and its refresh contracts independently of global Launcher/Tray switches and per-module surface visibility.
- Recognize terminated/zombie-only orphaned Tray process groups through guarded `/proc` inspection, without signalling an unauthenticated group or bypassing fail-closed handling of live/unknown members.

## Source evidence

- Gate131: `cargo test` **810 passed**, 0 failed, 1 ignored; `cargo fmt --check`, `cargo check --locked` and `git diff --check` passed.
- Gate132: 25/25 Active/Settings source contracts passed. 32 visibility/Active matrix rows are specifications only, not run on Live.
- Test Module remains **1.2.2** at `ee91d94b1026e7890379c656dbdee2896d00e32b`; CUSTOM V2 remains `c0afe5baa6a58f50166ed12f17a1637a667b4d88`; certified build CUSTOM Classic/Esbirro remains `20488f6818d5e227f043425a682115f614f4bf79`.
- No changes to module packages, Essentials, Test Module, CUSTOM repositories or NEEBLES OS are included.

## Release and final acceptance

The pinned CI workflow must compile, verify and publish exactly 11 assets before the published v1.0.32 release is considered successful. Existing releases, workflows, notes and triggers remain immutable.

Acceptance **after installation in a genuine Live system** must verify all directions of Active/Inactive via Boss UI, Launcher and Tray, with the runtime open and closed, external changes delivered to other surfaces without manual refresh, and settings (both global visual surfaces, per-module visibility, language, notification and telemetry values) preserved/restored. The source-only 32-row matrix does not satisfy Live acceptance.

Open synchronization is deliberately deferred until Active/Settings acceptance is closed.
