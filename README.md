# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

**Current source line:** preparation for **1.0.23**
**Latest published Boss release:** **1.0.22**
**Current integration status (2026-10-04):** generic module architecture closed; Test Module connectivity path source-certified; Boss 1.0.23 release preparation and new integrated ISO acceptance remain pending.

N.E.E.B.L.E.S. Boss is the governance, orchestration and runtime-control layer of the N.E.E.B.L.E.S. ecosystem.

> **Boss governs. Consumers declare. Authority is explicit. Technology remains outside Boss whenever a generic contract can describe the requirement.**

Boss owns:

- contracts;
- module transactions;
- Lifecycle execution;
- state and persistence;
- privilege boundaries;
- supplied authority registration and grants;
- domestic execution;
- module Preinstall orchestration;
- module materialization orchestration;
- runtime IPC;
- runtime identity verification;
- settings;
- notifications;
- UI / Launcher / Tray projection;
- Registry consumption;
- Critical Update coordination.

Boss does **not** own:

- module language;
- module framework;
- module package-manager semantics;
- module-specific build instructions;
- CUSTOM material truth;
- OS platform authority semantics;
- BUILD image construction;
- module-specific installation branches.

---

## Architectural law

The permanent authority model is:

```text
physical availability
    -> authenticated source / path
    -> supplied authority descriptor
    -> registered authority
    -> explicit grant
    -> constrained execution
```

Therefore:

```text
AVAILABLE != REQUESTED != REGISTERED != GRANTED != USED
```

Filesystem presence never creates permission.

Host tools are never an implicit fallback authority.

---

## Ecosystem ownership

```text
N.E.E.B.L.E.S. CUSTOM / Esbirro
    -> certified domestic material
    -> package membership
    -> material integrity
    -> domestic runtime worlds
    -> construction declarations
    -> controlled build worlds

N.E.E.B.L.E.S. OS
    -> platform authority semantics
    -> platform providers
    -> AuthoritySupply source truth

N.E.E.B.L.E.S. BUILD
    -> image-side materialization
    -> shared module territory
    -> recovery environment
    -> ISO composition

N.E.E.B.L.E.S. Boss
    -> governance
    -> authentication
    -> registration
    -> grants
    -> Preinstall
    -> materialization orchestration
    -> Lifecycle
    -> runtime IPC
    -> desktop integration

Module
    -> declares identity, contracts, lifecycle and intent
    -> implements its own behavior
```

---

# Module architecture

The current Boss module architecture is fully generic.

A module may be implemented in Python, Rust, Go, Node.js, C++, Java or another technology. Boss must not require recompilation merely because the module changes language, runtime or private implementation.

The central law is:

> **Boss knows how to execute generic capabilities. The module and CUSTOM declare what is needed.**

---

## Schema 4

The canonical module manifest schema is **4**.

Legacy top-level:

```text
manifest.commands
```

is removed and must not return.

The valid dynamic contract type:

```text
commands
```

remains supported through the module contracts namespace.

A module manifest may point to:

- Lifecycle;
- Surfaces;
- Commands;
- Settings;
- Tray provider;
- Notifications protocol;
- runtime entrypoint;
- other future generic contracts.

---

# Lifecycle

Lifecycle is the generic declarative execution model.

A module owns its transition vocabulary.

Examples such as:

```text
install
update
uninstall
enable
disable
open
close
```

are consumer vocabulary, not hardcoded Lifecycle law.

Governor actions bind through:

```text
governor.<action> -> <module transition>
```

The transition name and Governor action do not need to match.

---

## Battlefield and execution

Lifecycle supports dynamic:

- `hardcoded`;
- `require`;
- objects;
- transitions;
- operations;
- artillery;
- objective;
- munition;
- tactics;
- intelligence.

The productive execution model is:

```text
Lifecycle contract
    -> Battlefield interpolation
    -> Battleplan
    -> dependency graph
    -> requested capability
    -> operation-scoped registration
    -> FireControl
    -> CapabilityRegistry
    -> Rust handler
    -> normalized result
    -> intelligence
    -> state / failure / event
```

Lifecycle does not understand Python, Git, Qt, Tk, apt or module-specific technology.

---

# Domestic Construction

CUSTOM owns construction declarations.

Installed declarations live under:

```text
/usr/lib/neebles/domestic/construction/
```

A declaration is selected by:

```text
subject + step
```

Lifecycle reaches construction through the generic capability:

```text
artillery: boss.workspace_execution
objective: construction.step
munition:
    subject: <module>
    step: <step>
```

Boss resolves the declaration, authorities, runtime world, mounts, environment, identity and execution mode generically.

No Test Module-specific branch is allowed in productive Boss code.

---

## Construction execution modes

A construction step must explicitly declare its execution mode:

```text
foreground
persistent
```

`foreground` waits for completion.

`persistent` starts the child, returns control to Lifecycle and leaves a reaper responsible for the process exit.

This is used for long-lived module runtimes without turning Lifecycle into a supervisor.

---

## Session-aware execution

A construction step may explicitly declare:

```text
session: true
```

When requested, Boss resolves the authenticated desktop identity and certified desktop-session interface.

The session projection may include:

- desktop UID/GID;
- `XDG_RUNTIME_DIR`;
- `DBUS_SESSION_BUS_ADDRESS`;
- `DISPLAY`;
- `WAYLAND_DISPLAY`;
- `XAUTHORITY`;
- physical session sockets/files exposed read-only.

The runtime identity is dropped with the certified `boss.setpriv` world before entering the filesystem boundary.

Boss does not inherit arbitrary host desktop environment as authority.

---

# Domestic runtime authority

Runtime manifests are explicit authorities.

Current authority identities include:

```text
boss.runtime
modules.runtime
modules.installed_runtime
```

A Construction declaration references:

```text
runtime_authority
world
```

Boss resolves the named authority and world dynamically.

It does not contain technology-specific matching such as Python, Node.js or Java branches.

---

## Boss runtime

The installed Boss domestic runtime lives under:

```text
/opt/neebles/client/runtime/boss/
```

Its materialized manifest:

```text
/opt/neebles/client/runtime/boss/domestic-runtime.json
```

is generated during the certified release pipeline from Boss domestic contracts plus the pinned CUSTOM runtime corpus.

Examples of Boss runtime worlds include infrastructure tools such as:

```text
boss.git
boss.setpriv
```

The host executable is not the authority.

---

## Module runtime

The shared module runtime authority is supplied from the module material world.

The current Test Module uses:

```text
modules.python3.13-tk
```

That identity belongs to CUSTOM material truth, not to Boss source logic.

Future module worlds can be added without creating `boss.python`, `boss.node`, `boss.java` or equivalent technology branches.

---

# Dynamic read-only authority

Boss supports generic dynamic read-only projection.

A declaration supplies:

```text
authority
source
destination
```

Boss validates the source as a strict subpath of the supplied authority root and lowers it into the existing filesystem boundary.

This allows a module's installed runtime tree to be projected without teaching Boss the module identity.

---

# Filesystem boundary

The filesystem boundary is an OS-owned provider consumed by Boss.

Workspace execution can lower:

- fixed readonly authorities;
- dynamic readonly authorities;
- session readonly resources;
- writable authorities;
- runtime rootfs;
- environment;
- proc/dev/tmp mounts;
- working directory;
- command arguments.

Nested bind destinations are created generically by the boundary provider.

Boss does not copy arbitrary domestic trees to manufacture authority.

---

# Module Preinstall and materialization

A module does **not** download or materialize its own package world.

Correct flow:

```text
module declares requirements
    -> CUSTOM owns package membership and integrity truth
    -> Boss Preinstall ensures exact required DEBs exist
    -> Boss materializes required rootfs material
    -> Lifecycle executes
```

Preinstall is Boss responsibility.

CUSTOM package membership and material integrity remain distinct contracts.

Boss supports native `.deb` extraction for controlled module material:

```text
.deb
    -> ar
    -> data.tar.*
    -> tar
    -> safe path validation
    -> fresh staging
    -> symlink-safe publication
```

There is no host `dpkg-deb` compatibility fallback in the canonical materializer.

---

## Shared package pool law

The shared module package pool is cumulative.

```text
install
    -> reuse verified package when present
    -> download missing required package
    -> reject mismatched package

uninstall
    -> remove module state/runtime
    -> do not remove shared DEBs
```

There is no productive per-module package ownership, refcount or package garbage collection.

---

# Module IPC

The canonical module socket is:

```text
/run/neebles/modules.sock
```

The socket is owned by the authenticated desktop user and protected by exact peer validation.

Runtime registration is bound to kernel/process evidence:

```text
SO_PEERCRED PID
+
/proc/<pid>/stat start_time_ticks
```

PID alone is insufficient.

A module runtime registers:

- module identity;
- session identity;
- declared endpoints;
- live availability.

Runtime advertisement is availability, not declarative authority.

Installed contracts remain the source of endpoint and privilege truth.

---

## Runtime birth

The canonical runtime birth model is:

```text
Governor action
    -> Lifecycle transition
    -> construction.step
    -> runtime authority
    -> world resolution
    -> desktop identity when requested
    -> session projection when requested
    -> filesystem boundary
    -> persistent spawn when requested
    -> Module IPC register
    -> subscribe
    -> runtime serves endpoints
```

The runtime may then execute module-specific UI, Tray or other behavior.

---

# Settings and canonical state

Boss persists Boss-owned and module settings through explicit contracts.

The general law is:

> **persist first, project later**

UI, Launcher and Tray do not own competing functional truth.

For stateful Lifecycle objects:

```text
one functional object
    -> one canonical state
```

Surface state is projection.

---

# Surface projection

Boss UI, Launcher and Tray are presentation consumers over the same governed state/action path.

```text
UI
Launcher
Tray
    -> Boss control path
    -> Governor / Lifecycle
    -> canonical state
    -> surface invalidation
    -> refreshed projections
```

Presentation visibility and functional state remain distinct.

---

# Notifications

Notifications are a governed Boss subsystem.

Module notifications travel through Module IPC.

Boss owns:

- validation;
- policy;
- notification ownership;
- desktop-session presentation;
- replacement ownership;
- return routing.

Module notification product design remains module-owned.

The global Module IPC protocol and Notifications capability protocol remain separate contracts.

---

# Registry and installed truth

Remote Registry, installed inventory and live RuntimeRegistry are separate truths.

```text
Remote Registry
    -> discovery and immutable source selection

Installed inventory
    -> committed module material

RuntimeRegistry
    -> authenticated live runtime/session state
```

Remote membership does not imply installation.

Runtime registration does not imply privilege.

---

# Transactions

Install/update/uninstall remain governed transactions.

An incomplete candidate does not become installed truth.

Update keeps recovery material until publication succeeds.

Rollback and compensation remain explicit.

---

# Critical Update and Nightmare

Boss self-update is a Critical Update.

Critical Update coordinates versioned release instructions.

Nightmare remains a generic declarative transformation engine.

Neither subsystem becomes module construction.

---

# Recovery

Recovery is external to normal Boss operation.

N.E.E.B.L.E.S. BUILD owns:

```text
neebles-check
```

including dynamic module verification.

Boss startup does not depend on running recovery.

---

# Current source certification

The current source closure before Boss 1.0.23 release preparation has certified:

```text
cargo fmt --all -- --check          GREEN
cargo check --locked --all-targets  GREEN
Rust lib suite                      77 / 77
neebles-backend suite               589 / 589
runtime resolver                    3 / 3
domesticacion                       13 pass / 1 intentional ignore
module materialization              GREEN
runtime authority                   GREEN
desktop session projection          GREEN
dynamic readonly authority          GREEN
persistent execution                GREEN
Module IPC peer policy              GREEN
Test Module runtime birth preflight GREEN
git diff --check                    GREEN
```

These source gates do not replace Fresh Live acceptance.

---

# Release policy

The latest published release is **1.0.22**.

The next Boss release is **1.0.23**.

Before publishing 1.0.23 the release must be internally coherent across:

- `Cargo.toml`;
- `Cargo.lock`;
- release workflow;
- trigger;
- release notes;
- pinned CUSTOM revision;
- controlled Qt 6.8.2 build world;
- Rust 1.98.1 release toolchain;
- Boss runtime materialization;
- installer;
- auth agent;
- Launcher;
- Tray Host;
- runtime resolver;
- bootstrap;
- final payload verification.

The release must not be cut while those surfaces disagree.

---

# Fresh Live acceptance boundary

Source certification is not final installed-system certification.

The next integrated image must prove:

```text
new OS authorities present
    -> Boss bootstrap
    -> module discovery
    -> Test Module install
    -> Preinstall
    -> real 47-DEB materialization
    -> installed module
    -> Open
    -> Lifecycle open-runtime
    -> desktop identity/session authority
    -> persistent runtime
    -> Module IPC registration
    -> UI
    -> settings
    -> Tray / notifications where expected
    -> uninstall / reinstall package reuse
```

Only that gate permits the final production-ready claim.

---

# Work protocol

For the current N.E.E.B.L.E.S. workflow:

- shell command blocks must avoid the shell dollar character;
- do not use shell `set`;
- current repository state plus current certification evidence is the source of truth;
- historical dossiers do not override current source;
- do not claim a test passed without current evidence;
- Test Module is a consumer/template, never an architecture driver;
- no deprecated compatibility path is preserved merely for convenience;
- one architecture, one truth.

---

# Final principle

Boss must remain unchanged when a future module changes only its private technology.

If adding a normal new module requires a module-specific branch in Boss, the abstraction has failed.

The intended model is:

> **Declare the module, certify its material/worlds in CUSTOM/Esbirro, expose required OS authority, let Boss govern the generic contract, and keep module implementation outside the core.**
