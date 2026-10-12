# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

**Versión del código fuente en desarrollo:** **1.0.51**.
**Última versión estable de referencia:** **1.0.41** (publicada y recuperable).
**Estado de 1.0.43 (2026-10-12):** correcciones de fuente integradas localmente; pendientes de compilación certificada, publicación y aceptación funcional.

### Cambios incorporados en el código de 1.0.42

- **Critical Update sin reinicio completo de Plasma:** el handoff intenta renovar de forma selectiva Launcher, Spacer y los servicios Tray, manteniendo `plasmashell` en ejecución. Pendiente de confirmar en una actualización real que los plasmoides, sus preferencias y el escritorio sobreviven correctamente.
- **Startup de Boss UI:** preferencias separadas de inicio automático y de inicio minimizado, disponibles en **Config**, con traducciones `es_CL` y `en_US`; unidad `neebles-boss-ui-autostart.service` del usuario y camino `neebles start-session`. La apertura manual con `neebles start` sigue siendo visible. Pendiente de compilación y prueba tras reinicio de NEEBLES OS.
- **Privilegios:** revisión de las rutas de autorización sin modificaciones a Polkit ni al Auth Agent. Las operaciones administrativas siguen requiriendo privilegios donde corresponde.

**Límite de aceptación:** la presencia de código, release notes y comprobaciones estáticas no demuestra compilación ni correcto comportamiento en Plasma.

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

CUSTOM V2 owns module Construction declaration truth.

For an installed module, the productive authority chain is:

```text
exact pinned CUSTOM V2 revision
    -> runtime/construction/<module-id>.json
    -> Boss Preinstall fetch
    -> UTF-8 + schema parse
    -> declaration.subject == module identity
    -> MaterialBinding schema 2
    -> construction.json + authenticated SHA256
    -> productive Construction resolver
```

There is no productive static image-side Construction declaration territory.

Construction selection is based on two identities:

```text
Governor-owned module identity
    +
module-declared step identity
```

Lifecycle reaches Construction through the generic capability:

```text
artillery: boss.workspace_execution
objective: construction.step
munition:
    step: <step>
```

The module does **not** provide its own Construction subject through munition.

Governor injects `module.id` into the execution Battlefield. `PreparedOperation` carries that identity as `module_id`, and workspace execution fails closed if the Governor-owned identity is absent.

The workspace capability accepts exactly one munition key: `step`. Unknown keys are rejected before authority use.

Boss then resolves the authenticated Construction declaration from the active MaterialBinding, selects the requested step, resolves AuthoritySupply, runtime world, mounts, environment, identity and execution mode generically.

No Test Module-specific branch is allowed in productive Boss code.

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

After Preinstall, Boss persists a module-specific authenticated MaterialBinding containing the exact material authority required to recreate that installed module runtime later.

The current binding schema is:

```text
schema 2
```

Its identity binds:

```text
module identity
installed module version
CUSTOM V2 revision
Essential package selector + manifest
module-delta package selector + manifest
domestic-runtime.json
construction.json
```

Every stored authority blob has a SHA256 recorded in `binding.json`, including the Construction declaration.

The binding directory has exact membership validation. Missing, foreign or tampered authority files fail closed.

The binding does **not** duplicate the `.deb` files themselves; those remain in permanent package pools.

Persisting both runtime-manifest and Construction truth prevents a future execution from combining an installed old module with unrelated later CUSTOM V2 metadata.

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

RuntimeLease composition preserves certified filesystem semantics rather than flattening package content into unrelated copies. In particular, controlled TAR hardlinks are reconstructed inside package staging and remain hardlinks when the staged layer is merged into the candidate/composed lease rootfs.

Materialization never follows a destination-parent symlink as construction authority. Existing destination parents are validated component-by-component and any symlink parent or non-directory parent fails closed before publication.

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
    -> CUSTOM V2 owns exact Essential + module-delta material truth
    -> CUSTOM V2 owns exact module Construction truth
    -> Boss Preinstall authenticates the selected CUSTOM V2 revision
    -> Boss validates Essential package membership + integrity
    -> Boss validates module-delta package membership + integrity
    -> Boss validates domestic-runtime.json
    -> Boss validates Construction subject against module identity
    -> Boss reuses/downloads exact certified DEBs into permanent pools
    -> Boss produces MaterialBindingInput including runtime + Construction payloads
    -> install/update transaction activates MaterialBinding schema 2
    -> Lifecycle may execute
    -> Construction is loaded from that active MaterialBinding
    -> modules.runtime creates a RuntimeLease only when runtime is actually needed
```

Preinstall **does not publish a composed shared module rootfs** and does not publish a mutable shared runtime manifest.

### Raw material URL transport

CUSTOM V2 package filenames are opaque physical identities. Boss must never reinterpret filename text as URL structure.

The canonical transport law is:

```text
authenticated physical filename
    -> validate controlled path structure
    -> encode each path segment independently
    -> HTTPS transport
    -> one URL decoding layer
    -> exact original physical filename
```

This is especially important for Debian package filenames containing an encoded epoch. A physical filename such as:

```text
bsdutils_1%3a2.41.5-0+deb13u1_amd64.deb
```

must travel through the raw GitHub URL with the literal percent encoded as `%25`:

```text
bsdutils_1%253a2.41.5-0+deb13u1_amd64.deb
```

Boss constructs CUSTOM raw URLs with `url::Url` path-segment semantics. Revision identity is constrained to an exact 40-character hexadecimal commit and path structure rejects empty, current-directory, parent-directory and control-character segments.

The 1.0.27 transport gate certifies:

- 91 / 91 currently declared Essential + Test Module package URLs against the immutable CUSTOM revision;
- SHA256 equality for all 8 current literal `%3a` package filenames;
- old literal transport reproduces HTTP 404 while segment-aware transport returns HTTP 200;
- all 94 permitted printable ASCII filename characters round-trip as filename data;
- all 512 upper/lower `%00` through `%FF` escape-looking forms remain literal filename text;
- all 33 ASCII control characters are rejected fail-closed;
- 151,739 ordered sensitive-token single/pair/triple combinations round-trip exactly;
- encoded slash, encoded dot traversal, query, fragment, space, backslash, Unicode and reserved delimiters cannot escape their filename segment.

The fix changes transport representation only. Package identity, package membership, SHA authority, Essential/module-delta ownership and MaterialBinding semantics remain unchanged.

CUSTOM V2 package membership and material integrity remain distinct contracts.

Boss supports native `.deb` extraction for controlled module material:

```text
.deb
    -> ar
    -> data.tar.*
    -> controlled TAR parsing
    -> confined archive-entry paths
    -> fresh package staging
    -> deferred hardlink resolution
    -> certified metadata verification
    -> confined layer merge
    -> hardlink identity preservation
    -> fresh RuntimeLease rootfs
```

The canonical TAR materialization law is generic:

```text
regular / contiguous / GNU sparse
    -> controlled file material

symlink
    -> opaque link metadata
    -> never dereferenced as materialization authority

hardlink
    -> target normalized inside archive root
    -> target may appear before or after the link entry
    -> unresolved, escaping or non-regular target fails closed
    -> real filesystem hardlink reconstructed

character device / block device / FIFO / unsupported global PAX / unknown
    -> fail closed

merge destination parent
    -> existing parent chain must remain real directories
    -> symlink parent rejected before publication
```

Hardlink handling is package-agnostic. Boss contains no Perl-specific branch; `perl-base` is the real certified regression gate because it exposed the missing generic TAR hardlink semantic during Fresh Live.

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

Lifecycle may invoke an installed module Commands contract through the generic capability:

```text
artillery: boss.module_ipc
objective: commands
munition:
    endpoint: <logical-command>
```

The module identity comes only from Governor-owned execution context. The capability accepts the exact `endpoint` munition and resolves the installed Commands contract before invoking the authenticated runtime. A module cannot supply another subject identity through munition.

---

## Runtime birth

The canonical runtime birth model for a module is now:

```text
Governor action
    -> Governor injects module.id
    -> Lifecycle transition
    -> boss.workspace_execution / construction.step
    -> munition.step only
    -> load authenticated Construction from active MaterialBinding
    -> select declared Construction step
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

Boss Config now also carries a dynamic `features` inventory. The physical Settings leaf remains a string and contains serialized JSON keyed by module identity and Surface item identity.

The productive feature source is the installed module Surface contract:

```text
installed module
    -> surface: ui
    -> item_id
    -> typed require declaration
    -> Boss Config features inventory
```

This inventory is presentation/configuration metadata. It does not become a second owner of Lifecycle object state.

Install materializes the module inventory. Update and rollback reconcile it against the surviving installed contract. Uninstall removes that module inventory. Startup rebuild removes ghosts and reconstructs Features only from valid installed modules.

---

# Surface projection

Boss UI, Launcher and Tray are presentation consumers over the same governed state/action path.

Surface schema **2** adds typed presentation requirements:

```text
require:
    self: active | open
    modules:
        <module-id>: active | open
```

`active` means installed + enabled.

`open` means active + an exact authenticated runtime registration in the shared RuntimeRegistry.

The `open` requirement is fail-closed. Backend authorization queries RuntimeRegistry strictly; display helpers such as process-running observations are not authorization authority.

Requirements are gates only. They never auto-install, auto-enable or auto-open another module.

Presentation materialization preserves the raw requirement declaration and adds the evaluated `requirements_met` result. An unmet item may remain visible but disabled.

The backend revalidates requirements again when an action is invoked. QML therefore does not become requirement-policy authority.

The persistent Boss owns both routes:

```text
surface-model
    -> current Surface projection
    -> strict requirement evaluation

surface-action <owner> <item_id> <action>
    -> reload installed Surface declaration
    -> derive the declared target
    -> revalidate requirements
    -> execute governed Lifecycle target
```

The caller cannot invent an object, transition or undeclared action.

Module-owned `label_key` values are resolved from that module language dictionary. Boss global translations do not replace module translation authority.

Presentation ownership is:

```text
surface: ui
    -> Boss Config -> Features

surface: launcher
    -> Launcher

surface: tray
    -> Tray Host
```

The Modules tab remains administrative: install, uninstall, activate, deactivate and Open. It is not the module personalization surface.

Launcher and Tray inventories use bounded list presentation rather than unbounded dynamic Repeaters.

The canonical execution path remains:

```text
UI / Config / Launcher / Tray
    -> persistent Boss control path
    -> Governor / Lifecycle
    -> canonical state
    -> refreshed Surface projection
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

A module Feature may therefore reach Notifications without creating a second notification authority:

```text
Surface action
    -> Lifecycle
    -> boss.module_ipc
    -> installed Commands endpoint
    -> authenticated module runtime
    -> Module IPC notification request
    -> Boss notification policy / ownership
    -> desktop presentation
```

For stateful Feature transitions, canonical object state is committed only after the governed Lifecycle execution succeeds.

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

# Evidencia histórica de cierre local (versiones anteriores)

Point 1 corrective work and the existing Point 2 source architecture were checked on the exact working tree before this commit.

These are **local source verification gates**, not Esbirro certification, not a published Boss release and not Fresh Live / installed-system acceptance.

Evidencia de cierre local registrada para una línea anterior (no representa las pruebas actuales de 1.0.42):

```text
git diff --check                                  GREEN
module_materialization focused suite             14 / 14 GREEN
Boss library suite                               120 / 120 GREEN
Boss main suite                                  647 / 647 GREEN
domesticacion executed tests                      13 GREEN
domestic-root certification test                   1 intentionally ignored
cargo check --locked                              GREEN
cargo build --release --locked --bins             GREEN
real certified perl-base SHA256 gate              GREEN
perl/perl5.40.1 staging hardlink identity         GREEN
perl/perl5.40.1 merged hardlink identity          GREEN
destination symlink-parent escape gate            GREEN
```

The real regression package is:

```text
perl-base_5.40.1-6+deb13u1_amd64.deb
SHA256 b795464137a0f4d443fc9284f4b93e883fb83883cb533adf300ac660807a352a
```

Its `usr/bin/perl5.40.1` entry is a TAR hardlink to `usr/bin/perl`. The controlled 1.0.28 materializer preserves that relationship after extraction and after layer merge.

Point 1 source closure now verifies:

- Essential + per-module delta material composition;
- MaterialBinding schema 2;
- authenticated `construction.json` stored inside the binding with SHA256;
- Preinstall Construction fetch from the exact pinned CUSTOM V2 revision;
- Construction subject validation before binding publication;
- productive Construction resolution from active MaterialBinding;
- authenticated ephemeral RuntimeLease;
- confined native TAR hardlink materialization inside RuntimeLease staging;
- preservation of hardlink identity across staging -> candidate/composed layer merge;
- destination-parent symlink confinement during preflight and publication;
- fail-closed rejection of unsupported special TAR entry types;
- concurrent independent RuntimeLeases;
- install/update/uninstall MaterialBinding transaction semantics;
- Governor-owned module identity propagation into prepared Lifecycle operations;
- workspace execution accepting exactly `munition.step` and rejecting module-supplied subject identity.

Point 2 source closure additionally verifies:

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

**Point 1 corrective source closure and Point 2 source closure are locally GREEN.**

The Config/Features source closure additionally verifies:

- Surface schema 2 typed `require` declarations;
- strict `active` and RuntimeRegistry-backed `open` resolution;
- requirement preservation through Projection -> SurfaceContent -> JSON;
- persistent `surface-model` requirement evaluation;
- persistent `surface-action` execution keyed by owner + item_id + action;
- backend requirement revalidation before Lifecycle execution;
- module-scoped Surface translations;
- dynamic Config -> Features inventory materialization;
- install/update/rollback/uninstall/startup Feature reconciliation;
- Modules remaining administrative-only;
- bounded Launcher and Tray presentation lists;
- generic Lifecycle `boss.module_ipc` Commands execution;
- Test Module Notify button and Notify switch tutorial behavior;
- canonical switch state committed only after successful governed transition;
- Test Module Launcher and Tray Open projections.

**Config/Features source closure is locally GREEN.**

Release certification, Fresh Live acceptance and installed-system acceptance remain separate later gates.

## Boss 1.0.29 CAST29 local closure (histórico)

Fresh Live acceptance of published Boss 1.0.28 proved that package transport, the 91-package material world, MaterialBinding schema 2 and native RuntimeLease hardlink materialization were no longer the blocking frontier.

The next integration frontier exposed four independent defects:

- governed Tray provider birth was attempted from the desktop Tray Manager instead of the persistent Boss runtime authority;
- Boss Config Surface projection rejected C++ `QVariantList` values through JavaScript `Array.isArray()` checks;
- Launcher dynamic module content could be clipped below the fixed module-list viewport;
- Launcher `surfaceActionCommand()` contained a JavaScript automatic-semicolon-insertion return defect.

CAST29 closes those source defects generically:

- Tray Manager remains the desktop presentation/session coordinator and requests provider lifecycle from persistent Boss;
- persistent Boss owns governed Construction, RuntimeLease and provider process lifecycle;
- start, stop, reconcile and runtime-deactivate IPC boundaries validate installed module identity before touching runtime state;
- `runtime.env` carries the certified desktop UID/GID, runtime identity and session Tray socket into persistent Boss;
- Boss UI accepts QML array-like `QVariantList` / nested SurfaceContent values;
- Launcher height follows real `moduleList.contentHeight`;
- Launcher Surface action command construction returns the command value correctly;
- post-install enable receives a bounded 600-second execution window for legitimate first RuntimeLease composition.

Local CAST29 gates:

```text
git diff --check                                  GREEN
bash -n scripts/install.sh                        GREEN
Boss library suite                               120 / 120 GREEN
Boss main suite                                  650 / 650 GREEN
persistent runtime IPC confinement tests           3 / 3 GREEN
domesticacion executed tests                      13 GREEN
domestic-root certification test                   1 intentionally ignored
cargo check --locked                              GREEN
```

These remain source/local gates. Fresh Live and installed-system acceptance remain separate.
## Boss 1.0.30 — CAST30 source closure (histórico)

The CAST30 source gates close:

- B3A: persistent Boss module-Governor-action IPC and governed lifecycle results.
- B3B: canonical closed/opening/open state, atomic single-flight Open, generation/execution/PID ownership.
- B4: module-wide Launcher/Tray visibility and conservative 1.0.29 settings migration.
- B5: separate desktop-UID Notification Presenter, user-session authority, SO_PEERCRED,
  Present/Presented/Activate ownership handshake and authenticated return routing.

The recorded Gate 75 passed 665 Boss main tests, 120 library tests,
15 ownership tests, 3 Presenter tests and 13 domesticacion tests
(1 intentionally ignored). This does not imply installed-system acceptance.

## Boss 1.0.31 — CAST30 post-Fresh-Live repairs (registro histórico)

- Synchronize persistent runtime shutdown with RuntimeRegistry deregistration before disabling a module; preserve fail-closed behavior if cleanup fails.
- Retain diagnostic errors from the Boss UI operation transport rather than replacing them with a generic failure.
- Repair Tray Host selected-module QML scope and absolute icon URLs.
- Permit Installed -> Uninstall while a module runtime is running; Boss still owns lifecycle, dependency checks and the shutdown transaction.
- Consume Test Module v1.2.2 (commit `ee91d94b1026e7890379c656dbdee2896d00e32b`), including asynchronous `notification_closed` IPC handling.
- CUSTOM V2 source truth: `c0afe5baa6a58f50166ed12f17a1637a667b4d88`, with module-material manifest version 1.2.2 and Construction fetch/checkout bound to the same module commit.
- Local evidence: 805 Rust tests passed (one intentionally ignored), 12 Test Module Python tests passed, Boss UI/Tray Host stage builds passed with controlled Esbirro Qt 6.8.2. These results precede the v1.0.31 release build and are not Fresh Live acceptance.
- The published v1.0.30 release remains unchanged; full v1.0.31 CI and 11-asset verification precede trigger publication, then Fresh Live and installed-system acceptance.

## Boss 1.0.32 — CAST30 Active/Inactive and Open cross-surface synchronization (registro histórico)

- Send committed module state changes from short-lived CLI callers to the persistent Boss event owner; the latter publishes `module.lifecycle` for UI/Launcher/Tray subscribers. The module state in `ui.disabled_modules` remains canonical and distinct from Settings presentation toggles.
- Handle exited Tray process-group leaders conservatively: accept an orphaned group as quiescent only when kernel `/proc` inspection certifies that every visible member is terminated; unknown identities or live members fail closed.
- Gates131/132: 810 Rust tests passed, 1 intentionally ignored; 12 Active relay/tray and 25 Boss Settings source contracts passed. The 32-case visibility matrix is a **source specification**, not Live acceptance.
- Global `tray.enabled`, `launcher.enabled`, per-module surface visibility, locale, normal notifications and telemetry must not be altered by module Active/Inactive changes; hidden surfaces must recover canonical state when restored.
- Registry remains Test Module 1.2.2 (`ee91d94b1026e7890379c656dbdee2896d00e32b`); CUSTOM V2 remains `c0afe5baa6a58f50166ed12f17a1637a667b4d88`; pinned CUSTOM Classic/Esbirro remains `20488f6818d5e227f043425a682115f614f4bf79`.
- Gate137 Open projection: Boss UI retains its administrative Open control, enabled only for declared `governor.open` when the module is active and runtime is closed. Launcher/Tray Open remains optional SurfaceContent; object-owned Open actions remain module-declared. `RuntimeRegistry` is the single-flight authority, keyed by module, for `closed`, `opening`, `open`.
- Gate137: 815 Rust tests passed, 0 failed, 1 intentionally ignored; new tests cover single-module concurrent Open requests from three synthetic callers, independence between different modules, closed/opening/open presentation, and hidden Launcher/Tray without changing canonical state. These are **source tests, not actual GUI clicks or Fresh Live**.
- v1.0.32 release, genuine Settings/visibility matrix and actual GUI end-to-end Active/Inactive + Open tests remain pending. No release may be accepted before the live triplet is verified.

## Política de release actual — 1.0.42

- **Versión estable precedente:** 1.0.41. Preservar su release y punto de restauración.
- **Versión candidata:** 1.0.42. No afirmar que está publicada o funcional antes de los resultados reales.
- El código y el README se mantienen en `neebles-boss`; `Downloads` se utiliza únicamente para ejecuciones e informes.
- **Esbirro / CUSTOM:** seguir `neebles-custom/portable/esbirro/README.md` y el workflow `release-1.0.42.yml`, que utiliza el laboratorio controlado Qt 6.8.2 y los materiales Rust/domésticos certificados; no sustituirlo por compilación Qt del host.
- Antes de activar el trigger: versionado consistente (`Cargo`, CMake, C++, QML, metadata), ambas traducciones, release notes `es_CL`/`en_US`, pruebas estáticas y estado Git controlado.
- Ejecutar el workflow de **compilación y certificación del candidato**. Comprobar su resultado y la integridad de los **11 artefactos** previstos. Una ejecución verde no implica publicación automática.
- Publicar en un paso separado exactamente los artefactos certificados; no recompilar el candidato para publicarlo.
- Pruebas posteriores sobre NEEBLES OS instalado: (1) Critical Update desde 1.0.41 y verificación del escritorio, Launcher, Spacer, Tray y modal; (2) habilitar inicio automático y minimizado, reiniciar sesión/equipo y comprobar apertura minimizada, persistencia y apertura manual visible.
- Si las pruebas fallan, la 1.0.41 sigue siendo la base de recuperación. Registrar el defecto concreto; no atribuir resultados a pruebas no ejecutadas.

### Alcance explícito de 1.0.42

1. Handoff de Critical Update sin `systemctl --user restart plasma-plasmashell.service`.
2. Configuración de autostart y arranque minimizado de **Boss UI**. El inicio autónomo de otros módulos queda para cada módulo, no para esta release.


---

# Evolución de versiones 1.0.36 a 1.0.42

Las siguientes notas proceden de los archivos de release versionados en este repositorio.
Se conservan como historial técnico; una nota de candidato no demuestra una prueba en máquina real.
La 1.0.41 es la referencia estable previa y la 1.0.42 es código fuente pendiente de compilación y prueba.

## Versión 1.0.36 — notas de la versión

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

## Versión 1.0.37 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.37 — Post-update handoff and module release-notes correction

## Scope
- Install and validate the authenticated post-update handoff helper from packaged client-data.
- Maintain the existing verified upgrade receipt and handoff barrier, then restart only `plasma-plasmashell.service` and launch the new Boss UI.
- Display the one-time Boss release-notes modal after a genuine installed-version transition.
- Display a separate module update notes dialog only after a verified module upgrade with authenticated non-empty notes. Initial installs do not show notes.
- Preserve pinned CUSTOM Esbirro and runtime material.

## Acceptance pending
- Real Boss 1.0.36 → 1.0.37 Critical Update: installed version, receipt, handoff log, scoped Plasma restart and one-time notes modal.
- Real module 1.2.3 → 1.2.4 update: notes must be included in the module payload; verify the popup, with no Boss or Plasma restart.
- Do not claim runtime acceptance from build success alone.

## Publication
GitHub Actions produces only the candidate artifact. Publish a release only after candidate verification and approval.

## Versión 1.0.38 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.38 — Receipt permissions and desktop handoff

## Changes
- The privileged installer validates ownership and safe filesystem nodes before acting on the desktop user's configuration tree.
- The dedicated `~/.config/neebles` directory is automatically secured to private mode 0700, including from a legitimate user-owned mode 0775. No manual chmod or alteration of the parent `~/.config` is required.
- Failures in receipt generation include a specific diagnostic and explicitly report that handoff was skipped.
- A genuine Boss upgrade creates the one-time receipt, launches the installed handoff helper, restarts only `plasma-plasmashell.service`, reopens Boss, and offers localized release notes. Those runtime effects require real-device acceptance.
- Preserves module release-notes modal functionality from 1.0.37; an actual verified module update is required to accept it.

## Acceptance (not implied by candidate build)
- Real 1.0.37 → 1.0.38 upgrade on Ryzen and VivoProBook: verified receipt, handoff log, Plasma service restart, new Boss UI and localized one-time modal.
- Real verified module update with authenticated release notes: separate modal without restarting Boss or Plasma.

GitHub Actions generates an audited candidate only. Publishing is a separate explicitly authorized step.

## Versión 1.0.39 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.39 — Isolated desktop upgrade handoff

- Launch post-upgrade handoff through a separate transient `systemd --user` service, outside the retiring Boss UI cgroup.
- Fail closed if the helper is not executing in its expected dedicated service.
- Preserve authenticated upgrade receipt, verified old-UI D-Bus identity and pidfd termination, Plasma-only restart and automatic Boss relaunch.
- Add durable transition logging for diagnosing real-device behavior.
- Preserve the existing Boss and module release-notes dialog functionality.

The CI candidate alone cannot certify the on-device behavior. Validate a real 1.0.38 → 1.0.39 upgrade before marking desktop handoff accepted.

## Versión 1.0.40 — sin notas históricas disponibles

No existe el archivo de notas de esta versión en el repositorio local. Este README no reconstruye ni atribuye cambios sin una fuente verificable.

## Versión 1.0.41 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.41 — Recovery baseline

- Rebuild from the exact functional source baseline of v1.0.39 (commit 4a35114).
- Restore the isolated desktop upgrade handoff and existing CAST30 packaging behavior.
- Exclude v1.0.40 B8-B42 changes.
- Update product, installer, UI, launcher, tray, handoff and release version identities to 1.0.41.
- Candidate build and installed-system acceptance are pending independent verification.

## Versión 1.0.42 — notas de la versión

# N.E.E.B.L.E.S. Boss v1.0.42

## Español (es_CL)

### 1. Critical Update sin reiniciar Plasma
Critical Update sustituye el reinicio completo de `plasma-plasmashell.service` por la renovación selectiva de Launcher, Spacer y Tray de NEEBLES. El objetivo es mantener el escritorio KDE Plasma activo y preservar las preferencias y la disposición de sus componentes.

### 2. Inicio automático y minimizado de Boss
Se incorporan dos opciones independientes en **Config**: iniciar Boss con la sesión de NEEBLES OS e iniciar su ventana minimizada. Las preferencias persisten en la configuración de Boss; abrirlo manualmente continúa mostrando la ventana normalmente. Los módulos administran su propio inicio, sin depender del arranque de Boss UI.

## English (en_US)

### 1. Critical Update without a Plasma restart
Critical Update replaces a full restart of `plasma-plasmashell.service` with a selective refresh of NEEBLES Launcher, Spacer, and Tray. The goal is to keep the KDE Plasma desktop running while retaining component preferences and layout.

### 2. Automatic and minimized Boss startup
Two independent **Config** options control whether Boss starts with the NEEBLES OS desktop session and whether its window starts minimized. Settings persist across sessions; manual launches still open the window normally. Modules manage their own startup independently of Boss UI.

## Estado de validación / Validation status
Los cambios están integrados en el código fuente; compilación, prueba funcional y publicación de 1.0.42 siguen pendientes.
Changes are integrated in source; the 1.0.42 build, functional verification, and publication are still pending.

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
    -> Modules remains administrative-only
    -> Config -> Features materializes Notify button + Notify switch
    -> Launcher exposes Open
    -> Tray exposes Open Surface
    -> Notify button `active` requirement resolves correctly
    -> Notify switch is disabled before runtime registration
    -> Open enters Lifecycle
    -> construction.step selects open-runtime
    -> modules.runtime creates independent RuntimeLease
    -> Essential + Test Module delta compose in the lease rootfs
    -> certified hardlink/material semantics survive RuntimeLease composition
    -> desktop identity/session authority
    -> persistent runtime retains RuntimeLease for real process lifetime
    -> Module IPC registration
    -> Notify switch becomes enabled only while `open` is satisfied
    -> UI opens
    -> Notify button reaches Boss-governed desktop notification
    -> Notify switch notification transition succeeds
    -> canonical switch state changes only after successful transition
    -> runtime close makes `open` requirement fail closed
    -> reopen restores `open` requirement
    -> Tray provider enters through tray-provider Construction
    -> Tray provider registers through kernel peer identity
    -> settings and switches communicate Boss <-> module
    -> state changes are reflected through canonical surfaces
    -> disable / enable works
    -> uninstall removes Feature inventory and runtime/module state
    -> reinstall rematerializes valid Feature inventory
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

## NEEBLES Boss 1.0.43 — Correcciones pendientes de prueba


- Critical Update: evita la destrucción/recreación de plasmoides Launcher/Spacer activos mediante evaluateScript. Las instancias y geometría de Plasma quedan preservadas. El nuevo QML se recarga al iniciar una nueva sesión de Plasma, sin reiniciar Plasma durante el update.
- Startup: al desactivar inicio automático fuerza `startup.minimized=false`; imposibilita activar minimizado sin autostart y bloquea el switch desactivado en UI.
- UI: una invocación manual `neebles start` intenta recuperar y mostrar la ventana de una instancia previamente oculta mediante D-Bus.

Importante: esta versión debe compilarse con Esbirro Qt 6.8.2 y probarse en NEEBLES OS. No afirmar correcciones verificadas en runtime hasta probarlas.

## Boss 1.0.44 — Inicio minimizado (candidato)

- Inicio minimizado: solicitar showMinimized() después de registrar /BossWindow en D-Bus, para crear una ventana minimizada real y recuperable.
- Mantiene correcciones 1.0.43 de Critical Update y de los interruptores Startup.
- Servicio de sesión conserva After=graphical-session.target: no se ha demostrado que sea necesaria una espera adicional específica de KWin o Plasma.

Validación pendiente: compilación certificada Esbirro Qt 6.8.2, Rust 1.98.1, pruebas instaladas en X11 y eventualmente Wayland. No afirmar resolución hasta aceptación real.

## Boss 1.0.45 — Critical Update cgroup handoff

La identidad de la unidad transitoria aislada se deriva de TARGET; deja de depender de la versión anterior fija. Compilación Esbirro y aceptación instalada pendientes.

## Boss 1.0.46 — Launcher/Spacer surfaces and Tray

Las superficies gráficas se regeneran con Loader tras detectar cambios estables en los archivos instalados, sin eliminar applets. Se verifica mediante la próxima actualización 1.0.46 → 1.0.47.


## Boss 1.0.49 — Release Notes compartidas y actualización segura de módulos

- Una única presentación localizada de Release Notes para Boss y módulos, siempre con advertencia previa y botón de reinicio. El reinicio se solicita al cerrar el diálogo; no se añade un segundo modal.
- Mantiene intactos la confirmación del recibo de Boss y el requisito de transición verificada de versión de módulo.
- Corrige identidad de actualización de módulo (`transactionOperation.module`), y la tarjeta muestra Critical Update con la versión remota sin reemplazar el updater transaccional.
- `critical-update/manifest.json` de módulos se valida ANTES de desactivar la versión anterior: `[]`/ausencia legada permitidos, instrucciones sin ejecutor rechazadas. Nightmare de Boss permanece intacto.
- B5/R3 local Rust: 714/714 pruebas del binario; 171/171 pruebas activas de biblioteca (una ignorada); build Rust release GREEN con toolchain 1.98.1.
- **No certificar por estos tests:** Qt 6.8.2/Esbirro, DESTDIR, 11 assets, release publicada y aceptación instalada siguen pendientes. Test Module 1.2.3 permanece para el salto real a 1.2.4.


## Boss 1.0.50 — actualización de versión y handoff

- Critical Update solo desde una versión instalada inferior a la versión candidata, sin origen fijo.
- Igual versión no genera transición de actualización; los downgrades se rechazan.
- Instalador, recibo y handoff usan el destino 1.0.50 y conservan sus verificaciones.
- Nightmare, motor Rust de Critical Update, módulos y Esbirro/Qt 6.8.2 sin cambios.
- Pendiente: prueba real de actualización en Ryzen.


## Boss 1.0.51 — Launcher Open y presentación de transacciones

- El Launcher vincula su botón Open con la disponibilidad gobernada desde una lectura nueva del Surface Model (`closed`, `opening`, `open`), sin alterar contratos ni Governor.
- El módulo puede abrirse desde Boss UI, Launcher o Tray; la apertura única permanece defendida por RuntimeRegistry.
- La acción Open de Boss UI no muestra Module Transaction; Install, Update, Uninstall y Critical Update mantienen sus transacciones.
- Requiere certificación del pipeline Esbirro/Qt y validación visual real antes de declararse estable.
