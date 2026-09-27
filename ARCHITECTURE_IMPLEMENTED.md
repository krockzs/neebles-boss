# N.E.E.B.L.E.S. Boss — Implemented Architecture

Current source line: **1.0.15**

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

Lifecycle declaration parsing and validation are implemented.

Generic lifecycle execution is not implemented yet.

The existence of lifecycle recipes must not be interpreted as proof of a generic execution engine.

## Esbirro boundary

Esbirro uses the generic authority and domestic execution machinery to evaluate declarative spell matrices.

Its contracts and spell data are versioned with Boss.

Its controlled development and runtime material is preserved through N.E.E.B.L.E.S. CUSTOM.

## Recovery

Recovery is external to Boss and belongs to N.E.E.B.L.E.S. BUILD.

`neebles-check` can verify and reconstruct certified corpora independently from ordinary Boss startup.

## Downstream modules

Boss reaches its final generic contract first.

The Test Module is adapted afterward and does not constrain Boss design.

This document applies to **1.0.15** only.
