# N.E.E.B.L.E.S. Boss 1.0.15

**The Boss Contract release.**

Boss 1.0.15 is the architectural closure of N.E.E.B.L.E.S. Boss: the first published Boss line carrying the complete generic module governance, authority, Lifecycle, domestic execution, IPC, Settings, Surface, Notifications, privilege, Registry and construction architecture developed after 1.0.14.

## Architectural closure

- Points 1 through 8 are GREEN / CLOSED and the Boss contract is closed.
- Generic Lifecycle execution is productive, including Governor bindings, operation preparation, dependency orchestration, FireControl, CapabilityRegistry execution, normalized results, communication, state, failure handling, rollback and compensation.
- AVAILABLE, REQUESTED, REGISTERED, USED and GRANTED remain separate authority/capability states.
- Functional state, presentation state, runtime availability and persisted Settings remain separate truths.
- Stage 8 host independence remains permanent: physical availability does not imply authority.
- Telemetry is reduced to an optional generic error-reporting channel; disabled telemetry emits no telemetry payload and normal Boss operation requires no remote collector.

## Authority and domestic execution

- Platform AuthoritySupply is authenticated, registered and explicitly granted before use.
- Domestic executable, library, environment, filesystem and search authority are governed through controlled worlds.
- Boss now ships a permanent domestic runtime contract instead of relying on host dependency discovery.
- `neebles-runtime-resolve` provides the bootstrap/runtime resolver boundary.
- `boss-runtime.tar.gz` transports the certified Boss runtime with `domestic-runtime.json` plus its relocatable rootfs.
- Runtime transport rewrites and certifies domestic symlink relations instead of preserving host-root assumptions.
- Generic writable workspace execution is exposed through `boss.workspace_execution` / `construction.step`.

## Module platform

- Module Manifest Schema 4 is canonical.
- Top-level `manifest.commands` is removed and rejected; dynamic command contracts are canonical.
- Module IPC uses authenticated runtime/session identity and declared endpoint validation.
- RuntimeRegistry owns live runtime availability only; it does not own installation, Settings or privilege policy.
- The Module Governor provides transactional install, update, uninstall, enable and disable flows.
- Inter-module `require` includes recursive dependency handling, transaction journaling and reverse compensation.
- Universal module Preinstall consumes certified CUSTOM package membership and SHA integrity without teaching Boss module technology.
- Remote Registry, installed inventory and live RuntimeRegistry remain separate sources of truth.
- Registry installation pins immutable module revisions.

## Settings, Surfaces and desktop integration

- Module Settings use one Boss-governed persistent truth with realtime projection to consumers.
- Persist-first / project-later semantics are used for module Settings writes.
- Reinstall preserves validated Settings and update reconciles compatible local state with current defaults.
- SurfaceContent is the generic projection contract for Boss UI, Launcher and Tray.
- Presentation visibility is separate from global module Active/Inactive state.
- Launcher exposure comes from dynamic command contracts and canonical surfaces.
- Desktop-session execution transports only explicitly authorized graphical-session inputs.
- UI, Launcher and Tray remain presentation consumers rather than independent functional-state authorities.

## Notifications and Critical Update

- Notifications protocol v4 is governed through Module IPC.
- Module notification ownership is bound to exact module/session identity.
- Notification replacement, ACK and return routing preserve exact ownership.
- Boss presents through `org.freedesktop.Notifications` using the governed desktop-session interface; the legacy `notify-send` transport is removed.
- Boss self-update is owned by Critical Update.
- Critical Update consumes the release-owned `critical-update-manifest.json`.
- Zero-byte Critical Update manifests remain valid and mean that the release requires no Critical Update instructions.
- Nightmare remains a generic transformation engine and is consumed through the Critical Update adapter without becoming release-specific logic.

## CUSTOM v2 and Esbirro boundary

- CUSTOM owns certified domestic material, module membership/integrity data and Domestic Construction declarations.
- Esbirro remains the declarative certification layer over the generic domestic/authority machinery.
- Lifecycle does not domesticate runtimes and does not interpret module technology.
- OS defines platform authority, BUILD materializes it, Boss authenticates/grants/consumes it, and CUSTOM owns certified material.

## Test Module integration

- Point 9 Test Module adaptation is GREEN / CLOSED.
- Test Module 1.2.0 consumes the closed Boss contract through Schema 4, dynamic Commands, Lifecycle, Module IPC, canonical Settings, Surfaces, Tray, Notifications v4, Preinstall and Domestic Construction.
- The final Test Module revision `e5bd6a8c5ef13cc8a25f129ac105bb88bb37111c` is immutably aligned across Test Module HEAD, Boss Registry and CUSTOM construction.
- Test Module remains a communication/integration template rather than a technology template.

## Removed legacy architecture

This release does not revive the deprecated application paths removed during the 1.0.15 line, including:

- Stage0 preflight architecture;
- Local Installer;
- the old `dependencies.rs` dependency model;
- top-level `manifest.commands`;
- fixed Lifecycle open/close semantics;
- module-level `launcher_action` compatibility projection;
- host tool discovery as semantic authority;
- apt/dpkg module semantics;
- `notify-send` as the Boss Notifications transport;
- hardcoded host AuthoritySupply fallback paths.

## Release materialization

The 1.0.15 release pipeline now produces and verifies the final materialized payload rather than manually assembling the legacy 1.0.14 asset contract.

Build provenance:

- Rust toolchain `1.98.1`;
- controlled Qt `6.8.2` snapshot from N.E.E.B.L.E.S. CUSTOM revision `1efc6cb304d8e7d739849c266a1d33cc1e92dc54`, with its 115-package snapshot SHA-verified before compilation;
- certified Boss runtime material from N.E.E.B.L.E.S. CUSTOM revision `1efc6cb304d8e7d739849c266a1d33cc1e92dc54`;
- Test Module integration pinned to revision `e5bd6a8c5ef13cc8a25f129ac105bb88bb37111c`.

Published assets:

- `neebles-backend`
- `neebles-ui`
- `neebles-installer`
- `neebles-auth-agent`
- `client-data.tar.gz`
- `install.sh`
- `neebles-runtime-resolve`
- `boss-runtime.tar.gz`
- `critical-update-manifest.json`
- `bootstrap.json`
- `SHA256SUMS`

`bootstrap.json` is generated from the actual materialized assets and carries their SHA-256 identities. `SHA256SUMS` is generated from the same final payload.

## Acceptance boundary

Boss 1.0.15 closes the Boss architecture and Point 9 module adaptation. Full Point 10 Test Module functional certification and final installed N.E.E.B.L.E.S. OS / VM acceptance remain downstream acceptance work and are not falsely claimed by this release.
