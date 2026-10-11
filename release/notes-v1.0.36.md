# N.E.E.B.L.E.S. Boss v1.0.36 — Critical Update handoff and release-notes candidate

## Scope

- Boss in-place upgrade: post-install handoff to close the authenticated old Qt UI, restart only the Plasma shell service and relaunch the updated UI.
- Persistent user-owned Critical Update receipt and localized one-time Boss release-notes modal after a verified version transition.
- Module update path: validate staged release notes and show localized notes only after verifying the installed version transition; no Boss or Plasma restart for ordinary module updates.
- Keep the existing CUSTOM classic / CUSTOM V2 authority separation and the controlled Esbirro Qt 6.8.2 DESTDIR certification path.
- Synchronize version 1.0.36 across Rust, Qt clients, Installer, Auth Agent, Tray Host, Launcher plugin and Plasma metadata.

## Verification status before release

- Rust tests: 896 passed, 2 ignored; release backend and static MUSL resolver compiled.
- Controlled Esbirro Qt 6.8.2: Boss UI, Installer, Auth Agent, Launcher plugin and Tray Host built and staged, with pre-domestication certification.
- Domestic Boss runtime transport: certified; local 1.0.36 release candidate materialized and passed `verify-release.py`.
- In-place installed Boss 1.0.35 → 1.0.36 Critical Update, one-time modal, restart ordering and real module update flow remain unverified.
- Qt QML import warnings and UTF-8 locale warnings occurred during controlled chroot build; runtime acceptance must assess them.

## Critical Update acceptance

- Keep installed Boss 1.0.35 unchanged until candidate, rollback and publication are approved.
- Verify installed target version, receipt provenance, authenticated UI shutdown, Plasma shell-only restart and new UI launch.
- Confirm es_CL/en_US Boss notes appear once after a true upgrade and not on a clean install or no-op.
- Test module 1.2.3 clean install (no notes), then 1.2.4 true upgrade (localized notes) with tray operation and no Boss/Plasma restart.

## Publication policy

The release workflow produces a candidate artifact; it does not automatically publish a GitHub Release. Do not create or modify `release/trigger-v1.0.36` before candidate preflight approval. Publishing is a separate, explicitly authorized step after candidate certification.
