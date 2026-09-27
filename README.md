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

## Lifecycle status

The generic lifecycle declaration contract exists.

Boss can resolve, load and validate lifecycle references and generic recipe fields.

**Generic lifecycle execution is not implemented yet.**

That execution engine will be completed in Boss before downstream modules are adapted to it.

The Test Module does not define or constrain Boss architecture.

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
