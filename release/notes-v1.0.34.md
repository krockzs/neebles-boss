# N.E.E.B.L.E.S. Boss v1.0.34 — Multi-RootFS candidate

## Scope

- Source integration of authenticated Multi-RootFS lifecycle: Preinstall, Install, Update, Rollback, and RuntimeLease.
- Physical composed-rootfs prepublication recertification and staging tests.
- Version synchronized across Cargo, Qt executables, Installer, Auth Agent, Tray Host, Launcher and Plasma metadata.

## Current verification

- Rust unit/binary tests, formatting and locked all-target checks passed before this version migration.
- The 1.0.34 version, release build, generated artifacts, and VM integration are **not yet certified**.
- Full packaging and installation rights/UID/GID require further verification.

## Publication policy

The 1.0.34 workflow builds and verifies a candidate, but does **not** publish a GitHub release. Publication requires a separate explicit authorization and a separate controlled action. Do not create or modify release/trigger-v1.0.34 until authorized.
