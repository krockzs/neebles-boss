# N.E.E.B.L.E.S. Boss v2.0.7 — Dependency protection in Tray and Launcher

## Changes
- Boss projects the verified installed dependency graph into the shared `boss surface-model` (`dependency_guard`).
- Launcher and Tray hide the disable control for providers with installed dependents and show a red warning naming those dependents.
- An unavailable or invalid dependency snapshot fails closed rather than showing a potentially unsafe toggle.
- Lifecycle `can_disable`, Registry rules, Critical Update, and installed module state remain unchanged.

## Verification
- F15 Rust regression tests: **2 passed, 0 failed** on Ryzen (`dependency_surface_guard_f15_tests`).
- CI must build Rust 1.98.1 and Qt 6.8.2 through controlled Esbirro, certify stage artifacts and publish the exact 11-file candidate including `SHA256SUMS`.
- **Live acceptance pending**: update Boss from 2.0.6, verify that Launcher and Tray hide Disable for Secrets while Connect is installed, and verify Release Notes/Reboot after publication.

## User action
Use NEEBLES Critical Update. Save your work before choosing **Reboot** in the release notes dialog.
