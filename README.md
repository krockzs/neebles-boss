# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

**Version:** **2.0.6** (candidato en desarrollo; no publicado).

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

# Idioma global y traducciones de modulos

Boss administra la preferencia global `ui.language`. Su selector utiliza `client/languages/manifest.json` y cada modulo declara sus propios `languages/manifest.json` y JSON de claves. Boss gestiona la preferencia y la proyeccion generica; la UI/Tray/runtime del modulo resuelven los textos de su tecnologia. No inventar una segunda fuente de idioma ni una traduccion interna a Boss de cadenas privadas de un modulo.

Test Module 1.2.12 lee `ui.language` desde sus Settings; el cambio de idioma en vivo llega a su UI/runtime/Tray mediante el mecanismo de Settings existente. El hecho de que el codigo y las pruebas fuente lo soporten **no demuestra** que todos los 253 idiomas hayan pasado revision lingueistica o aceptacion grafica completa.

Para aceptar el contrato en un modulo nuevo: seleccionar varios idiomas desde Boss, alternar con runtime ya abierto/cerrado, revisar labels de Surfaces/Tray y notificaciones, observar estado autentico del Governor y confirmar fallback cuando falta un locale. Mantener los IDs tecnicos sin traducir.

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

### Uninstall — transactional teardown (TXN V5)

Uninstall is a Boss-governed, fail-closed transaction. A module declares the resources it owns; Boss authenticates their identity, coordinates their shutdown, detaches installed material, and retires module state. A module's implementation language does not create a technology-specific uninstall path.

The optional `manifest.uninstall` contract uses schema **1**. Its `main` authority is `governor`; the `hardcoded` values are resolved through the existing Battlefield resolver. Its `extras` describe additional governed resources or module-owned `systemd-user` services; `cleanup` describes module-data directories and optional local settings. `failure.unknown_process_owner`, `failure.resource_still_running`, and `failure.cleanup_error` use the `abort` policy. The declared contract is validated before resource shutdown begins.

#### Resource ownership and shutdown

- Installed dependents are checked before Lifecycle or filesystem mutation. An installed module required by another installed module cannot be uninstalled.
- Governor executes the module's declared `uninstall` Lifecycle transition. Boss then quiesces additional declared resources and deactivates the governed module runtime. A Tray provider already managed by Boss is not stopped a second time.
- A declared `systemd-user` unit must belong to the module's `neebles-<module>-*.service` namespace. Boss checks the desktop-user identity, unit ownership, file contents, `ExecStart`, manager fragment and service state instead of trusting a process name or arbitrary unit path. Stop is bounded by the declared timeout and verifies inactivity.
- Desktop service units are stopped while their original definitions remain available for compensation. Final disabling and unit-file retirement occur only after the uninstall crosses the commit boundary.
- Cleanup directories are restricted to normal relative paths under the module's `shared/data/<module>` territory. Path traversal, symlinked parents, non-directory paths, overlapping cleanup declarations and foreign service resources are rejected.

#### Transaction boundaries

```text
installed dependency check
    -> Governor uninstall transition
    -> capture original installation and service identity
    -> validate and quiesce declared extras
    -> deactivate governed runtime
    -> stage module installation outside the active modules tree
    -> stage selected module-data directories and local settings
    -> stage authenticated MaterialBinding removal
    -> remove Boss-owned module state
    -> irreversible commit boundary
    -> finalize detached filesystem material
    -> retire declared systemd-user units
    -> report lifecycle result
```

The detached installation, selected module-data directories, local settings, and active MaterialBinding participate in the same transaction. Meaningful local settings are preserved unless removal is explicitly requested; `cleanup` entries marked `user-choice` follow that decision. Permanent, verified Essential and module-delta DEB pools are reusable material and are not erased merely because a module is uninstalled.

#### Abort and service compensation

Before commit, a failed uninstall restores staged resources instead of treating a partially detached module as absent. Boss records which declared user services were originally active and compensates them **in reverse order**, restarting only those that were active before shutdown. A partially failed service stop is included in that recovery record.

Service reactivation requires the original module path, byte-identical module manifest, previously existing data and settings paths, and authenticated MaterialBinding state to be restored. Boss also revalidates the original user unit, ownership, contents, `ExecStart` and systemd manager identity. If any authority or physical resource cannot be re-established, reactivation is refused and a `CRITICAL` error is returned rather than launching an unknown or incomplete installation. Individual service recovery failures are accumulated rather than masking other compensations.

#### Post-commit cleanup

The irreversible boundary is entered before MaterialBinding finalization or destructive deletion. **Once deletion begins, rollback is forbidden.** Detached data must never be silently resurrected after a partial removal.

Filesystem cleanup continues across independent resources and collects failures instead of aborting on the first `remove_dir_all` error. The module-data staging guard is always consumed/finalized, including when deletion of the detached main module fails, so its destructor cannot restore user data after commit. If the main detached module cannot be fully deleted, selected local settings are preserved for manual recovery where possible. Service-unit retirement is still attempted independently; cleanup failures are surfaced as post-commit errors, not mislabeled as a successful rollback.

This contract distinguishes a **reversible abort** from a **committed uninstall with incomplete residual cleanup**. Boss does not substitute host tools, follow arbitrary symlinks or execute module-supplied privileged shell cleanup in order to make either result appear successful.

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

## Candidato de desarrollo — Boss 2.0.1 (F10/F11)

F10 corrige la validación global indebida de todos los mundos CUSTOM dentro de cada RootFS de un módulo. Construction selecciona explícitamente `domination.world` por RootFS y la misma selección se usa en el staging de Install y en RuntimeLease. Preinstall, Essential y los pools de `.deb` por SHA no cambian. Test Module 1.2.12 vuelve al Registry junto a Connect 1.0.0.

La compilación y las pruebas Rust se realizaron en Ryzen antes de F11. El versionado 2.0.1 sigue siendo un candidato local; no equivale a una release publicada. La versión estable publicada continúa siendo 2.0.0 hasta completar el workflow certificado de Esbirro, preservar y publicar sus 11 artefactos exactos y verificar Install/Open/Close/Uninstall de ambos módulos en el Ryzen.

---

# Estado y publicacion vigente — Boss 2.0.0

**Release estable publicada:** [`v2.0.0`](https://github.com/krockzs/neebles-boss/releases/tag/v2.0.0), compilada desde `68b92d703aaf59e003e6c5624c37254be68cd2cc` (no confundir con los commits posteriores de `registry/modules.json`).

El workflow de release verifico **11 artefactos** y publico sus checksums reales en `SHA256SUMS`: `bootstrap.json`, `boss-runtime.tar.gz`, `client-data.tar.gz`, `critical-update-manifest.json`, `install.sh`, `neebles-auth-agent`, `neebles-backend`, `neebles-installer`, `neebles-runtime-resolve` y `neebles-ui`. El codigo Rust se compilo con 1.98.1; los consumidores Qt se compilaron y certificaron con el laboratorio controlado de Esbirro Qt 6.8.2 y su etapa `DESTDIR`.

La traduccion de Boss dispone de **253 opciones** y 149 claves por JSON; `es_CL` es el idioma predeterminado del catalogo Boss. Los modulos declaran diccionarios propios y su fallback. Test Module 1.2.12 declara `en_US` como fallback, tiene 253 JSON de 32 claves, y se selecciona mediante Registry con SHA `3d265bc62a7ff51ed1c42aecf0158abfa29b3223`. La propagacion del idioma mantiene a Boss como autoridad global; los cambios de visualizacion concretos deben seguir verificandose por superficie (UI, Launcher, Tray, notificaciones), sin inferir la aceptacion por una prueba estatica.

## Ley permanente de release

1. Revisar el `main` y las fuentes reales del workflow de la **version objetivo**; no copiar rutas antiguas sin verificarlas.
2. Alinear version de Rust/Cargo, CMake/C++, Launcher/metadata, installer, release notes, cliente y bootstrap.
3. Compilar en el laboratorio certificado adecuado (Rust y Qt/Esbirro separados); verificar producto y **cada artefacto** en el workflow, con SHA-256 real. Una ejecucion verde puede guardar un candidato sin publicarlo.
4. Publicar una GitHub Release independiente **con los mismos bytes certificados**; no recompilar al momento de publicar.
5. En equipo real verificar Install/Update, modal de Release Notes con **Reboot siempre visible** y aviso de guardar trabajo, idiomas, Tray, Launcher y servicios de sesion. La release publicada no certifica por si sola este ultimo gate.
6. Para un modulo nuevo o actualizado, versionar la fuente del modulo y CUSTOM V2 por separado y cambiar solo `registry/modules.json` de Boss. Eso no obliga a reconstruir ni publicar otro Boss si no cambia una capacidad generica del gobernador.

Se conservan los antecedentes 1.0.28–1.0.51, politica 1.0.42, gates historicos y matrices antiguas en [`docs/history/BOSS_RELEASE_HISTORY.md`](docs/history/BOSS_RELEASE_HISTORY.md), no como instrucciones actuales.

---

# Aceptacion Live/instalada

La prueba de fuente, la compilacion Esbirro y la GitHub Release son puertas **diferentes**. Para una nueva ISO o un nuevo modulo, registrar evidencia verificable de:

- instalacion y update del Boss publicado con integridad SHA y version instalada;
- fuente del modulo y material CUSTOM/RootFS autenticados por su SHA y manifest;
- Open/Close single-flight desde Boss UI, Launcher y Tray, sincronizado con Governor;
- Settings e idioma de Boss proyectados en modulos sin falsificar el estado Lifecycle;
- inicio de Tray despues de iniciar sesion, con preferencias y servicios gobernados;
- Uninstall/Reinstall, preservacion/eliminacion de Settings y rollback/fallos inducidos;
- ciclo Fresh Live y despues instalacion en disco cuando se prueba la ISO.

No afirmar que se probo un recorrido fuera del equipo, sistema y version efectivamente observados.

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
