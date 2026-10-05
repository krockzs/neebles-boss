# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

**Current source line:** **1.0.26 preparation**
**Latest published Boss release recorded by the repository:** **1.0.25**
**Current integration status (2026-10-05):** **Point 1 CLOSED / GREEN. Point 2 CLOSED / GREEN at source level.** MaterialBinding + RuntimeLease are the productive module-material architecture. Tray providers now enter through module-owned Construction, `modules.runtime`, authenticated RuntimeLease material, desktop-session authority and governed process ownership. The legacy persistent shared module rootfs/runtime-manifest model and direct host Tray-provider spawn path are retired. **Fresh Live and installed-system acceptance remain pending. No new Boss release is claimed by this document.**

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
N.E.E.B.L.E.S. CUSTOM classic
    -> certified Boss runtime corpus
    -> certified Calamares runtime corpus
    -> controlled Boss/Calamares build material

N.E.E.B.L.E.S. CUSTOM V2
    -> certified module package material
    -> global Essential layer
    -> per-module package deltas
    -> per-module membership + integrity manifests
    -> module domestic runtime/world truth
    -> module Construction declaration truth

N.E.E.B.L.E.S. Esbirro
    -> controlled engineering/certification workspace
    -> creation and certification of worlds/material
    -> portable restoration of controlled laboratories

N.E.E.B.L.E.S. OS
    -> platform authority semantics
    -> platform providers
    -> AuthoritySupply source truth

N.E.E.B.L.E.S. BUILD
    -> image-side materialization
    -> recovery environment
    -> ISO composition

N.E.E.B.L.E.S. Boss
    -> governance
    -> authentication
    -> registration
    -> grants
    -> Preinstall
    -> MaterialBinding ownership
    -> RuntimeLease ownership
    -> Lifecycle
    -> runtime IPC
    -> desktop integration

Module
    -> declares identity, contracts, lifecycle and intent
    -> implements its own behavior
```

**CUSTOM classic and CUSTOM V2 are not interchangeable names.** Point 1 changed the module-material path owned by CUSTOM V2 and Boss; it did not redefine the classic Boss/Calamares corpus.

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

`modules.runtime` is a governed runtime authority, **not** a persistent shared rootfs.

The productive model is:

```text
permanent certified package pools
        +
persistent MaterialBinding for the installed module
        -> modules.runtime execution
        -> fresh RuntimeLease
        -> Essential layer + module delta
        -> authenticated domestic-runtime.json
        -> generic Workspace execution
        -> process exit
        -> RuntimeLease drop
```

The current reference world remains:

```text
modules.python3.13-tk
```

That identity belongs to CUSTOM V2 material truth, not to Boss source logic. Boss does not gain Python/Tk-specific branches.

### Essential layer

Essential is a **global material layer**, not a fake module and not a module-specific recipe.

The current certified split is:

```text
Essential                59 DEBs
Test Module delta        32 DEBs
```

The delta must not duplicate material already owned by Essential merely because a module consumes it.

### Persistent MaterialBinding

After Preinstall, Boss persists a module-specific authenticated MaterialBinding containing the exact material authority needed to recreate the runtime later.

Its identity binds at least:

```text
module identity
installed module version
CUSTOM V2 revision
```

The binding preserves authenticated authority blobs for Essential membership/integrity, module membership/integrity and the runtime manifest payload. It does **not** duplicate the `.deb` files themselves; those remain in permanent package pools.

This prevents a future execution from accidentally combining an installed old module with unrelated "latest" module-runtime metadata.

### Ephemeral RuntimeLease

For `modules.runtime`, Construction uses the module subject to request a fresh RuntimeLease.

A lease:

```text
loads the active MaterialBinding
    -> creates a new private temporary territory
    -> materializes Essential + module delta into lease/rootfs
    -> writes the authenticated domestic-runtime.json
    -> validates the runtime manifest
    -> verifies manifest.root resolves to lease/rootfs
    -> hands the absolute manifest to generic Workspace
```

The lease owns its temporary rootfs. Drop destroys it.

Two concurrent executions receive physically independent leases. There is no global rootfs symlink swap and no mutable global runtime manifest.

Lifetime is part of correctness:

```text
foreground
    -> retain lease through command.status()

persistent
    -> move lease with the child waiter
    -> retain through child.wait()
```

`modules.installed_runtime` remains a separate read-only authority for the installed module tree under `/opt/neebles/modules`. It must not be repurposed as `modules.runtime` or as the composed runtime rootfs.

### Module material territories

The productive Boss-side territories are:

```text
/opt/neebles-build/modules/packages/essentials/
    -> permanent verified Essential DEB cache

/opt/neebles-build/modules/packages/
    -> permanent verified module-delta DEB cache

/opt/neebles-build/modules/material/<module-id>/
    -> persistent authenticated MaterialBinding

/opt/neebles-build/modules/runtime-leases/
    -> ephemeral independent runtime leases
```

The following legacy shape is **not** productive architecture:

```text
/opt/neebles-build/modules/rootfs
shared mutable domestic-runtime.json
```

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

A module does **not** download, authenticate or materialize its own package world.

The productive flow after Point 1 is:

```text
module declares requirement
    -> CUSTOM V2 owns exact module material truth
    -> Boss Preinstall authenticates the selected CUSTOM V2 revision
    -> Boss validates Essential package membership + integrity
    -> Boss validates module-delta package membership + integrity
    -> Boss validates the module runtime manifest payload
    -> Boss reuses/downloads exact certified DEBs into permanent pools
    -> Boss produces MaterialBindingInput
    -> install/update transaction activates the MaterialBinding
    -> Lifecycle may execute
    -> modules.runtime creates a RuntimeLease only when runtime is actually needed
```

Preinstall **does not publish a composed shared module rootfs** and does not publish a mutable shared runtime manifest.

CUSTOM V2 package membership and material integrity remain distinct contracts.

Boss supports native `.deb` extraction for controlled module material:

```text
.deb
    -> ar
    -> data.tar.*
    -> tar
    -> safe path validation
    -> fresh staging
    -> certified metadata verification
    -> strict layer merge into a new destination
```

There is no host `dpkg-deb` compatibility fallback in the canonical materializer.

---

## Shared package pool law

Module packages are a cumulative reusable arsenal split by material role:

```text
Essential pool
    -> /opt/neebles-build/modules/packages/essentials/

module-delta pool
    -> /opt/neebles-build/modules/packages/
```

For either pool:

```text
existing file + correct SHA
    -> reuse

missing required file
    -> obtain exact certified payload

existing file + wrong SHA
    -> reject
```

A later module may reuse an already-certified Essential or module package without downloading it again when identity and SHA match.

Uninstall removes module state/binding as required by the transaction but **does not erase the permanent DEB arsenal**.

There is no productive per-module package ownership, package refcount or package garbage collection.

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

The canonical runtime birth model for a module is now:

```text
Governor action
    -> Lifecycle transition
    -> construction.step
    -> runtime authority selection
    -> if modules.runtime: load active MaterialBinding
    -> create independent RuntimeLease
    -> materialize Essential + module delta
    -> validate lease domestic-runtime.json
    -> world resolution
    -> desktop identity when requested
    -> session projection when requested
    -> filesystem boundary
    -> foreground/persistent spawn
    -> retain RuntimeLease for the real process lifetime
    -> Module IPC register
    -> subscribe
    -> runtime serves endpoints
```

Workspace remains generic. It receives an absolute valid manifest and executes; it does not know about MaterialBinding, module package pools or RuntimeLease semantics.

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

Install/update/uninstall remain governed transactions, and MaterialBinding is now part of that same truth.

An incomplete candidate does not become installed truth.

### Install

```text
Preinstall
    -> prepare authenticated MaterialBinding staging
    -> module transaction succeeds
    -> activate_install()
```

Activation refuses to silently replace an unexpected active binding.

### Update

Update carries both filesystem publication and MaterialBinding state through rollback.

The certified physical outcomes are:

```text
PreviousRestored
    -> previous module path is active
    -> previous binding remains/restores active

NewPreserved
    -> previous version could not be restored
    -> new module path remains active
    -> new binding remains/finalizes active

NoActiveModule
    -> neither previous nor new module path is active
    -> no binding may claim an active installed module
```

The rollback classification is derived from the filesystem reality that survives the failed operation, not from a guessed textual phase.

### Uninstall

Binding removal is staged transactionally. If uninstall does not finalize, the previous binding can be restored. Once uninstall is committed and the module is no longer active, the active binding is removed.

This keeps installed module state, physical module state and authenticated runtime-material truth synchronized.

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

Point 1 and Point 2 closure were followed by directed security gates and full regression on the exact working tree.

Current Point 2 closure evidence:

```text
git diff --check                       GREEN

desktop-session runtime grants        2 / 2 GREEN
Construction Point 2 directed tests   5 / 5 GREEN
Tray ownership directed tests         5 / 5 GREEN

cargo test --lib                      116 / 116 GREEN
cargo test --bin neebles-backend      583 / 583 GREEN
cargo check --lib                     GREEN
cargo check --bin neebles-backend     GREEN
```

The binary check currently emits **87 known non-blocking warnings**, primarily historical Lifecycle `dead_code` / naming warnings. Point 2 does not authorize an unrelated warning-cleanup refactor.

Point 1 source closure certifies:

- Essential + per-module delta material composition;
- persistent MaterialBinding;
- authenticated ephemeral RuntimeLease;
- concurrent independent RuntimeLeases;
- Construction projection into generic Workspace;
- install/update/uninstall MaterialBinding transaction semantics.

Point 2 source closure additionally certifies:

- Tray contracts declare a generic `construction_step`;
- Tray providers are not directly executed from the host path;
- Tray provider birth uses module-owned Construction;
- `modules.runtime` creates and retains the authenticated RuntimeLease;
- the declared world remains module/CUSTOM V2 truth, never Boss technology knowledge;
- session-local readonly resources are granted by `platform.desktop_session_interface`;
- Workspace re-authenticates session readonly grants before boundary composition;
- dynamic Construction environment injection is sealed to the `NEEBLES_*` namespace;
- host execution inputs such as `PATH`, `LD_PRELOAD`, `LD_LIBRARY_PATH` and `PYTHONPATH` cannot be injected through that overlay;
- Tray runtime ownership uses PID incarnation identity: `pid + start_time_ticks`;
- the Construction supervisor owns a dedicated process group;
- lifecycle stop signals only the authenticated governed group;
- loss or change of authenticated leader identity becomes UNKNOWN/UNSAFE and cannot authorize group signaling;
- Tray registration authenticates the real provider PID through kernel `SO_PEERCRED`;
- stale waiters cannot erase ownership for a newer Tray runtime.

**Point 1 and Point 2 are CLOSED / GREEN at source level.**

These source gates do not claim a published Boss release and do not replace later Fresh Live or installed-system acceptance.

---

# Release policy

The repository now carries **1.0.26 preparation** surfaces in source/workflow metadata. The latest published Boss release recorded by the current repository documentation is **1.0.25**.

This README deliberately does **not** promote a preparation version into a release claim.

Current sequencing is:

```text
Point 1
    -> CLOSED / GREEN

Point 2
    -> CLOSED / GREEN at source level

README / repository truth
    -> current work

next Boss version preparation
    -> pending

release Actions + publication
    -> pending

Fresh Live acceptance
    -> pending

installed-system acceptance after Calamares
    -> pending
```

Before the next release is cut, the repository must be audited for coherent versioning and release surfaces, including:

- `Cargo.toml` / `Cargo.lock`;
- all Boss version constants and generated metadata;
- current release workflow and trigger;
- release notes;
- release asset/tag names;
- pinned CUSTOM/CUSTOM V2 revisions actually required by the release;
- controlled Qt 6.8.2 build world where applicable;
- Rust release toolchain;
- Boss runtime materialization;
- installer/auth agent/Launcher/Tray Host/runtime resolver/bootstrap;
- final payload verification.

A source preparation number is not, by itself, release authority.

---

# Fresh Live acceptance boundary

Source certification is not final integrated-system certification.

The first acceptance phase uses **one Test Module only**.

```text
Fresh Live
    -> Boss install
    -> Test Module install
    -> Preinstall authenticates Essential + module delta
    -> permanent package pools are correct
    -> active MaterialBinding matches installed module truth
    -> Launcher exposes Open and declared controls
    -> Tray exposes the module and its declared controls
    -> Open enters Lifecycle
    -> construction.step selects open-runtime
    -> modules.runtime creates independent RuntimeLease
    -> Essential + Test Module delta compose in the lease rootfs
    -> desktop identity/session authority
    -> persistent runtime retains RuntimeLease for real process lifetime
    -> Module IPC registration
    -> UI opens
    -> Tray provider enters through tray-provider Construction
    -> Tray provider registers through kernel peer identity
    -> settings and switches communicate Boss <-> module
    -> state changes are reflected through canonical surfaces
    -> disable / enable works
    -> uninstall cleans runtime ownership and module state
    -> reinstall succeeds
```

After the complete Fresh Live sequence is GREEN:

```text
Calamares install
    -> boot installed N.E.E.B.L.E.S. OS
    -> repeat the complete single-module acceptance sequence
```

Only after the single-module Live + installed-system cycle is GREEN will the reference Test Module be cloned into another repository under a **different module identity**, for example `test-module-2`.

The complete acceptance battery will then be repeated with both modules simultaneously to prove:

```text
two installed module identities
    -> both project independently in Launcher and Tray
    -> both can Open
    -> independent MaterialBindings
    -> independent RuntimeLeases
    -> shared Essential material
    -> no mutable shared rootfs
    -> isolated settings and switches
    -> uninstall/reinstall of one does not disturb the other
    -> repeat after Calamares installation
```

The obsolete phrase **real 47-DEB materialization** is not current architecture.

Current certified material truth:

```text
Essential layer          59 DEBs
Test Module delta        32 DEBs
runtime composition      Essential + module delta
ownership                MaterialBinding + RuntimeLease
```

Only those later integrated gates permit an installed-system production-ready claim.

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
