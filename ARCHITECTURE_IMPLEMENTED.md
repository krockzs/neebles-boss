# N.E.E.B.L.E.S. Boss — Implemented Architecture

Current source line: **1.0.18**

This file describes the architecture implemented in the current Boss working tree.

## Governance boundary

Boss is a generic governor.

It owns contracts, module transactions, authority registration, grants, privilege boundaries, runtime transport, IPC routing, desktop integration, notifications and persistent-state transformation orchestration.

Technology-specific implementation remains owned by the relevant consumer or platform component.

## Authority invariant

```text
resource existence != authority to use resource
```

## Platform ownership

N.E.E.B.L.E.S. OS owns canonical platform authority definitions.

N.E.E.B.L.E.S. BUILD materializes those definitions into the image.

Boss receives AuthoritySupply and consumes the supplied authority.

## Domestic worlds

Boss models controlled worlds for executables, libraries, environment data and filesystem boundaries.

Search authority and execution authority are explicit and independent concepts.

ELF resolution can observe interpreters and required libraries, resolve them inside controlled search authority and certify recursive closure.

## Permanent runtime

The installed Boss runtime is retained below:

```text
/opt/neebles/client/runtime/boss/
```

## Installer transaction

The installer consumes certified resolver and runtime inputs.

Client data and integrations are staged before publication.

Failure paths preserve rollback semantics.

## Module governor

The module governor provides transactional staging, validation, publication and cleanup.

Partial candidates do not become active state.

## Lifecycle boundary

Lifecycle declaration parsing, validation and generic execution are implemented.

Governor actions bind to module-owned transitions through the generic `governor.<action> -> <module-owned transition id>` contract.

Productive execution resolves declarative operations through Battlefield preparation, dependency orchestration, requested capability registration, FireControl, CapabilityRegistry execution, normalized results, communication, state and governed failure handling.

Lifecycle participates in transactional Governor behavior and rollback semantics without learning module technology.

Lifecycle does not own Esbirro, domestic certification, physical module material or technology-specific construction.

## Esbirro boundary

Esbirro uses the generic authority and domestic execution machinery to evaluate declarative spell matrices.

Its contracts and spell data are versioned with Boss.

Its controlled development and runtime material is preserved through N.E.E.B.L.E.S. CUSTOM.

## Recovery

Recovery is external to Boss and belongs to N.E.E.B.L.E.S. BUILD.

`neebles-check` can verify and reconstruct certified corpora independently from ordinary Boss startup.

## Downstream modules

Boss reached its final generic contract before Test Module adaptation and remains the architecture source of truth.

Test Module has since been adapted as a consumer of the closed Boss contract. Its implementation does not constrain Boss technology or introduce Test Module specialization into productive Boss architecture.

Point 9 is GREEN / CLOSED. The final Test Module documentation revision is aligned across Test Module HEAD, Boss Registry and CUSTOM Domestic Construction.

Point 10 full Test Module functional certification remains in progress.

This document applies to **1.0.18** only.
