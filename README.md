# N.E.E.B.L.E.S. OS

N.E.E.B.L.E.S. OS is the operating-system layer of the N.E.E.B.L.E.S. ecosystem.

It contains the platform-side configuration, bootstrap resources and canonical platform authority definitions required to build and run N.E.E.B.L.E.S. without coupling Boss to the Linux distribution underneath it.

The OS is based on Debian Trixie and KDE Plasma.

## Architectural role

N.E.E.B.L.E.S. OS describes the platform.

It does not teach Boss how Linux works and it does not duplicate module semantics. Its responsibility is to expose the platform authority and bootstrap material that the rest of the ecosystem consumes through stable contracts.

The current ownership chain is:

```text
N.E.E.B.L.E.S. OS
    describes platform authority
            |
            v
N.E.E.B.L.E.S. BUILD
    materializes it into the image
            |
            v
N.E.E.B.L.E.S. Boss
    consumes the resulting authority
            |
            v
N.E.E.B.L.E.S. CUSTOM / Esbirro
    supplies certified domestic runtime material
    required by module execution
```

Boss therefore remains independent from the Linux distribution currently hosting it.

## Bootstrap

The OS bootstrap transports the information required to connect the installed system with Boss.

Its current responsibilities include:

- carrying domestic runtime information into the Boss installation flow;
- transporting the platform `AuthoritySupply`;
- providing the platform-side material required by the installed N.E.E.B.L.E.S. stack;
- keeping Linux-specific knowledge outside Boss.

Bootstrap resources belong to the OS/platform boundary. They are not module lifecycle logic.

## Platform authority

N.E.E.B.L.E.S. OS is the semantic owner of platform authority.

Canonical authority definitions live below:

```text
platform/authority/
```

Current authority families include:

```text
platform.filesystem_boundary
system.dns_resolver_config
boss.modules.install_staging
boss.modules.update_staging
```

These authorities describe what the platform provides. Boss consumes them as declared capabilities instead of hardcoding host-specific Linux paths or assumptions.

## Platform boundary provider

The platform boundary provider lives below:

```text
platform/bin/
```

Its job is to expose the OS-owned platform boundary in the form expected by the rest of N.E.E.B.L.E.S.

The provider belongs to the OS layer. Boss must consume its result rather than reimplementing the same platform knowledge.

## BUILD integration

N.E.E.B.L.E.S. BUILD is responsible for materializing OS definitions into the generated image.

The boundary is intentionally strict:

```text
OS
    defines

BUILD
    materializes

Boss
    consumes
```

BUILD does not become the semantic owner of the authority it packages.

## CUSTOM and Esbirro integration

Linux-specific runtime material used by modules is intentionally kept outside Boss.

N.E.E.B.L.E.S. CUSTOM works with Esbirro to produce and certify the domestic runtime material required by module execution.

This keeps the responsibilities separated:

```text
OS
    platform authority

BUILD
    image materialization

Boss
    generic interpretation and coordination

CUSTOM
    domestic runtime material

Esbirro
    validation, certification and authority enforcement
```

Boss is therefore free to interpret module contracts without knowing which package manager, filesystem convention, shell environment or Linux distribution implementation produced the underlying capability.

## Boss integration boundary

Boss is allowed to consume platform authority, but it must not become the owner of Linux-specific knowledge.

The current architectural rule is:

```text
platform fact
    -> OS authority

image construction
    -> BUILD

domestic runtime implementation
    -> CUSTOM / Esbirro

module semantics
    -> module Lifecycle / contracts

coordination
    -> Boss
```

This separation is what allows N.E.E.B.L.E.S. OS, Boss and modules to evolve independently.

## Current state

The platform isolation work is complete for the current architecture.

Boss no longer needs to own the Linux-specific knowledge that was moved behind platform authority and domestic runtime boundaries.

The subsequent Boss work also consolidated module runtime, settings, Surface and administrative authority so the platform boundary established here remains the single intended direction of dependency:

```text
OS / BUILD / CUSTOM
        |
        v
      Boss
        |
        v
     Modules
```

There is no reverse dependency in which OS must understand module implementation details.

## Ownership rule

The canonical ownership rule remains:

```text
OS describes platform authority.

BUILD materializes it.

Boss consumes it.

CUSTOM supplies certified domestic runtime material.

Esbirro validates and certifies that material.

Modules declare their own behavior through contracts and Lifecycle.
```

That boundary is part of the current N.E.E.B.L.E.S. architecture and should be preserved by future changes.
