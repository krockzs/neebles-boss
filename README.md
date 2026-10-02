# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

Current source line: **1.0.16**

Historical published closure baseline before the final Point 7/8 work: **`17281b9` — `feat(boss): close points 5 and 6`**

Current macro state: **BOSS CONTRACT CLOSED**. **Point 9 Test Module adaptation GREEN / CLOSED. Point 10 full Test Module certification is IN PROGRESS.**

N.E.E.B.L.E.S. Boss is the governance and orchestration layer of the N.E.E.B.L.E.S. ecosystem.

Boss owns contracts, state transitions, privilege boundaries, authority transport, module transactions, runtime IPC and desktop integration.

Technology-specific implementation remains outside the Boss core whenever a generic contract can describe the requirement.

> **Boss governs. Consumers declare. Execution receives explicit authority.**

---

## Current status

The current Boss source line is **1.0.16**.

Current macro fronts:

```text
Point 1  Contracts + Module IPC                GREEN / CLOSED
Point 2  Settings + authority/persistence      GREEN / CLOSED
Point 3  Nightmare + Critical Update           GREEN / CLOSED
Point 4  Notifications                         GREEN / CLOSED
Point 5  Auth / privileges                     GREEN / CLOSED
Point 6  Registry / module catalog             GREEN / CLOSED
Point 7  CUSTOM v2 integration                 GREEN / CLOSED
Point 8  Final Boss integration / pre-VM gate  GREEN / CLOSED
Point 9  Test Module adaptation                GREEN / CLOSED
Point 10 Full Test Module certification        IN PROGRESS
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

The Boss contract is **CLOSED**. Test Module has been adapted as a consumer of that closed architecture and Point 9 is GREEN / CLOSED. Its final revision is aligned across Test Module HEAD, Boss Registry and CUSTOM Domestic Construction.

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
neebles.domestic_workspace
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

`world Modules` belongs to CUSTOM v2 / Esbirro.

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

# Point 4 — Notifications

Point 4 is **GREEN / CLOSED**.

Notifications is now a governed Boss subsystem with one generic presentation core, explicit desktop-session authority, module transport through `modules.sock`, exact ownership and return routing, and no dependency on the old `notify-send` transport.

The closure preserves a strict boundary:

```text
consumer intent
    -> Boss notification contract
        -> policy + validation
            -> authorized desktop-session interface
                -> org.freedesktop.Notifications through zbus
                    -> Plasma
```

Notifications does not become Lifecycle, Telemetry, EventLog, Registry, UI authority or a second module-control channel.

## Boss notification law

Current Boss product notifications use the **Default Notification** shape.

Boss-owned Default notifications use:

```text
application        N.E.E.B.L.E.S.
timeout            3000 ms
actions            none
inline reply       none
ownership          Boss
```

Normal notifications obey the persisted Boss policy:

```text
normal_notifications = false
    -> suppress normal notifications

Critical / Fatal
    -> mandatory
```

A suppressed notification does not fabricate a Plasma notification id.

When Plasma presents a Boss notification and returns a nonzero id, Boss records that id as `NotificationOwner::Boss`.

The current Boss product notification families are intentionally simple Default notifications:

| Boss notification | Current severity | Final Point 4 format |
| --- | --- | --- |
| module update available | `info` | Default |
| module update available while module is running | `warning` | Default |
| notifications enabled | `success` | Default |
| notifications disabled | `info` | Default |
| telemetry enabled | `success` | Default |
| telemetry disabled | `info` | Default |

Point 4 deliberately does **not** add Rich presentation, actions, inline reply or Custom behavior to those current Boss notifications merely because the infrastructure can support them.

## Module notification tunnel

Module notification product behavior is intentionally **not designed inside Boss**.

Boss exposes the generic notification tunnel through `modules.sock` and does not need to be recompiled for each future module notification design.

The current law is:

```text
module
    -> modules.sock
        -> Boss generic notification contract
            -> policy / ownership / presentation
                -> Plasma
```

The global Module IPC protocol remains:

```text
MODULES_PROTOCOL_VERSION = 1
```

The Notifications capability protocol is:

```text
MODULE_NOTIFICATIONS_PROTOCOL_VERSION = 4
```

A future module may therefore use the already-closed contract without teaching Boss the module name, framework, language or technology.

## Default module notification

The module Default path supports:

- module-owned application identity;
- canonical module icon resolution;
- severity;
- title and message;
- optional `replace_id`;
- optional explicit expiry override;
- exact module/session ownership;
- returned Plasma notification id.

Timeout law:

```text
no module timeout override
    -> 3000 ms

explicit module timeout override
    -> preserve the module-provided value
```

Therefore the 3000 ms Boss Default does not remove future module control over notification lifetime.

## Rich notification contract

The Rich path remains available as generic infrastructure for future consumers.

`NotificationPresentation` carries presentation data separately from `NotificationOptions`.

The closed Rich contract includes, among other fields:

- application;
- icon;
- title;
- message;
- ordered actions;
- optional inline reply;
- replacement id;
- expiry timeout;
- category;
- desktop entry;
- resident/transient semantics;
- sound name / sound file / suppress sound;
- image path;
- structured image data;
- KDE URLs;
- KDE origin/application hints.

Actions use explicit key/label pairs.

The reserved key:

```text
inline-reply
```

belongs only to the inline-reply bridge and cannot be reused as an ordinary action key.

Ordinary action keys must be non-empty and unique. Action labels and reply labels must be non-empty.

The presenter flattens actions to the FreeDesktop alternating key/label array while preserving order.

KDE inline reply is mapped through the reserved action plus the supported reply hints.

## Replacement, ACK and ownership

Notification ids are productive contract data, not presentation trivia.

Boss distinguishes ownership as:

```text
NotificationOwner::Boss
NotificationOwner::Module { module, session_id }
```

Ownership laws include:

- id `0` is rejected;
- same-owner registration is idempotent;
- ownership cannot silently transfer;
- module replacement requires the exact module/session owner;
- Boss-owned notifications cannot be replaced by a module;
- a changed replacement id moves ownership to the newly returned id;
- a suppressed replacement preserves the existing owner;
- session cleanup removes only the exact runtime incarnation.

The module forward path returns the real presented notification id through its ACK contract.

## Return channel

Boss listens to the Plasma Notifications return channel and understands the current signal family:

```text
NotificationClosed
ActionInvoked
NotificationReplied
ActivationToken
```

Module-facing return messages are routed only to the exact owning runtime session through `RuntimeRegistry::send_to_session`.

There is no broadcast fallback.

Boss-owned return events never enter `modules.sock` as module events.

`NotificationClosed` releases ownership after terminal handling according to the closed router contract.

## Desktop-session authority

Notifications does not derive desktop authority from arbitrary host environment state.

The productive presenter and return listener consume the same resolved `DesktopSessionInterface`.

That interface is produced through the closed platform capability-authority path and carries the authorized desktop uid and session bus address required for the zbus connection.

Conceptually:

```text
platform authority
    -> generic capability provider
        -> validated desktop-session values
            -> DesktopSessionInterface
                -> Notifications presenter + return listener
```

This preserves the permanent Stage 8 law:

> Physical availability is not permission.

## Transport cleanup

The previous `notify-send` path is gone from Boss Notifications.

Point 4 removed `notify-send` from:

- the domestic target catalog;
- the domestic grimorio;
- Boss runtime self-certification;
- Boss installer requirements;
- Notifications documentation.

Boss now presents through:

```text
org.freedesktop.Notifications
```

using zbus.

`qdbus6` was **not** removed because it remains a real Installer dependency unrelated to the closed Notifications presenter.

## Point 4 certification

The final Point 4 product closure certified the contract and regression surface together.

Key closure evidence:

```text
notify-send legacy transport             absent
qdbus6 Installer authority               preserved
Boss Default timeout                     3000 ms
Boss Default actions                     none
Boss Default inline reply                none
normal notification policy               preserved
Critical / Fatal mandatory policy        preserved
Notifications protocol                   v4
Global Module IPC protocol               v1
module timeout override                  preserved
Rich action/reply contract               GREEN
return signal contract                   GREEN
ownership / exact-session routing        GREEN
DesktopSessionInterface consumption      GREEN
domesticacion_elfica                     13 passed / 1 explicitly ignored
full neebles-backend suite               568 passed / 0 failed
full lib suite                           54 passed / 0 failed
git diff --check                         GREEN
```

Point 4 closes the source contract, implementation and regression certification for Notifications.

Real visual/desktop behavior remains for the dedicated full-system VM acceptance phase. Point 8 closed the pre-VM Boss contract without falsely claiming that later machine-level acceptance had already been executed.

Final verdict:

```text
FINAL :: GREEN
POINT :: 4 NOTIFICATIONS CLOSED
```

Test Module remained frozen throughout Point 4 and did not drive Boss architecture.

---

# Point 5 — Auth / privileges

Point 5 is **GREEN / CLOSED**.

The final privilege boundary keeps Boss as the authority owner and keeps Qt as a transport/presentation consumer.

The canonical flow is:

```text
authenticated Boss process
    -> process-owned AuthoritySupply
        -> scoped projection to Boss UI
            -> opaque --authority-supply transport
                -> child Boss process
                    -> Rust authentication / privilege enforcement
```

Qt does not authenticate, register or grant AuthoritySupply.

The UI receives only the exact AuthoritySupply reference already owned by the authenticated parent Boss process. Child Boss invocations transport that same opaque reference back through the canonical Rust entry boundary.

## Privilege ownership

Privilege policy remains backend-owned.

Installed contract endpoint metadata owns `requires_root`.

Module IPC runtime advertisement cannot invent or weaken that requirement.

The module router resolves the installed declaration first and then applies:

```text
requires_root
    -> Rust privilege boundary
```

Runtime advertisement remains availability only.

The Qt client no longer acts as a second privilege-policy authority for ordinary operations that the backend does not require to be elevated.

The cleaned presentation paths include:

- module enable / disable;
- settings writes;
- Surface visibility;
- module-update-notified;
- post-install enable.

Install, update and uninstall remain governed through the existing backend root boundary.

Critical Update remains a privileged governed path and continues to consume the process-supplied AuthoritySupply reference.

## AuthoritySupply transport

The Boss dispatcher projects the authenticated process AuthoritySupply reference into the desktop UI boundary without placing it in the global runtime environment.

The UI forwards that exact reference as:

```text
--authority-supply <opaque supplied path>
```

to child Boss invocations.

The child Boss process performs normal Rust-side authentication again.

Therefore:

```text
transporting a reference != authenticating authority
transporting a reference != registering authority
transporting a reference != granting authority
```

The existing auth-agent remains a privilege transport mechanism. It does not become a second authority engine.

## Point 5 certification

Point 5 closed with:

```text
Rust lib suite                         54 passed / 0 failed
full neebles-backend suite            568 passed / 0 failed
domesticacion_elfica                  13 passed / 1 explicitly ignored
cargo fmt -- --check                  GREEN
cargo check --locked --all-targets    GREEN
git diff --check                      GREEN
Boss UI canonical Qt 6.8.2 build      GREEN
```

The Qt certification used the controlled **Qt 6.8.2** world supplied through N.E.E.B.L.E.S. CUSTOM. The host Qt version is not used as a compatibility fallback.

Final law:

> **Privilege policy belongs to Boss. UI transports intent and explicit authority references; it does not manufacture privilege.**

Test Module remained frozen throughout Point 5 and did not drive Boss architecture.

---

# Point 6 — Registry / module catalog

Point 6 is **GREEN / CLOSED**.

Point 6 certified that Boss contains several registry/catalog structures with different semantic domains and that those structures do not collapse into one another merely because they share registry-like naming.

The closed ownership map is:

```text
Remote Registry
    -> external module catalog / discovery / immutable source selection

Installed module inventory
    -> committed material below modules_root() + validated local manifests

Module IPC RuntimeRegistry
    -> authenticated live runtime/session state

AVAILABLE capability catalog
    -> Rust capabilities that exist and may be requested

LifecycleArsenal
    -> operation-scoped REGISTERED execution pair

FireControl
    -> route from artillery + objective to implementation id

CapabilityRegistry
    -> implementation id to executable Rust handler

SuppliedAuthorityRegistry
    -> authenticated process-scoped supplied authorities

AuthorityGrantSet
    -> explicit GRANTED subset
```

These are intentionally different truths.

## Remote Registry vs installed truth

Remote Registry membership is not installation state.

The remote registry supplies catalog/discovery information and immutable source selection for governed install/update operations.

Installed truth is resolved from committed local material below:

```text
modules_root()
```

and validated module manifests.

`available_modules_json()` may annotate remote catalog entries with local installed state, but it obtains that installed state from the local installed inventory. The remote registry does not manufacture it.

Staged `require` candidates are explicitly transaction-local and are not installed until publication into `modules_root()` succeeds.

Moving a module out of `modules_root()` during uninstall removes it from installed truth before final destructive cleanup.

## RuntimeRegistry

Module IPC RuntimeRegistry is authenticated live-runtime state.

Before registration, Boss validates the real Unix peer and installed module identity.

Installed contract JSON remains the declarative source of truth.

A runtime may advertise only a subset of declared endpoints.

It may not advertise an undeclared contract or endpoint.

The routing law remains:

```text
installed declaration
    -> privilege policy / endpoint identity

runtime registration
    -> authenticated live availability
```

Therefore:

> **Runtime advertisement is availability, not authority.**

`RuntimeRegistry` and `PendingRegistry` are process-local runtime structures. They are not durable installation truth.

## AVAILABLE, REQUESTED and REGISTERED

The Lifecycle capability states remain deliberately separated:

```text
AVAILABLE != REQUESTED != REGISTERED != USED
```

A capability existing in `AvailableCapabilityCatalog` does not automatically enter the productive arsenal.

`LifecycleArsenal::register_requested()` performs the explicit operation-scoped conversion for the requested artillery/objective.

That conversion pairs:

```text
FireControl route
+
CapabilityRegistry handler
```

as one productive execution unit.

An AVAILABLE but unrequested capability remains unregistered.

FireControl cannot manufacture a handler.

CapabilityRegistry cannot manufacture a route.

Neither layer manufactures AuthoritySupply grants.

## REGISTERED vs GRANTED authority

Authority registration and authority grant remain separate operations:

```text
AuthoritySupply
    -> authenticated supplied descriptors
        -> SuppliedAuthorityRegistry
            -> explicit grant construction
                -> AuthorityGrantSet
```

Boss rejects a requested grant when the authority was not previously supplied and registered.

Filesystem presence by itself is therefore insufficient to create permission.

The permanent law remains:

> **Physical availability is not permission.**

## Legacy and duplicate truth audit

Point 6 found no productive deprecated registry path and no duplicate registry owning the same semantic source of truth.

Top-level legacy:

```text
manifest.commands
```

continues to be rejected by `read_manifest()`.

The valid dynamic contract type:

```text
"commands"
```

remains independent from that removed top-level schema.

Historical names such as registry-related folder resolution do not change ownership: installed module lookup still resolves local material below `modules_root()`.

## Point 6 certification

Point 6 closed through ownership census, cross-domain boundary audit, semantic closure and global certification.

Final certification:

```text
Remote Registry -> installed truth leak       ABSENT
RuntimeRegistry -> declaration leak           ABSENT
RuntimeRegistry -> privilege-policy leak      ABSENT
AVAILABLE -> REGISTERED implicit promotion    ABSENT
REGISTERED -> GRANTED collapse                ABSENT
registry persistence ownership violation      ABSENT
productive deprecated registry path           ABSENT
cargo fmt -- --check                          GREEN
cargo check --locked --all-targets            GREEN
full Rust suite                               GREEN
Rust lib suite                                54 passed / 0 failed
full neebles-backend suite                    568 passed / 0 failed
git diff --check                              GREEN
```

Final verdict:

```text
FINAL :: GREEN
POINT :: 6 REGISTRY / MODULE CATALOG CLOSED
```

No source changes were required for Point 6. Its closure certified the boundaries already present in the current Boss architecture.

Test Module remained frozen throughout Point 6 and did not drive Boss architecture.

---

# Point 7 — CUSTOM v2 integration

Point 7 is **GREEN / CLOSED**.

Its technical certification was accepted into the final Boss contract during Point 8 global integration.

Point 7 connected CUSTOM v2 to Boss without turning Boss into a module-technology interpreter.

The final ownership map is:

```text
N.E.E.B.L.E.S. CUSTOM
    -> certified domestic material
    -> module package membership
    -> module material integrity
    -> domestic construction declarations
    -> Esbirro / world Modules semantics

N.E.E.B.L.E.S. OS
    -> canonical platform authority
    -> neebles.domestic_workspace

N.E.E.B.L.E.S. BUILD
    -> image-side authority materialization
    -> /opt/neebles-build/modules shared territory
    -> opaque construction-declaration publication
    -> independent neebles-check recovery

N.E.E.B.L.E.S. Boss
    -> supplied authority authentication
    -> explicit registration and grants
    -> generic domestic construction loading
    -> generic workspace execution
```

## Generic domestic workspace authority

OS defines:

```text
authority = neebles.domestic_workspace
family    = neebles-writable-data-authority
root      = /opt/neebles-build
```

Boss does not hardcode this authority identity into the generic workspace engine.

Authority identity arrives through the normal AuthoritySupply path.

The permanent law remains:

```text
AVAILABLE != REGISTERED != GRANTED != USED
```

Filesystem presence does not manufacture authority.

## Domestic construction

Boss now supports a generic `DomesticConstructionDeclaration`.

A declaration contains generic construction steps and explicit authority references.

It does not contain compiler, framework, package-manager or module-technology semantics that Boss must understand.

The productive projection is:

```text
CUSTOM construction declaration
    -> Boss canonical declaration loader
        -> registered AuthoritySupply
            -> WorkspaceExecutionRequest
                -> existing domestic workspace execution engine
```

Canonical installed declarations live below:

```text
/usr/lib/neebles/domestic/construction/
```

The dispatcher exposes the generic action:

```text
domestic-construction-execute
```

and the CLI surface:

```text
neebles domestic execute <subject> <step>
```

The loader validates safe subject identity, canonical location, platform-controlled ownership and declaration identity before execution.

Consumer declarations cannot supply authority-descriptor paths directly.

Boss resolves those paths from the authenticated supplied authority registry.

## CUSTOM construction publication

CUSTOM owns canonical construction declarations below:

```text
runtime/construction/
```

BUILD materializes those files opaquely into the image.

BUILD does not parse construction JSON or learn module technology.

An empty construction namespace remains valid.

## Shared Modules material

CUSTOM owns the shared module source pools:

```text
runtime/modules/packages/
runtime/modules/rootfs/
```

BUILD owns the image-side shared territory:

```text
/opt/neebles-build/modules/
```

with:

```text
modules   root:root 0755
packages  root:root 0775
rootfs    root:root 0755
```

Material is shared across modules instead of being physically duplicated per module.

## Module membership and integrity

CUSTOM owns the dynamic module manifest namespace:

```text
runtime/manifests/modules/
```

Package membership and material integrity are intentionally different contracts.

Membership:

```text
<module_id>.packages.tsv
```

with exactly:

```text
package
version
arch
filename
sha256
```

Integrity:

```text
<module_id>.manifest.json
```

with required entries using the established inventory contract:

```text
path
type
mode
size
sha256
target
```

For required DEBs, filename membership and SHA256 must agree between the TSV and JSON authorities.

Rootfs integrity remains represented by the JSON integrity manifest.

## `neebles-check --module`

The independent BUILD-owned recovery engine now supports:

```text
neebles-check --module <module_id>
```

Module checking remains dynamic; Modules is not a third static checker component beside Boss and Calamares.

The checker:

- validates the dynamic module identity before building remote paths;
- fetches package membership and integrity data from the same remote revision;
- inventories only required paths for the requested module;
- ignores unrelated shared material;
- detects missing or changed required material;
- detects mode, size, SHA256, type and symlink-target mismatches;
- rejects package membership disagreement;
- rejects TSV-vs-JSON SHA disagreement;
- rejects absolute paths and parent traversal;
- rejects symlink-ancestor escape;
- prevents a module from claiming the shared `packages` or `rootfs` roots.

Privileged module inventory is fixed to:

```text
/opt/neebles-build/modules
```

The caller cannot substitute another root.

## Point 7 architectural laws

Point 7 preserves all prior boundaries:

```text
Boss governs generic execution.
CUSTOM owns construction semantics.
Esbirro stays outside Lifecycle.
Lifecycle does not domesticate runtimes.
Lifecycle does not validate module technology.
OS defines platform authority.
BUILD materializes.
Boss authenticates, registers, grants and consumes.
Stage 8 host independence remains permanent.
```

Point 7 did not introduce:

- a second Lifecycle;
- a second Registry;
- a second authority engine;
- a static Modules checker component;
- host-tool discovery as semantic authority;
- Test Module specialization in productive Boss code.

The only `neebles-test-module` reference in the Point 7 Boss construction source remains a generic safe-identity unit-test fixture.

## Point 7 technical certification

Final technical certification produced:

```text
Python source syntax                        GREEN
OS domestic workspace authority            GREEN
BUILD module shared territory              GREEN
dynamic module checker contract            GREEN
package selector contract                  GREEN
integrity manifest contract                GREEN
same-branch TSV + JSON lookup              GREEN
TSV package membership                     GREEN
TSV / JSON SHA cross-check                 GREEN
shared-pool subset semantics               GREEN
unrelated shared material ignored          GREEN
missing required package rejected          GREEN
undeclared module package rejected         GREEN
productive Test Module specialization      ABSENT
cargo fmt --all -- --check                 GREEN
cargo check --locked --all-targets         GREEN
Rust lib suite                             68 passed / 0 failed
neebles-backend suite                      571 passed / 0 failed
domesticacion_elfica                       13 passed / 1 explicitly ignored
git diff --check across 4 repositories     GREEN
```

The explicitly ignored domestic test requires an explicit domestic-root certification context and is not a failing test.

Final technical result:

```text
GLOBAL: PASS
STEP 2AY: GREEN
POINT 7 TECHNICAL IMPLEMENTATION: CERTIFIED
POINT 7 FINAL STATUS: GREEN / CLOSED
```

No productive `neebles-check --module` invocation against a real module was required for Point 7.

Test Module remained frozen as an architecture driver throughout Point 7. With the Boss contract now closed by Point 8, its adaptation belongs to Point 9 and its full certification to Point 10.

---

# Point 8 — Final Boss integration and certification

Point 8 is **GREEN / CLOSED**.

This front did not redesign Boss. It cross-certified the architecture already closed in Points 1 through 7, reconciled the last productive divergences found by current evidence and established the final pre-VM contract baseline.

The closure was intentionally performed with Test Module frozen so that Boss architecture remained independent from a sample module.

## Point 8 scope

The final audit covered:

- owner and source-of-truth boundaries;
- productive seam graph across Boss subsystems;
- AVAILABLE / REQUESTED / REGISTERED / USED capability separation;
- supplied authority vs explicit grant separation;
- persistence and duplicate-truth census;
- transaction, rollback and compensation behavior;
- runtime identity and privilege boundaries;
- UI / Launcher / Tray convergence over canonical state;
- module notification return ownership;
- CUSTOM v2 / Esbirro boundary;
- deprecated and diverging productive paths;
- full Rust regression;
- controlled Qt 6.8.2 builds from the current Boss source;
- BUILD materialization behavior;
- exact final worktree audit.

## Cross-surface convergence

Point 8 certified that productive module state/action mutations publish a Boss surface invalidation event through:

```text
module.lifecycle
```

BossEventClient subscribes to that event family.

Launcher and Tray refresh their module projections from Boss rather than maintaining independent functional truth.

The permanent law remains:

```text
UI / Launcher / Tray
    -> same governed action path
    -> same canonical functional state
    -> cross-surface invalidation
```

Presentation may cache rendering data temporarily, but it does not own a second functional boolean.

## Final legacy reconciliation

Point 8 removed the remaining productive module-level `launcher_action` projection.

Launcher exposure is now derived exclusively from the Schema 4 dynamic `commands` contract and canonical `surface_content`.

The helper that validates a launcher-capable dynamic commands contract remains legitimate validation logic; the removed productive compatibility projection must not return.

Final deprecated/divergence census was GREEN.

## BUILD transactional and authority materialization

Point 8 hardened BUILD publication in three ways.

First, stale platform authority material was reconciled so that:

```text
neebles.domestic_workspace
```

is present in the image-side AuthoritySupply material.

Second, CUSTOM construction publication and OS platform-authority publication were made transactional: failed publication restores the previous valid material instead of exposing a partially replaced tree.

Third, platform-controlled JSON authority material is normalized by BUILD to:

```text
root:root 0644
```

while platform providers remain:

```text
root:root 0755
```

This is required by Boss platform-controlled path authentication, which rejects group-writable or other-writable authority components.

BUILD changes permissions without changing the material bytes.

## Qt 6.8.2 current-source certification

Point 8 did not certify a stale historical copy of Boss.

The current tracked Boss working tree was projected into a disposable controlled CUSTOM Qt world and verified byte-for-byte before compilation.

Final source parity:

```text
tracked regular files checked   491
missing                           0
different                         0
```

The controlled Qt identity was:

```text
Qt                 6.8.2
C++ compiler       /usr/bin/g++
build tool         /usr/bin/gmake
build type         Release
```

Final productive consumers built GREEN:

```text
Boss UI            GREEN
Launcher plugin    GREEN
Tray Host          GREEN
```

The final artifact set contained the Boss UI executable, Tray Host executable and both Launcher shared objects.

During this certification, a latent C++ include defect in Tray compilation was found and fixed by explicitly including `QJsonObject` where `QJsonValue::toObject()` is consumed.

## Point 8 corrective fixes

Point 8 required **zero architectural redesigns**.

It found and closed six local corrective issues:

1. stale BUILD materialization for `neebles.domestic_workspace`;
2. non-transactional BUILD publication paths;
3. missing cross-surface invalidation after productive module mutations;
4. obsolete productive module-level `launcher_action` projection;
5. missing `QJsonObject` include exposed by the real controlled Qt build;
6. BUILD authority JSON mode normalization to `0644`.

None changed the governing architecture of Boss.

## Point 8 final certification

Final pre-VM gate:

```text
cargo fmt --all -- --check                 GREEN
cargo check --locked --all-targets         GREEN
Rust lib suite                             68 passed / 0 failed
neebles-backend suite                      571 passed / 0 failed
domesticacion_elfica                       13 passed / 1 explicitly ignored
productive deprecated census               GREEN
productive host-discovery recertification  0 matches
normal Boss -> neebles-check dependency    ABSENT
BUILD Python syntax                        GREEN
disposable CUSTOM publication              GREEN
disposable platform publication            GREEN
platform authority modes                   GREEN
current Boss -> Qt lab source parity       491 / 491
controlled Qt                              6.8.2
Boss UI build                              GREEN
Launcher plugin build                      GREEN
Tray Host build                            GREEN
Boss git diff --check                      GREEN
BUILD git diff --check                     GREEN
Point 8 worktree contract                  GREEN
```

The explicitly ignored domestic test still requires its explicit domestic-root certification context and is not a failing test.

The first broad host-discovery sensor produced a false RED because it matched the ordinary English word `which` inside comments. The corrected precise sensor found:

```text
PRECISE HOST DISCOVERY MATCHES :: 0
```

No product change was required for that recertification.

## VM acceptance boundary

Point 8 is the final **pre-VM Boss contract gate**.

It does not claim that complete installed-system behavior has already been exercised inside the final N.E.E.B.L.E.S. virtual machine.

The later VM phase remains responsible for real installed-system acceptance, including service startup, desktop integration, sockets, persistence, notifications, Launcher/Tray behavior and full multi-component behavior under the actual image.

A VM finding may expose a real implementation defect and may require correction. It does not reopen or replace the Boss architecture by assumption; current evidence must demonstrate the contradiction first.

## Point 8 final verdict

```text
FINAL :: GREEN
POINT :: 8 FINAL BOSS INTEGRATION / PRE-VM CERTIFICATION CLOSED
BOSS CONTRACT CLOSED :: YES
ARCHITECTURAL REDESIGNS REQUIRED :: 0
LOCAL CORRECTIVE FIXES :: 6
```

Test Module did not drive Point 8.

Boss is now the fixed architecture source for Point 9 adaptation.


---

# Post-Boss roadmap

Points 1 through 8 are **GREEN / CLOSED** and define the closed Boss architecture.

9. **Test Module adaptation — GREEN / CLOSED**
   - Schema 4 and dynamic command contracts are adapted;
   - Governor and generic Lifecycle integration are adapted;
   - authenticated Module IPC is adapted;
   - canonical realtime Settings are adapted;
   - Tray and Notifications v4 are adapted;
   - universal Preinstall and CUSTOM material integration are adapted;
   - Domestic Construction and generic workspace execution are adapted;
   - Registry installation uses an immutable Test Module revision.

Point 9 closes with the final Test Module documentation revision aligned across Test Module HEAD, Boss Registry, CUSTOM fetch and CUSTOM checkout.

10. **Full Test Module certification — IN PROGRESS**

Already GREEN locally: install, disable, enable, disable/re-enable, update, uninstall preserving settings, reinstall preserving settings byte-for-byte, uninstall removing settings, persistent DEB reuse, Runtime/UI/Tray integration, realtime Settings propagation and restart persistence.

Remaining Point 10 fronts include governed OPEN, the complete commands/endpoints matrix, positive and negative Module IPC, Notifications v4, disabled behavior, privilege/rejection paths, Surface behavior and remaining rollback/failure paths.

Point 10 validates Test Module against the already-closed Boss architecture. Full installed-system acceptance remains a later N.E.E.B.L.E.S. OS / VM phase.

> **Order law:** Boss defines the generic contract. Modules consume it. A demonstrated generic defect may require correction, but a module does not redefine Boss architecture by assumption.

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

- commands/casters must avoid shell input patterns known to crash the working Konsole session;
- commands/casters must not contain the shell `$` character;
- do not use shell `set`;
- one implementation step should produce one consolidated validation artifact when evidence is required;
- GREEN advances immediately;
- RED is repaired and recertified before advancing;
- never claim a path was tested without current evidence;
- current source plus the latest certification evidence are the source of truth;
- historical documents do not override current source;
- after **BOSS CONTRACT CLOSED**, Test Module may be adapted only as a consumer of the closed contract and must never become an architecture driver.

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
POINT 4 NOTIFICATIONS          GREEN / CLOSED
POINT 5 AUTH / PRIVILEGES      GREEN / CLOSED
POINT 6 REGISTRY / CATALOG     GREEN / CLOSED
POINT 7 CUSTOM V2              GREEN / CLOSED
POINT 8 FINAL BOSS GATE        GREEN / CLOSED
POINT 9 TEST MODULE            GREEN / CLOSED
POINT 10 TEST MODULE CERT      IN PROGRESS
BOSS CONTRACT CLOSED           YES
SOURCE LINE                    1.0.16
RELEASE VERSION                1.0.16
```

Current continuation law:

**Boss architecture remains closed. Point 9 is GREEN / CLOSED. Continue Point 10 certification against that closed contract, and keep full installed-system / VM acceptance as the later system-level phase.**

---

## Version policy

All current documentation in this repository describes the **1.0.16** source line.

Source updates and documentation updates do not create a release by themselves.
