# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

Current source line: **1.0.15**

Published Point 2 closure baseline: **`e3fd46a` — `refactor(boss): close point 2 authority and settings cleanup`**

Current macro closure: **Point 3 — Nightmare + Critical Update GREEN / CLOSED**.

N.E.E.B.L.E.S. Boss is the governance and orchestration layer of the N.E.E.B.L.E.S. ecosystem.

Boss owns contracts, state transitions, privilege boundaries, authority transport, module transactions, runtime IPC and desktop integration.

Technology-specific implementation remains outside the Boss core whenever a generic contract can describe the requirement.

> **Boss governs. Consumers declare. Execution receives explicit authority.**

---

## Current status

The current Boss source line is **1.0.15**.

Completed macro fronts:

```text
Point 1  Contracts + Module IPC                GREEN / CLOSED
Point 2  Settings + authority/persistence      GREEN / CLOSED
Point 3  Nightmare + Critical Update           GREEN / CLOSED
Point 4  Notifications                         NEXT
```

The Lifecycle and generic Surface front remains **GREEN / CLOSED** and is not reopened unless current source proves a real contradiction.

The completed closure includes:

- generic Lifecycle contract and execution;
- Governor → Lifecycle integration;
- productive AVAILABLE capability consumption;
- inter-module `require`;
- transaction, rollback and compensation semantics;
- normalized result, failure, event and telemetry flow;
- object transitions and canonical object state;
- generic SurfaceContent projection;
- productive Boss UI controls;
- productive Boss Launcher controls;
- productive Boss Tray controls;
- removal of the obsolete `lifecycle_open_close` specialization;
- final global certification.

The final certification closed with:

```text
PASS - 1.7 GLOBAL CERTIFICATION GREEN
PASS - 1.X SURFACE/LIFECYCLE FRONT CLOSED
```

`launcher_action` legacy compatibility has already been reconciled during Point 2. Launcher exposure is now resolved from the dynamic `commands` contract with `launcher=true`; the removed top-level `manifest.commands` model must not be revived.

Test Module remains **frozen** until **BOSS CONTRACT CLOSED**.

---

## Architectural law

Resource existence does not imply permission to use that resource.

The current authority flow is:

```text
physical supply
    ↓
platform-controlled path authentication
    ↓
authority descriptors
    ↓
process-scoped registration
    ↓
explicit grants
    ↓
constrained execution
```

N.E.E.B.L.E.S. OS owns platform authority definitions.

N.E.E.B.L.E.S. BUILD materializes those platform resources into the operating-system image.

N.E.E.B.L.E.S. CUSTOM owns the certified domestic runtime corpus.

Boss consumes supplied authority. It does not derive authority from arbitrary host state.

The Stage 8 host-independence law is permanent.

---

## Runtime authority

The installed Boss client keeps its permanent domestic runtime below:

```text
/opt/neebles/client/runtime/boss/
```

The runtime contains the resolver contract and manifest required by Boss clients.

Normal module runtime ownership is now canonicalized in **Module IPC RuntimeRegistry**.

Physical runtime identity is authenticated with:

```text
SO_PEERCRED PID
+
/proc/<pid>/stat start_time_ticks
```

PID alone is insufficient because Linux may reuse it.

Regular module runtime liveness is not owned by a generic pid-file marker.

Tray provider runtime remains a separate authority owned by **Tray Manager**. Its tray-specific marker machinery is legitimate provider infrastructure and must not be conflated with normal Module IPC runtime ownership.

---

## Platform AuthoritySupply

The current platform authority family includes definitions such as:

```text
platform.filesystem_boundary
system.dns_resolver_config
boss.modules.install_staging
boss.modules.update_staging
```

AuthoritySupply enters the Boss process before normal dispatch.

Boss validates, registers and grants supplied authority without manufacturing platform credentials.

---

## Domestic execution

The domestic execution layer resolves executable and library requirements inside explicit worlds and search authorities.

The ELF machinery models interpreter observation, dependency observation, search authority, recursive closure and execution grants.

Host executable availability is not treated as implicit authority.

---

## Esbirro

Esbirro is the declarative domestic certification system built from Boss authority engines, spell execution logic, world resolution and tester matrices.

Its canonical source lives in Boss.

The declarative contracts live below:

```text
src/contracts/
```

The certification matrices live below:

```text
testings/domesticacion/
```

N.E.E.B.L.E.S. CUSTOM supplies the controlled material world used by that certification system.

Lifecycle does **not** talk to Esbirro.

Lifecycle does **not** domesticate runtimes.

Lifecycle does **not** validate module technology.

The future `world Modules` belongs to CUSTOM v2 / Esbirro.

Boss receives material already built, domesticated and certified.

---

## Module transactions

Module installation and update use governed staging and publication semantics.

An incomplete candidate does not become active module state.

Install and update staging are provided as explicit writable authorities.

Update preserves recovery material until the governed operation succeeds.

Failure restores the previous valid state when required by the transaction.

---

# Lifecycle

Boss now includes the complete generic Lifecycle architecture.

Lifecycle is the communication and execution contract between a module and Boss. It describes **what** must happen without teaching Boss the technology, framework or implementation details of the module.

> **Boss interprets and governs. Modules own intent and implementation.**

Lifecycle is not Esbirro, module construction or domestic certification.

---

## Lifecycle contract

A module may provide a Lifecycle contract containing:

- arbitrary `hardcoded` string mappings;
- dynamic `require` declarations;
- arbitrary objects;
- arbitrary top-level transitions;
- arbitrary object transitions;
- arbitrary operation identifiers;
- dynamic operation maps for artillery, objective, munition, tactics and intelligence.

Boss does not prescribe transition vocabulary.

Names such as `install`, `update`, `uninstall`, `enable`, `disable`, `open`, `close` or any future action are not universal Lifecycle laws.

They are only consumer vocabulary.

Operation values remain declarative until execution preparation, where they are resolved against the current Battlefield.

---

## Execution architecture

The productive execution path is:

```text
LifecycleContract
    ↓
Battlefield + resolver
    ↓
Battleplan IR
    ↓
dependency graph / orchestration
    ↓
PreparedOperation
    ↓
REQUESTED capability
    ↓
AVAILABLE capability catalog
    ↓
operation-scoped REGISTERED arsenal
    ↓
FireControl
    ↓
CapabilityRegistry
    ↓
Rust handler
    ↓
NormalizedResult
    ↓
intelligence
    ↓
state + communication
    ↓
failure context
    ↓
event
    ↓
optional telemetry
```

The capability states are intentionally distinct:

```text
AVAILABLE != REQUESTED != REGISTERED != USED
```

A capability may exist in Boss without being requested by a module.

A requested capability is registered only for the operation that needs it.

Registration does not imply successful execution.

---

## AVAILABLE arsenal

Boss exposes a productive AVAILABLE capability catalog to Lifecycle.

Governor execution consumes that catalog through the productive runtime path instead of depending on an empty prebuilt arsenal.

The catalog is the stable expansion point for Rust capability adapters.

Adding new Rust capabilities must not require redesigning Lifecycle, Governor, DAG execution, FireControl or module contracts.

Rust APIs remain compiled and typed.

The adapter layer translates generic Lifecycle operations into concrete Rust implementations while Lifecycle remains technology-agnostic.

---

## FireControl and CapabilityRegistry

FireControl resolves the requested `(artillery, objective)` pair to a registered implementation.

CapabilityRegistry owns the executable Rust handler.

Neither layer infers module intent.

Neither layer fabricates routes, handlers or grants.

The same implementation may receive arbitrary objectives.

Missing routes, missing implementations and capability failures become normalized failures instead of bypassing Lifecycle semantics.

---

## Governor binding

Governor actions bind to module-owned transitions through the generic `hardcoded` map:

```text
governor.<action> -> <module-owned transition id>
```

The action name is arbitrary.

The transition name is arbitrary.

The Governor action and transition do not need to share the same name.

A missing binding is valid absence, not an error.

Future Governor actions can therefore be introduced without modifying the Lifecycle binding engine.

---

## Governor integration

The Module Governor is a consumer of Lifecycle for its governed module operations.

Current Governor vocabulary includes:

```text
install
update
uninstall
enable
disable
```

Those names belong to the Governor consumer. They are not hardcoded into the generic Lifecycle engine.

Boss resolves the module-owned transition bound to the requested Governor action and executes that transition through the same generic Lifecycle pipeline used by other consumers.

The Test Module does not define this architecture.

It must adapt to Boss after **BOSS CONTRACT CLOSED**.

---

## Require inter-module

The `require` path is closed and includes:

- recursive DFS;
- cycle detection;
- shared dependencies without losing parents or paths;
- baseline preservation;
- acquisition only for absent requirements;
- dependency-first publication;
- transaction journal;
- reverse compensation;
- compensation continuation even when one rollback action fails;
- `RequireHumanContext` propagation into state, communication, failure and event data.

Dependency state and execution remain governed by Boss.

---

## Transactions, failure and rollback

Lifecycle execution is integrated with the existing transactional module machinery.

Governor paths preserve transaction semantics across install, update, uninstall, enable and disable operations.

Require acquisition records a transaction journal and compensates acquired requirements in reverse order on rollback.

Module staging does not publish incomplete candidates.

Lifecycle failure, infrastructure failure and cancellation remain distinct.

Rollback failure does not stop the remaining compensations from being attempted.

---

## State, intelligence and communication

Lifecycle maintains explicit runtime state for execution, transition, operation and object scope.

Successful operation payloads may be selected into persistent intelligence for later operations.

Ephemeral result data is not confused with persistent intelligence.

Communication snapshots expose dynamic progress, result and failure namespaces without imposing a fixed schema on future fields.

Require context and object identity remain explicit throughout execution.

---

## Objects and canonical state

Objects are dynamic module-owned identities.

Boss does not infer object state from transition names.

State semantics are explicit.

A stateful object may declare:

```text
initial_active
transition_active
transitions
```

`initial_active` is optional because not every object is boolean-stateful.

`transition_active` may only reference declared transitions and requires an initial state.

Transition names remain arbitrary.

The durable Boss authority for functional object state is the canonical module-state configuration.

`ObjectStateStore` is the runtime projection of that authority.

Reconciliation:

- preserves known stateful object values;
- initializes new stateful objects from `initial_active`;
- removes state for objects that no longer exist;
- removes module object state on uninstall.

One functional object has one canonical state.

UI, Launcher and Tray do not own separate functional state.

---

# Surface projection

Lifecycle state is projected to Boss surfaces through generic SurfaceContent.

The architecture is:

```text
module
  └─ declares objects / capabilities / surfaces
       ├─ content Boss UI can represent
       ├─ content Boss Launcher can represent
       └─ content Boss Tray can represent
```

A module may publish zero, one or many items to any surface.

Surface names are generic.

Presentation visibility is separate from functional state.

UI, Launcher and Tray are consumers of the same canonical state and the same governed action path.

```text
UI ────────┐
Launcher ──┼──► Boss control path ─► Governor / Lifecycle ─► canonical state
Tray ──────┘                                            │
                                                       ▼
                                             Surface Projection
                                               ┌──────┼──────┐
                                               ▼      ▼      ▼
                                               UI   Launcher Tray
```

Cross-surface law:

```text
UI change        -> Launcher + Tray reflect
Launcher change  -> UI + Tray reflect
Tray change      -> UI + Launcher reflect
Lifecycle change -> all relevant surfaces reflect
```

There is no local boolean ownership in any presentation surface.

---

## Boss UI

Boss UI consumes generic `surface_content`.

Functional controls are rendered from module declarations.

Stateful controls read canonical `active`.

Actions flow through the Boss/Governor/Lifecycle path.

The UI does not infer module technology or transition semantics.

Existing N.E.E.B.L.E.S. visual controls, including the switch design, remain presentation components only.

---

## Boss Launcher

Boss Launcher is permanent Boss infrastructure.

Its existence is not controlled by module declarations.

Modules may publish arbitrary Launcher content through `surface_content`.

The productive Launcher:

- filters `surface == "launcher"`;
- renders generic buttons and switches;
- reads canonical `active`;
- routes module actions through the governed module-action path;
- transports explicit object and transition targets;
- preserves root Launcher enable/disable independently from module content;
- keeps the root Boss-open action separate from module action semantics.

The obsolete productive dependency on `launcher_action` has been removed from the Launcher.

---

## Boss Tray

Boss Tray is permanent Boss infrastructure.

Its root StatusNotifierItem / Plasma integration remains separate from module functional state.

Modules may publish arbitrary Tray content through `surface_content`.

The productive Tray:

- filters `surface == "tray"`;
- renders generic buttons and switches;
- reads canonical `active`;
- routes actions through the governed module-action path;
- transports explicit object and transition targets;
- does not use provider `Open` as generic module semantics;
- preserves root Boss Tray infrastructure and root Boss-open behavior.

`TrayMessage::Open` / `TrayMessage::Close` remain legitimate provider/SNI infrastructure where required. They are not Lifecycle semantics.

---

## Legacy reconciliation

The obsolete specialized Lifecycle `open/close` layer was removed.

Deleted:

```text
src/lifecycle_open_close.rs
```

The generic Lifecycle engine no longer depends on a privileged `open` / `close` transition pair.

The productive Launcher and Tray no longer depend on the old module-level `launcher_action` model.

Point 2 completed the remaining compatibility reconciliation. Launcher actions are resolved from Schema 4 dynamic contracts, and the old top-level `manifest.commands` execution model and compatibility fallback were removed.

Do not revive:

- Construct;
- Stage0 as an application architecture;
- `dependencies.rs` as the old dependency architecture;
- Local Installer;
- apt/dpkg application semantics;
- private runtime architectures;
- fixed `open/close` Lifecycle semantics.

---

## Certification status

The closed Lifecycle front has certified:

- generic contract validation;
- arbitrary transition and operation vocabulary;
- runtime resolution and IR preparation;
- dependency orchestration;
- AVAILABLE capability selection;
- FireControl and CapabilityRegistry execution;
- Governor binding and execution;
- normalized results;
- intelligence propagation;
- runtime state and communication;
- structured failure handling;
- require rollback;
- module transaction rollback;
- object transitions;
- canonical object state;
- cancellation, retry and deadlines;
- event generation;
- telemetry packaging;
- install/update/uninstall/enable/disable Governor integration;
- generic SurfaceContent;
- Boss UI generic controls;
- Boss Launcher generic controls;
- Boss Tray generic controls;
- legacy `lifecycle_open_close` removal;
- global regressions and architecture assertions.

Final closure:

```text
PASS - 1.7 GLOBAL CERTIFICATION GREEN
PASS - 1.X SURFACE/LIFECYCLE FRONT CLOSED
```

The Lifecycle/surface closure remains recorded as:

```text
aa5646d Close Lifecycle and generic Boss surfaces
```

The later functional Point 2 authority/settings closure is:

```text
e3fd46a refactor(boss): close point 2 authority and settings cleanup
```

---

# Point 1 — Contracts + Module IPC

Point 1 is **GREEN / CLOSED**.

Canonical ownership:

```text
normal module runtime
    -> Module IPC RuntimeRegistry

tray provider runtime
    -> Tray Manager

semantic module behavior
    -> Lifecycle

physical process identity
    -> authenticated kernel/process evidence
```

The module IPC boundary now includes authenticated session registration, declared endpoint validation, invoke/response transport, ping/pong, cooperative shutdown and unregister semantics.

Runtime registration is bound to the actual Unix peer through `SO_PEERCRED`, then verified against the installed module entrypoint and guarded with process `start_time_ticks`.

The old regular-runtime pid-marker authority was removed.

## Schema 4

The canonical module schema is **4**.

Top-level legacy:

```text
manifest.commands
```

is rejected.

This must not be confused with the valid dynamic contract type:

```text
"commands"
```

Dynamic contracts remain generic and extensible.

Launcher exposure is declared by an endpoint in the dynamic `commands` contract using:

```text
launcher = true
```

No global `deny_unknown_fields` policy is used because future contract extensibility remains intentional.

---

# Point 2 — Settings, persistence and authority

Point 2 is **GREEN / CLOSED**.

The settings work expanded into a broader authority cleanup and removed multiple obsolete or duplicated truths.

Current settings law:

```text
defaults.json
    -> installation / first-birth seed only

local_settings_boss.json
    -> live persisted Boss-owned settings authority
```

An existing Boss local settings file is not continuously reseeded from defaults.

Boss owns only Boss administrative/configuration state.

Module-private settings remain module-owned and are accessed through the generic settings/IPC path.

Functional object state remains canonical Lifecycle/Boss state.

Presentation visibility belongs to **SurfaceProjection** at projection-item identity, not to old module-level hidden Tray/Launcher lists.

`modules.rs` remains the administrative transaction coordinator across real domain owners. It is not a second Lifecycle, settings engine, runtime registry or surface-state authority.

## Point 2 authority cleanup

The functional Point 2 closure is:

```text
e3fd46a refactor(boss): close point 2 authority and settings cleanup
```

The cleanup produced:

```text
8 files changed
357 insertions
1321 deletions
```

The significance is architectural, not numeric. The removed material included:

- obsolete top-level `manifest.commands` execution and compatibility paths;
- regular runtime pid-marker authority;
- deprecated deactivate registry / `deactivate.json`;
- module-level Launcher/Tray visibility truths;
- redundant Governor, dependency-plan, Surface and launcher wrappers;
- an unsafe public `SurfaceProjection::get_mut` escape hatch;
- dead fallout from the previous authority model.

Point 2 is closed. There is no planned Point 2.6.

---

# Point 3 — Nightmare + Critical Update

Point 3 is **GREEN / CLOSED**.

The closure preserves a strict ownership boundary:

```text
release/update owner
    -> Critical Update coordination
        -> opaque release-owned instructions
            -> generic CriticalUpdateConsumer router
                -> matching consumer
                    -> generic transformation/execution engine
```

Critical Update owns update/release coordination. Nightmare remains a transversal declarative transformation engine and does not become update-specific logic.

## Critical Update law

Boss self-update is a **Critical Update**. There is no parallel "normal Boss update" architecture.

Each release owner owns its own Critical Update manifest:

```text
<owner-repository>/critical-update/manifest.json
```

For Boss releases, the release pipeline materializes that owner file as:

```text
critical-update-manifest.json
```

A zero-byte owner manifest is valid and means:

```text
no Critical Update instructions for this release
```

A non-empty manifest is an ordered JSON array of opaque strings.

Valid conceptual shape:

```json
[
  "instruction.one",
  "instruction.two"
]
```

The contract does not expose typed action objects, `nightmare=true`, `required`, `order`, a consumer-specific DSL or any other second semantic schema.

The consumer interprets the string.

## Opaque instruction routing

Critical Update owns a generic consumer boundary.

The router:

- receives opaque strings;
- preserves instruction order;
- requires exactly one accepting consumer;
- rejects missing consumers;
- rejects ambiguous consumers;
- propagates consumer failure;
- executes nothing for an empty manifest.

The current productive consumer is the Nightmare adapter when the referenced formula is executable.

Critical Update does not teach Nightmare about releases, versions or Boss-specific update semantics.

Nightmare does not teach Critical Update transformation semantics.

## Nightmare execution

Nightmare remains generic.

The existing formula model now supports optional generic execution metadata while preserving passive formulas as valid declarations.

The productive execution surface resolves a formula into a generic execution plan and uses the existing Nightmare primitives for:

- source;
- target;
- destination;
- preserve rules;
- logical `set`;
- logical `add`;
- logical `remove`;
- logical `rename`;
- atomic publication;
- symlink refusal and existing filesystem safety rules.

This capability is reusable by future consumers. It is not a Critical Update one-off.

## Boss update boundary

Boss update detection consumes the existing stable release bootstrap.

Critical Update execution stages and verifies release assets before installation.

The update boundary does not derive permission from physical host state.

The obsolete hardcoded platform AuthoritySupply path was removed.

Critical Update now consumes the AuthoritySupply reference already supplied to the running Boss process and transports that same explicit reference to the installer.

The closed path therefore does not contain:

- a hardcoded `/usr/lib/neebles/platform/authority/authority-supply.json` assumption;
- a host fallback for AuthoritySupply;
- direct host `curl` execution;
- direct host `git` execution;
- a `critical-update discover` command;
- the removed `registry/critical-update.json` compatibility path.

## Boss UI

Boss exposes Critical Update only when a newer Boss release is available.

The Boss-owned Critical Update control remains presentation over the governed backend path. It does not become a second update engine.

## Point 3 certification

The final Point 3 certification closed with:

```text
owner manifest exists                         GREEN
owner manifest zero byte                      GREEN
release materializes Critical Update manifest GREEN
release verifies Critical Update manifest     GREEN
Boss parses Critical Update manifest          GREEN
Boss executes opaque instruction router       GREEN
Nightmare consumer exists                     GREEN
Nightmare productive executor exists          GREEN
deprecated registry manifest absent           GREEN
forbidden discovery command absent            GREEN
hardcoded AuthoritySupply absent              GREEN
process-supplied AuthoritySupply consumed     GREEN
direct host curl absent                       GREEN
direct host git absent                        GREEN
```

Regression closure:

```text
cargo fmt -- --check                  GREEN
cargo check --locked                  GREEN
boss_update                           10 passed
critical_update                        9 passed
nightmare                             41 passed
library authority suite               37 passed
domesticacion_elfica                  13 passed / 1 explicitly ignored
git diff --check                      GREEN
```

Final verdict:

```text
FINAL :: GREEN
POINT :: 3 NIGHTMARE + CRITICAL UPDATE CLOSED
```

Test Module remained frozen throughout Point 3 and did not drive Boss architecture.

---

# Remaining roadmap

Points 1, 2 and 3 are closed.

The remaining Boss work starts at **Point 4**:

4. **Notifications**
   - close the generic notification contract and ownership model;
   - define emission, transport, state and capabilities;
   - do not turn Notifications into a parallel control channel.

5. **Auth / privileges**
   - close `requires_root`, auth-agent and privilege escalation semantics;
   - align Lifecycle, IPC, Settings and sensitive operations with explicit authority;
   - preserve the Stage 8 law: physical availability never implies permission.

6. **Registry / module catalog**
   - close metadata, version, installed state, enable/disable state and contract references;
   - align Registry with Governor, Lifecycle, Settings and IPC;
   - the catalog describes and governs identity/state; it does not execute module technology.

7. **CUSTOM v2 integration into Boss**
   - integrate certified CUSTOM v2 output without making Boss understand module technologies;
   - keep Esbirro outside Lifecycle;
   - keep `world Modules` in CUSTOM v2 / Esbirro;
   - preserve Stage 8 host independence.

8. **Final Boss integration and certification**
   - cross-certify Module IPC, Settings, Nightmare/Critical Update, Notifications, Auth, Registry and CUSTOM v2 against the already-closed Lifecycle/surface architecture;
   - run the real Boss E2E;
   - remove any remaining duplicate or diverging contracts;
   - declare **BOSS CONTRACT CLOSED** only after global GREEN.

9. **Adapt Test Module to what Boss dictates**
   - unfreeze Test Module only after **BOSS CONTRACT CLOSED**;
   - adapt manifest, contracts, Lifecycle, IPC, Settings, Tray, Notifications and Auth;
   - use CUSTOM v2 + Esbirro for module construction, domestication and certification;
   - Test Module demonstrates the final Boss contract and never drives Boss architecture.

10. **Full Test Module certification**
    - certify the complete matrix:

```text
install
  -> enable
  -> UI
  -> settings
  -> tray
  -> notifications
  -> disable
  -> enable
  -> update
  -> uninstall
  -> reinstall
```

    - verify rollback, persistence, IPC, privileges and UI behavior against the already-closed Boss contract;
    - keep the Test Module repository as the canonical module mold after PASS.

> **Order law:** Boss is completed first. Test Module adapts afterward. Test Module never defines Boss.

---

## Recovery boundary

Recovery is external to Boss.

N.E.E.B.L.E.S. BUILD owns the independent `neebles-check` recovery environment.

Boss normal startup does not depend on invoking the recovery engine.

---

## Telemetry

Telemetry is an optional generic error-reporting channel.

It is not a dependency engine, readiness gate or recovery engine.

Normal Boss operation does not require a remote telemetry endpoint.

---

## Nightmare

Nightmare is the generic declarative transformation engine for persistent state and other formula-driven transformations.

Its role is transversal: formulas describe reusable transformations without forcing Boss to gain technology-specific branches for every consumer.

Point 3 added a productive generic execution surface and connected Critical Update through a consumer adapter.

Nightmare remains independent from Critical Update. Future Boss subsystems may consume Nightmare without changing Nightmare into update-specific infrastructure.

---

## Work protocol

For the current N.E.E.B.L.E.S. workflow:

- commands/casters must not contain the shell prompt character;
- do not use shell `set`;
- one implementation step should produce one consolidated validation artifact when evidence is required;
- GREEN advances immediately;
- RED is repaired and recertified before advancing;
- never claim a path was tested without current evidence;
- current source plus the latest certification evidence are the source of truth;
- historical documents do not override current source;
- Test Module stays frozen as an architecture driver until **BOSS CONTRACT CLOSED**.

---

## Current handoff position

```text
STAGE 8                         GREEN / CLOSED
LIFECYCLE                      GREEN / CLOSED
REQUIRE                        GREEN / CLOSED
GOVERNOR -> LIFECYCLE          GREEN / CLOSED
CANONICAL OBJECT STATE         GREEN / CLOSED
BOSS UI / LAUNCHER / TRAY      GREEN / CLOSED
POINT 1 MODULE IPC             GREEN / CLOSED
POINT 2 SETTINGS / AUTHORITY   GREEN / CLOSED
POINT 3 NIGHTMARE / CU         GREEN / CLOSED
POINT 4 NOTIFICATIONS          NEXT
TEST MODULE                    FROZEN
BOSS CONTRACT CLOSED           NO
```

Current rule for continuation:

**Begin Point 4 with a current-state audit of Notifications. Do not code until ownership, emission, transport, state and capability boundaries are proven from current source.**

---

## Version policy

All current documentation in this repository describes the **1.0.15** source line.

Source updates and documentation updates do not create a release by themselves.
