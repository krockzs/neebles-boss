# N.E.E.B.L.E.S. Boss

**Nested Evolutionary Engine for Behavioral Language Emergent Systems**

Current source line: **1.0.15**

N.E.E.B.L.E.S. Boss is the governance and orchestration layer of the N.E.E.B.L.E.S. ecosystem.

Boss owns contracts, state transitions, privilege boundaries, authority transport, module transactions, runtime IPC and desktop integration.

Technology-specific implementation remains outside the Boss core whenever a generic contract can describe the requirement.

> **Boss governs. Consumers declare. Execution receives explicit authority.**

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

## Runtime authority

The installed Boss client keeps its permanent domestic runtime below:

```text
/opt/neebles/client/runtime/boss/
```

The runtime contains the resolver contract and manifest required by Boss clients.

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

## Domestic execution

The domestic execution layer resolves executable and library requirements inside explicit worlds and search authorities.

The ELF machinery models interpreter observation, dependency observation, search authority, recursive closure and execution grants.

Host executable availability is not treated as implicit authority.

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

## Module transactions

Module installation and update use governed staging and publication semantics.

An incomplete candidate does not become active module state.

Install and update staging are provided as explicit writable authorities.

## Lifecycle

Boss now includes the complete generic Lifecycle execution architecture.

Lifecycle is the communication and execution contract between a module and Boss. It describes **what** must happen without teaching Boss the technology, framework or implementation details of the module.

The architectural boundary is:

> **Boss interprets and governs. Modules own intent and implementation.**

Lifecycle does not perform Esbirro certification, runtime domestication or technology validation. Those concerns belong to the module-construction flow in N.E.E.B.L.E.S. CUSTOM. By the time a module reaches Boss, Lifecycle only needs to interpret and execute the declared contract.

### Lifecycle contract

A module may provide a Lifecycle contract containing:

- arbitrary `hardcoded` string mappings;
- dynamic `require` declarations;
- arbitrary objects;
- arbitrary top-level transitions;
- arbitrary object transitions;
- arbitrary operation identifiers;
- dynamic operation maps for artillery, objective, munition, tactics and intelligence.

Boss does not prescribe transition vocabulary.

`install`, `update`, `uninstall`, `enable`, `disable`, `open`, `close` or any future action are not universal Lifecycle laws. They are only names when a consumer chooses to use them.

Operation values remain declarative until execution preparation, where they are resolved against the current Battlefield.

### Execution architecture

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

A capability may exist in Boss without being requested by a module. A requested capability is registered only for the operation that needs it. Registration does not imply successful execution.

### AVAILABLE arsenal

Boss exposes a productive AVAILABLE capability catalog to Lifecycle.

Governor execution consumes that catalog through `GovernorLifecycleRuntime::from_available(...)`; module operations no longer depend on an empty prebuilt Lifecycle arsenal.

The catalog is the stable expansion point for Rust capability adapters. Adding new Rust capabilities does not require redesigning Lifecycle, Governor, the DAG executor, FireControl or module contracts.

Rust APIs remain compiled and typed. The adapter layer translates the generic Lifecycle `PreparedOperation` into the concrete Rust implementation while Lifecycle itself remains technology-agnostic.

### FireControl and CapabilityRegistry

FireControl resolves the requested `(artillery, objective)` pair to a registered implementation.

CapabilityRegistry owns the executable Rust handler.

Neither layer infers module intent.

The same implementation may receive arbitrary objectives, and Lifecycle does not require artillery or objective names to belong to a fixed vocabulary.

Missing routes, missing implementations and capability failures become normalized technical failures instead of bypassing the Lifecycle result model.

### Governor binding

Governor actions bind to module-owned transitions through the existing generic `hardcoded` map:

```text
governor.<action> -> <module-owned transition id>
```

The action name is arbitrary.

The transition name is arbitrary.

The Governor action and transition do not need to share the same name.

A missing binding is valid absence, not an error.

This means future Governor actions can be introduced without modifying the Lifecycle binding engine.

### Governor integration

The Module Governor is adapted as a consumer of Lifecycle for its current operations:

```text
install
update
uninstall
enable
disable
```

These names belong to the Governor consumer; they are not hardcoded into the generic Lifecycle engine.

Boss first resolves the module-owned transition bound to the requested Governor action and then executes that transition through the same generic Lifecycle pipeline used by any other consumer.

The Test Module does not define or constrain this architecture. It must adapt to the Boss contract after the Boss contract is closed.

### Transactions, failure and rollback

Lifecycle execution is integrated with the existing transactional module machinery.

The current Governor paths preserve transactional behavior across install, update, uninstall, enable and disable operations.

Require acquisition keeps a transaction journal and compensates acquired requirements in reverse order on rollback.

Module staging does not publish incomplete candidates.

Update keeps recovery material available until Lifecycle succeeds and restores the previous module state when execution fails.

Lifecycle failures are represented explicitly and remain distinct from infrastructure failures and cancellation.

Rollback failure does not prevent remaining compensations from being attempted.

### State, intelligence and communication

Lifecycle maintains explicit runtime state for execution, transition, operation and object scope.

Successful operation payloads may be selected into persistent intelligence for later operations.

Ephemeral result data is not confused with persistent intelligence.

Communication snapshots expose dynamic progress, result and failure namespaces without imposing a fixed schema on future fields.

Require context and object identity remain explicit throughout execution.

### Objects and object transitions

Objects are dynamic module-owned identities.

Boss does not infer object state from the contract and does not prescribe object-transition names.

Object transitions compile and execute through the same generic execution architecture while preserving object identity in runtime state, communication and failure context.

### Control and orchestration

Lifecycle supports dependency graphs, independent operations, joins and dependency chains.

Execution control supports explicit retry limits, deadlines and cancellation.

A failed dependency blocks dependent work without destroying unrelated successful work.

Runtime intelligence produced by one operation can feed later dependent operations before their values are materialized.

### Result and failure model

Every operation terminates through a normalized result model:

```text
success
failure
cancelled
```

Cancellation is explicit and is never inferred from error text.

Capability failures, route failures and infrastructure failures remain distinguishable.

Transition failure produces structured failure context that can flow into events and optional telemetry.

### Certification status

The complete Lifecycle path has been exercised through focused tests, destructive module-transaction tests and the full Boss test suite.

The current source line has certified:

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
- cancellation, retry and deadlines;
- event generation;
- telemetry packaging;
- install/update/uninstall/enable/disable Governor integration.

Lifecycle is therefore implemented in the current Boss `1.0.15` source line.

## Pending roadmap

Lifecycle backend/governance is closed. The remaining work now starts from the UI surface and then continues through the remaining Boss layers.

1. **Lifecycle UI**
   - consume existing `RuntimeState` and Lifecycle Communication;
   - expose `state`, `progress`, `result`, `failure` and `human`;
   - display transitions, operations and objects without adding module-specific semantics;
   - do not create a second semantic channel beside Lifecycle.

2. **Contracts + Module IPC**
   - close the runtime boundary between modules and Boss;
   - cover session, registration, ownership, endpoints and invoke/response;
   - preserve ping/pong, shutdown and unregister as transport infrastructure;
   - IPC transports; Lifecycle governs execution and runtime semantics.

3. **Settings + symbols / hardcoded**
   - separate Boss settings infrastructure from module-declared settings;
   - Boss owns generic persistence, overrides, resolution, ownership, validation and access;
   - modules declare and consume their settings;
   - reuse existing `hardcoded` / symbols semantics instead of introducing a parallel symbol system.

4. **Nightmare + Critical Update**
   - preserve Nightmare as a transversal declarative transformation engine;
   - integrate Critical Update as a real consumer;
   - close publication, failure and rollback semantics for persistent transformations.

5. **Boss Tray + Plasma**
   - reconcile Tray Manager, tray socket, Tray Host, StatusNotifierItem and Plasma;
   - consume Lifecycle, Settings and IPC where appropriate;
   - do not duplicate state or execution semantics.

6. **Notifications**
   - close the generic notification contract and ownership model;
   - define emission, transport, state and capabilities;
   - do not turn Notifications into a parallel control channel.

7. **Auth / privileges**
   - close `requires_root`, auth-agent and privilege escalation semantics;
   - align Lifecycle, IPC, Settings and sensitive operations with explicit authority;
   - keep the Stage 8 law: physical availability never implies permission.

8. **Registry / module catalog**
   - close metadata, version, installed state, enable/disable state and contract references;
   - align Registry with Governor, Lifecycle, Settings and IPC;
   - the catalog describes and governs identity/state; it does not execute module technology.

9. **CUSTOM v2 integration into Boss**
   - integrate certified CUSTOM v2 output without making Boss understand module technologies;
   - keep Esbirro outside Lifecycle;
   - the future `world Modules` belongs to CUSTOM v2 / Esbirro;
   - preserve Stage 8 host independence.

10. **Final Boss integration and certification**
    - cross-certify Lifecycle UI, Module IPC, Settings, Nightmare/Critical Update, Tray, Notifications, Auth, Registry and CUSTOM v2;
    - run the real Boss E2E;
    - remove any remaining duplicate or diverging contracts;
    - declare **BOSS CONTRACT CLOSED** only after global GREEN.

11. **Adapt Test Module to what Boss dictates**
    - unfreeze Test Module only after **BOSS CONTRACT CLOSED**;
    - adapt manifest, contracts, Lifecycle, Module IPC, Settings, Tray, Notifications and Auth;
    - use CUSTOM v2 + Esbirro for module construction/domestication/certification;
    - Test Module demonstrates the final Boss contract and never drives Boss architecture.

12. **Full Test Module certification**
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

## Recovery boundary

Recovery is external to Boss.

N.E.E.B.L.E.S. BUILD owns the independent `neebles-check` recovery environment.

Boss normal startup does not depend on invoking the recovery engine.

## Telemetry

Telemetry is an optional generic error-reporting channel.

It is not a dependency engine, readiness gate or recovery engine.

Normal Boss operation does not require a remote telemetry endpoint.

## Nightmare

Nightmare is the declarative transformation engine for persistent state.

Its role is transversal: formulas describe transformations without forcing Boss to gain technology-specific branches for every consumer.

## Version policy

All current documentation in this repository describes the **1.0.15** source line.

Source updates and documentation updates do not create a release by themselves.
