# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

N.E.E.B.L.E.S. Boss is the governance and orchestration layer of the N.E.E.B.L.E.S. ecosystem.

It is not a monolithic application that knows how to do everything. Its job is to define stable boundaries, enforce contracts, coordinate lifecycle, recover dependencies, protect persistent state and give independent modules a predictable runtime in which to operate.

> **Boss governs. A module declares. A module executes.**

The system is intentionally built around that separation.

---

## Status

Current stable release: **1.0.7**.

The `main` branch contains architecture that extends beyond the current stable release line. Stable release artifacts and development-state documentation must therefore be treated as related but distinct views of the project.

The installed runtime lives under:

```text
/opt/neebles/
```

The global command is:

```text
neebles
```

The privileged backend is:

```text
/opt/neebles/client/backend/neebles-backend
```

---

## Design principles

N.E.E.B.L.E.S. Boss follows a small set of rules that shape the whole architecture.

### 1. Governance belongs to Boss

Boss owns cross-cutting concerns that must behave consistently across the ecosystem:

- lifecycle
- dependency integrity
- runtime ordering
- IPC boundaries
- persistent state policy
- module compatibility
- recovery
- desktop ownership
- release/bootstrap contracts
- external reporting boundaries
- persistent-file migration mechanics

### 2. Behavior belongs to modules

Modules own their actual feature behavior, commands, settings, assets, translations and optional runtime providers.

Boss should not need module-specific branches to understand how an independent module performs its work.

### 3. Declarative knowledge is preferred over hardcoded knowledge

Boss contains stable engines and contracts.

Mutable ecosystem knowledge should live in registries, manifests, dictionaries or catalogs whenever possible.

This allows the platform to learn new cases without turning the Boss binary into a growing collection of special cases.

### 4. Local survival comes before remote convenience

A healthy local system must remain usable without Internet access whenever the required state already exists locally.

The general recovery principle is:

```text
usable local state
        ↓
validated cache / bundled recovery data
        ↓
remote data only when local state is insufficient
```

Remote access improves recovery and freshness. It must not become an unnecessary survival dependency.

### 5. Persistent changes are convergent and transactional

When Boss changes persistent state it should produce a complete intended result before publishing it.

A failed intermediate operation should not leave half-written configuration behind.

---

# Architecture overview

At a high level:

```text
                         N.E.E.B.L.E.S. OS
                                │
                                │ bootstrap / declarative data
                                ▼
┌───────────────────────────────────────────────────────────────┐
│                         N.E.E.B.L.E.S. Boss                  │
│                                                               │
│  Stage0        Dependency integrity      Local Installer      │
│     │                   │                       │              │
│     ├───────────────────┼───────────────────────┤              │
│     │                   │                       │              │
│     ▼                   ▼                       ▼              │
│  Runtime ─────────── Module governance ───────── Registry     │
│     │                   │                                      │
│     │                   ├── settings                           │
│     │                   ├── lifecycle                          │
│     │                   ├── language/runtime contracts         │
│     │                   └── tray providers                     │
│     │                                                          │
│     ├── neebles.sock                                           │
│     ├── modules.sock                                           │
│     └── external.sock                                          │
│                                                                │
│                    Nightmare engine                            │
│                         │                                      │
│                         ▼                                      │
│             persistent-file transformations                   │
└───────────────────────────────────────────────────────────────┘
                                │
                                ▼
                       Independent modules
```

Boss is the center of coordination, not the center of every implementation detail.

---

# Runtime layout

The installed layout is organized so the Boss runtime, modules and shared state remain physically separate.

```text
/opt/neebles/
├── client/
│   ├── bin/
│   │   └── neebles
│   ├── backend/
│   │   └── neebles-backend
│   ├── ui/
│   │   └── neebles-ui
│   ├── auth/
│   │   └── neebles-auth-agent
│   ├── config/
│   ├── languages/
│   ├── assets/
│   └── systemd/
├── modules/
│   └── <module>/
└── shared/
    ├── settings/
    ├── cache/
    └── tmp/
```

Modules are installed independently under:

```text
/opt/neebles/modules/<module>/
```

Shared local settings live under:

```text
/opt/neebles/shared/settings/
```

Validated reusable recovery/cache data may live under:

```text
/opt/neebles/shared/cache/
```

---

# Boot and Stage0 convergence

N.E.E.B.L.E.S. does not treat service startup as equivalent to system readiness.

Before the normal runtime is allowed to proceed, **Stage0** performs the convergence checks required to establish a usable local state.

Logical contract:

```text
service.stage0
```

Systemd unit:

```text
neebles-stage0.service
```

Backend entry point:

```text
neebles-backend stage0 run
```

Stage0 writes its runtime state to:

```text
/run/neebles/stage0.json
```

The runtime is ordered behind Stage0 rather than racing it.

Stage0 is responsible for checking the critical dependency surface and module preflight state needed by the platform before reporting readiness.

Its design follows a simple rule:

> **Internet access may improve recovery, but a healthy local installation must not be blocked merely because the network is unavailable.**

The systemd startup chain uses Stage0 as a real readiness barrier rather than a cosmetic boot step.

---

# Dependency integrity and recovery

Boss models dependency state explicitly instead of reducing everything to “installed” or “missing”.

The dependency layer preserves information such as:

- dependency name
- detected version
- required version
- whether the requirement is satisfied
- whether repair was attempted
- state before repair
- state after repair

This allows Boss to distinguish between:

- missing dependencies
- installed but incompatible dependencies
- healthy dependencies
- repairable dependencies
- failed repairs

Required dependencies may block the operation when their contract cannot be satisfied.

Optional dependencies do not unnecessarily block an otherwise healthy system.

After repair, Boss re-verifies the dependency instead of assuming that an installer command succeeded semantically.

---

# Local Installer

Boss contains a generic local-installer engine used to resolve declarative installation and repair operations.

The engine does not hardcode every distribution-specific command into dependency-resolution logic.

Instead, it consumes a validated installer dictionary supplied by the N.E.E.B.L.E.S. OS repository.

Current AMD64 dictionary path in the OS repository:

```text
config/installers/instaladores_amd64.json
```

Installed/bundled and cached dictionary locations include:

```text
/opt/neebles/client/config/installers/instaladores_amd64.json
/opt/neebles/shared/cache/installers/instaladores_amd64.json
```

The resolver validates schema and architecture before using dictionary content.

Remote resolution is pinned to a concrete Git commit before raw content is downloaded. Boss does not intentionally execute installer knowledge from a floating `main` URL.

The cache publication path uses temporary-file creation, validation and atomic rename semantics.

This keeps the installer mechanism stable while allowing declarative recovery knowledge to evolve separately.

---

# Module architecture

Modules remain independent repositories and independent runtime components.

Boss owns the mechanics needed to:

- discover
- install
- update
- enable
- disable
- uninstall
- launch
- reconcile
- preflight
- validate compatibility

A module owns its own feature behavior.

Installed state is discovered from:

```text
/opt/neebles/modules/
```

The effective module catalog is built from installed state plus the remote Boss registry.

Boss does not need to know the internal implementation language of a module in order to govern it. The boundary is contract-driven; concrete module language/runtime support is declared through module manifests and Boss language/runtime adapters.

That makes the module architecture language-agnostic at the governance boundary without coupling Boss to one application language.

---

## Module lifecycle

Lifecycle transitions are treated as observable platform events rather than isolated filesystem mutations.

Install, update, enable, disable and uninstall operations reconcile the dependent runtime state after changing module state.

This includes integration surfaces such as tray providers and settings state.

Uninstall operations can require confirmation and are designed not to silently destroy preserved user settings that may be needed during later reinstall.

Reinstall behavior reconciles new defaults with compatible preserved local state rather than blindly restoring obsolete configuration.

---

## Module dependency contracts

Modules may declare minimum compatible module versions.

Boss performs centralized compatibility checks before treating the dependency as usable.

A missing module and an incompatible installed module are different states and are handled differently.

The compatibility boundary is centralized so Stage0, install and update paths do not each invent their own interpretation of the same requirement.

---

# IPC architecture

N.E.E.B.L.E.S. separates IPC surfaces by responsibility.

The architecture intentionally avoids one giant socket carrying unrelated trust and lifecycle semantics.

## `neebles.sock`

Administrative/runtime IPC:

```text
/run/neebles/neebles.sock
```

Used for privileged Boss coordination.

## `modules.sock`

Module runtime IPC:

```text
/run/neebles/modules.sock
```

This is the governed module-facing boundary used for module requests, events and module-owned interactions.

Nightmare is an internal Boss capability and does not require its own socket. A module that needs Nightmare reaches it through the normal governed module path:

```text
module
  ↓
modules.sock
  ↓
Boss
  ↓
Nightmare
  ↓
filesystem
```

A dedicated Nightmare socket would only be justified by a future real process/security boundary such as a separate daemon, sandbox or privilege domain.

## `external.sock`

Outward-reporting boundary:

```text
/run/neebles/external.sock
```

External reporting is intentionally separated from administrative and module IPC.

The transport uses Unix `SOCK_SEQPACKET` semantics so one packet maps to one envelope.

The receiver applies packet-size limits and validates the systemd-activated socket before accepting traffic.

Peer authorization uses `SO_PEERCRED` and permits the expected root/desktop identities rather than trusting payload-declared identity.

Socket mode and peer credentials are defense in depth; they are not presented as cryptographic module identity attestation.

---

# External Envelope

Boss defines a generic outward-facing envelope:

```json
{
  "type": "<String>",
  "endpoint": "<String>",
  "activate": false,
  "package": {},
  "message": {}
}
```

`type` and `endpoint` are intentionally open strings.

`package` and `message` must be JSON objects.

The `activate` flag is a hard producer/receiver guard. When disabled, producers must return before performing expensive payload construction, serialization, socket work or network-oriented dispatch.

Current generic message families include:

- error
- incompatibility
- Stage0 state/reporting

Producers exist at the Stage0, Boss and module boundaries.

External reporting is best-effort and must not turn a healthy local runtime into a network-dependent runtime.

The receiver currently validates and consumes the local envelope boundary; remote dispatch policy remains intentionally separate from the transport contract.

---

# Settings model

Boss treats settings as persistent sparse local overrides over declared defaults.

Settings leaves are represented as strings.

Examples:

```text
"true"
"compact"
"[\"alpha\",\"beta\"]"
```

The effective value is the local override when present; otherwise the declared default is used.

Writing the same raw value as the default removes the local override.

During reconciliation Boss preserves compatible user state, removes stale/default-equivalent values and applies hardcoded platform values with higher precedence.

Boss supports a top-level `hardcoded` namespace whose string values may be referenced textually through placeholders such as:

```text
${key}
```

Unknown placeholders remain literal.

The persistent model is recursive and supports Object/String trees rather than a fixed flat schema.

---

## Module-owned settings IPC

Modules may own their settings behavior while Boss retains the generic persistence and transport mechanics.

Settings events are isolated by module ownership so one module does not become the implicit settings authority for another.

The module IPC layer supports subscriptions so runtimes can observe settings changes without polling global files.

---

# Nightmare persistent-file migration engine

Nightmare is Boss's generic persistent-file migration engine.

Its purpose is not to know JSON, INI, XML, `.env`, a proprietary application format or any other specific file language.

Its purpose is to know **how to apply a formula**.

> **Boss does not need to understand a file format. It needs to understand the formula that describes how that file is interpreted and reconstructed.**

This moves format-specific knowledge out of the stable engine and into declarative catalog data.

---

## PersistentDocument

Nightmare begins with one abstract document contract containing four objects with four distinct responsibilities:

```json
{
  "descriptor": {},
  "content": {},
  "meta": {},
  "formula": {}
}
```

### `descriptor`

Describes what kind/class/identity of persistent object is being treated.

### `content`

Carries the raw material.

The current base contract requires:

```json
{
  "content": {
    "raw": "complete file contents as a String"
  }
}
```

All supported persistent files enter Nightmare as a complete string rather than as a language-specific Rust type.

### `meta`

Reserved for treatment context such as version/preservation/rules and future migration metadata.

The engine does not prematurely freeze an unnecessary internal schema for this object.

### `formula`

Identifies the declarative formula used to interpret and reconstruct the raw content.

Current base contract:

```json
{
  "formula": {
    "id": "nightmare.example.v1"
  }
}
```

---

## Nightmare formula catalog

The external catalog is named:

```text
insert.nightmare.json
```

It belongs to the N.E.E.B.L.E.S. OS repository, not to the compiled Boss binary.

Repository path:

```text
config/nightmare/insert.nightmare.json
```

Cached runtime path:

```text
/opt/neebles/shared/cache/nightmare/insert.nightmare.json
```

The catalog may validly begin empty:

```json
{
  "formulas": {}
}
```

Formulas are added when Boss or real modules need them. The engine does not need a synthetic catalog full of hypothetical formats.

Each formula contains at least:

```json
{
  "formulas": {
    "nightmare.example.v1": {
      "reader": {},
      "writer": {}
    }
  }
}
```

Formula data is declarative. Nightmare does not evaluate arbitrary shell code, `eval` strings or dynamically supplied executable snippets as a formula language.

---

## Catalog resolution

Nightmare prefers a usable validated local cache.

If no usable cache exists it resolves the N.E.E.B.L.E.S. OS `main` branch to a concrete Git SHA and only then constructs the raw catalog URL.

Conceptually:

```text
validated cache
      ↓ if unavailable
resolve neebles-os main → commit SHA
      ↓
download insert.nightmare.json @ SHA
      ↓
validate
      ↓
publish cache atomically
      ↓
use catalog
```

Floating branch names are not accepted as the raw catalog revision contract.

The cache is written through a temporary file created with private permissions and `O_NOFOLLOW`, then synchronized and atomically renamed into place.

---

## Reader

A resolved formula exposes a reader object.

The current minimal reader primitives are:

- `record_separator`
- `field_separator`
- `trim`
- `ignore_prefixes`

The reader transforms the raw string into a logical ordered document.

Nightmare deliberately uses an ordered field representation instead of a hash map so it can preserve:

- field order
- duplicate keys
- deterministic occurrence behavior

Logical representation:

```text
LogicalDocument
└── fields[]
    ├── key
    └── value
```

Field splitting uses the declared separator once, allowing values to contain the same character without being destroyed by an uncontrolled split.

Records that do not match the active formula are rejected rather than silently discarded.

---

## Writer

The writer performs the reverse operation.

Current minimal writer primitives include:

- `record_separator`
- `field_separator`
- `final_record_separator`

The writer converts a complete logical document into one complete final string.

Nightmare does not partially patch the target file during logical transformation.

---

## Logical operations

Nightmare currently supports deterministic logical operations:

- `set`
- `add`
- `remove`
- `rename`

Current semantics:

```text
set    → update all matching occurrences
add    → append a new occurrence
remove → remove all matching occurrences
rename → rename all matching occurrences
```

Operations are applied transactionally in memory to a cloned logical document.

If a later operation fails, the original logical document remains unchanged.

Removing a field that is already absent is convergent/idempotent.

---

## Preserve semantics

Persistent migration often requires two different authorities:

```text
OLD
→ user state

TARGET
→ current structure
```

Nightmare therefore provides explicit preservation semantics.

The rule is:

> **TARGET owns structure. OLD owns preserved values.**

A field removed from the new target structure is not resurrected merely because it existed in the old file.

For duplicate preserved keys, values are mapped by occurrence. If the target contains more occurrences than the old source, the final preserved source value is reused deterministically.

This makes preservation predictable while still allowing the new version to define the authoritative shape of the configuration.

---

## Atomic publication

After Nightmare has produced the complete intended string, publication happens atomically.

Flow:

```text
existing target
      ↓
validate regular file / reject symlink
      ↓
create temporary file in same directory
      ↓
write complete final String
      ↓
preserve intended UID / GID / mode
      ↓
fsync temporary file
      ↓
rename over target
      ↓
fsync parent directory
```

The target is not incrementally edited in place.

Nightmare refuses direct symlink targets and refuses publication through a symlink parent directory in the atomic replacement path.

For replacement of an existing file, ownership and permissions are preserved rather than accidentally converting desktop-user configuration into root-owned state.

---

## Nightmare end-to-end flow

The complete engine can be summarized as:

```text
insert.nightmare.json
        ↓
    formula.id
        ↓
  reader contract
        ↓
OLD raw String ───────┐
                      ├── logical documents
TARGET raw String ────┘
                      ↓
             preserve / operations
                      ↓
               writer contract
                      ↓
              complete new String
                      ↓
               atomic replacement
```

The engine stays stable while the catalog can continue to grow with real formulas.

---

# Desktop ownership and runtime identity

The privileged runtime itself remains owned by root.

Resources that must be consumed by the graphical desktop session are assigned to the real desktop identity detected during installation.

Boss persists that identity in:

```text
/etc/neebles/runtime.env
```

Example:

```text
NEEBLES_DESKTOP_UID=1000
NEEBLES_DESKTOP_GID=1000
```

Boss uses that identity when creating resources that must remain accessible from the desktop session.

Runtime sockets such as:

```text
/run/neebles/neebles.sock
/run/neebles/modules.sock
```

are private to the intended desktop user and use restrictive permissions.

Shared settings directory:

```text
/opt/neebles/shared/settings
```

uses private ownership and permissions.

Local settings files use restrictive file permissions and preserve the intended desktop-user ownership even when updated through privileged Boss operations.

---

# Desktop integration

Boss provides a native KDE Plasma desktop identity.

Installed desktop entry:

```text
/usr/share/applications/org.neebles.Boss.desktop
```

Application identity:

```text
org.neebles.Boss
```

Desktop integration defines the Boss executable, canonical icon and startup WM identity so launcher, taskbar and application window resolve consistently.

The installer treats the desktop entry and icon as managed Boss resources and includes them in install, reinstall and rollback handling.

---

# Tray architecture

Boss provides generic tray infrastructure without embedding module-specific tray behavior into the core.

```text
module tray provider
        ↓
Tray Manager
        ↓
tray.sock
        ↓
Subscribe / Snapshot / Event
        ↓
Tray Host
        ↓
org.kde.StatusNotifierItem
        ↓
org.kde.StatusNotifierWatcher
        ↓
KDE Plasma
```

User-session tray socket:

```text
/run/user/<uid>/neebles/tray.sock
```

The Tray Manager owns module tray-provider lifecycle and reconciles provider state after module lifecycle changes.

The Tray Host maintains a persistent subscription and exposes visible tray records to KDE Plasma through StatusNotifierItem over D-Bus.

Module providers remain independent of Plasma and D-Bus implementation details.

The Tray Host uses Rust `zbus` for the desktop D-Bus bridge.

---

# Registry and declarative knowledge

Boss owns its runtime registry under:

```text
registry/
├── checking.json
├── critical-update.json
└── modules.json
```

## `checking.json`

Defines the expected structural state of the current stable Boss runtime.

Checks may describe:

- existence
- type
- path
- permissions
- SHA-256
- symlink target
- restoration information

The checking contract belongs to Boss and does not pretend to describe every internal health rule of independent modules.

## `critical-update.json`

Describes critical structural migrations of Boss.

When no critical migration is pending, the update value is null.

## `modules.json`

Contains the module catalog visible to Boss.

Modules remain separate repositories and are installed independently from Boss.

Boss is responsible for deciding which compatible stable module release should be selected.

---

# Release architecture

Boss owns its release composition and bootstrap contract.

The stable release line publishes runtime/bootstrap assets such as:

- `neebles-backend`
- `neebles-ui`
- `neebles-installer`
- `neebles-auth-agent`
- `client-data.tar.gz`
- `install.sh`
- `bootstrap.json`
- SHA-256 verification data

The release manifest defines the version, base URL, assets, SHA-256 values, executable flags and launch contract required by the N.E.E.B.L.E.S. injector.

A rebuilt asset may retain a release version only when the release process intentionally does so; its SHA-256 must always describe the exact published artifact.

The stable 1.0.7 line includes the desktop/runtime integration work for desktop ownership, canonical Boss application identity and Tray Host lifecycle reconciliation.

Development `main` now contains additional architecture beyond that stable line, including Stage0/dependency hardening, the External boundary and Nightmare.

---

# Security model

Boss uses multiple narrow controls rather than assuming one mechanism is sufficient.

Examples include:

- restrictive Unix socket permissions
- explicit desktop runtime identity
- `SO_PEERCRED` checks for the external boundary
- packet-size limits
- systemd socket validation
- no trust in payload-declared peer identity
- regular-file checks before persistent replacement
- symlink rejection in sensitive write paths
- `O_NOFOLLOW` for temporary/cache creation
- complete-file publication through atomic rename
- preserving intended ownership and permissions
- avoiding arbitrary executable formula content in Nightmare
- validating remote declarative data before caching or consuming it
- pinning remote raw catalog/dictionary reads to concrete Git commits

No single item is presented as absolute security. They are layered controls intended to reduce accidental privilege leakage and narrow the useful attack surface.

---

# Ownership boundaries

## Boss owns

- Boss runtime
- Boss releases and release manifest
- Boss checking contract
- critical Boss migrations
- module catalog and module selection logic
- module lifecycle mechanics
- module compatibility enforcement
- Stage0 convergence
- dependency integrity and repair orchestration
- Local Installer engine
- runtime IPC mechanics
- `neebles.sock`
- `modules.sock`
- `external.sock`
- External Envelope contract
- shared settings persistence and reconciliation
- module settings IPC/subscriptions
- runtime identity policy
- desktop ownership policy for Boss-managed resources
- Boss desktop identity/application integration
- Tray Manager lifecycle mechanics
- Tray Host desktop integration
- Nightmare persistent-file migration engine

## Modules own

- module feature behavior
- module commands
- module-specific settings declarations
- module translations
- module assets
- module tray-provider behavior
- module-specific persistent-file intent/formula selection when applicable

## `neebles-os` owns

- OS bootstrap
- Calamares resources
- OS assets
- OS-specific installation composition
- installer dictionary data
- Nightmare formula catalog (`config/nightmare/insert.nightmare.json`)

This boundary is intentional: stable engines live in Boss; ecosystem knowledge that can evolve independently lives in declarative repositories.

---

# Current architectural contract

The shortest useful description of Boss today is:

```text
Boss governs
    ↓
contracts define boundaries
    ↓
modules remain independent
    ↓
Stage0 establishes readiness
    ↓
dependency engines converge the local system
    ↓
IPC surfaces separate responsibilities
    ↓
settings preserve user state
    ↓
Nightmare transforms persistent files from declarative formulas
    ↓
remote knowledge can evolve without turning Boss into a monolith
```

The project is deliberately designed so adding a new module, dependency recipe or persistent-file formula does not require teaching the core a new special case every time.

That is the central architectural goal of N.E.E.B.L.E.S. Boss.
