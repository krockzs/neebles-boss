# N.E.E.B.L.E.S. Telemetry

N.E.E.B.L.E.S. Telemetry exists to report meaningful runtime and diagnostic events produced by Boss and the system components governed by it.

It is not a general activity tracker and it is not intended to continuously describe everything the user does.

Its purpose is much narrower:

> When N.E.E.B.L.E.S. tries to execute, repair, validate or initialize something and an important technical event occurs, the External subsystem can report what happened.

Telemetry is controlled by one persistent Boss setting:

`telemetry.enabled`

The default value is:

`false`

A fresh N.E.E.B.L.E.S. installation therefore does not transmit External telemetry unless the user explicitly enables it.

---

## Why telemetry exists

N.E.E.B.L.E.S. is designed to automate, validate and repair many parts of its own environment.

To do that, Boss frequently interacts with software that N.E.E.B.L.E.S. does not own.

For example, Boss may need to install a dependency through APT. A normal operation could ultimately require something equivalent to:

```text
sudo apt install <package>
```

APT is an external dependency from the point of view of N.E.E.B.L.E.S. Boss governs the operation, but Boss does not control the development of APT itself.

A future version of APT could change an argument, modify expected behaviour, reject an option that previously worked or return a new kind of error.

Without diagnostic reporting, an installation recipe that worked correctly yesterday could begin failing on N.E.E.B.L.E.S. systems without the maintainers immediately knowing why.

Telemetry exists in part to make those changes visible.

For example:

```text
Boss requests an installation
        ↓
APT is invoked
        ↓
APT behaviour has changed
        ↓
an argument is no longer accepted
        ↓
Boss detects the execution failure
        ↓
External event: error
```

That event can provide enough structured information to identify that an external component changed and that N.E.E.B.L.E.S. may require an updated installer recipe.

---

## Operational purpose

When telemetry is enabled and External transport is configured, diagnostic events may be delivered to telemetry infrastructure operated by the creator or maintainers of N.E.E.B.L.E.S.

The purpose of receiving those events is to detect ecosystem problems early.

Examples include:

- an external installer changing its command-line behaviour;
- an installer recipe becoming obsolete;
- a dependency no longer satisfying the expected version contract;
- Stage0 being unable to recover a required system component;
- Boss failing during initialization or runtime;
- a N.E.E.B.L.E.S. module encountering a runtime error;
- a module dependency becoming incompatible with the current ecosystem.

A simplified operational cycle is:

```text
N.E.E.B.L.E.S. installation
        ↓
technical problem occurs
        ↓
Boss / Stage0 / module detects it
        ↓
External telemetry event
        ↓
telemetry infrastructure
        ↓
maintainers identify a recurring ecosystem problem
        ↓
installer, contract or component is corrected
        ↓
N.E.E.B.L.E.S. hotfix can be prepared and published
```

Telemetry does not itself modify the operating system or automatically create or install a hotfix. Its role is diagnostic.

The normal N.E.E.B.L.E.S. update mechanisms remain responsible for distributing corrections.

This separation allows telemetry to help the creator and maintainers react quickly to ecosystem changes without making telemetry part of the repair mechanism itself.

---

## Installers and dependency knowledge

N.E.E.B.L.E.S. does not hardcode every installation procedure independently inside every subsystem.

Installer definitions are represented through system data such as:

`instaladores_amd64.json`

This catalog contains installation knowledge for supported amd64 dependencies and tools.

Conceptually, Boss can ask:

```text
I need dependency X.
How is X installed on this architecture?
```

The installer catalog provides the corresponding recipe.

Boss executes that procedure and then verifies the resulting state.

This distinction is important. N.E.E.B.L.E.S. does not assume that an installation worked merely because an installer command returned. The resulting dependency contract is checked again.

For example:

```text
Boss requires package X
        ↓
instaladores_amd64.json contains its installer recipe
        ↓
Boss executes the recipe
        ↓
APT reports an unexpected argument
        ↓
External event: error
```

A future hotfix could then update the affected installer recipe.

Another situation is:

```text
Boss requires version >= 3.0
        ↓
installed version is 2.4
        ↓
Boss attempts repair or update
        ↓
dependency is checked again
        ↓
installed version is still 2.4
        ↓
External event: incompatibility
```

An execution error and an incompatibility are therefore different diagnostic conditions.

---

## `error`

The `error` event represents a runtime or execution failure.

Typical examples include:

- an installer command failing;
- APT rejecting an argument;
- an external tool returning an unexpected error;
- a Boss runtime operation failing;
- a module reporting an execution error through the Boss IPC contract.

Example:

```text
Boss
  ↓
requires a dependency
  ↓
installer recipe invokes APT
  ↓
APT rejects an argument
  ↓
Boss receives an execution failure
  ↓
External event: error
```

The current error message contains:

- `code`
- `message`

Module-originated errors may additionally identify their source through:

- `package.module`

This allows the diagnostic system to identify what failed and where the failure originated.

---

## `incompatibility`

An `incompatibility` event means that a component exists but does not satisfy the contract required by N.E.E.B.L.E.S.

For example:

```text
Boss requires dependency X >= 5.0
        ↓
dependency X is installed
        ↓
detected version: 4.2
        ↓
Boss attempts repair/update
        ↓
dependency is verified again
        ↓
detected version is still 4.2
        ↓
External event: incompatibility
```

This is different from simply reporting that an installer command failed.

The installer may have completed successfully while the final system state still fails to satisfy the required contract.

An incompatibility event currently describes:

- `component`
- `current`
- `required`
- `message`

This lets maintainers identify both the state N.E.E.B.L.E.S. found and the state the ecosystem expected.

If the incompatibility is caused by an external change, that information can be used to prepare an updated contract, installer definition or hotfix.

---

## Stage0

Stage0 is the early N.E.E.B.L.E.S. preflight and recovery phase.

Its job is not merely to detect problems. Its normal model is:

```text
check
  ↓
detect problem
  ↓
attempt recovery
  ↓
verify again
  ↓
continue when healthy
```

For example:

```text
N.E.E.B.L.E.S. starts
        ↓
Stage0 checks required system state
        ↓
dependency is missing or invalid
        ↓
Boss attempts repair
        ↓
dependency is checked again
        ↓
healthy
        ↓
Stage0 reaches ready
```

The important part is that Stage0 attempts to heal recoverable problems before treating them as permanent failures.

Recovery, however, cannot always succeed.

For example:

```text
N.E.E.B.L.E.S. starts
        ↓
Stage0 detects a problem
        ↓
repair is attempted
        ↓
repair cannot satisfy the required contract
        ↓
verification still fails
        ↓
External diagnostic event can describe the problem
```

That information is useful because a failure that appears across multiple installations may indicate that something external to N.E.E.B.L.E.S. changed.

Stage0 itself can also produce `stage0` events describing runtime state.

Current Stage0 message fields are:

- `state`
- `message`

These events help identify problems during initialization of the N.E.E.B.L.E.S. ecosystem.

Telemetry must never become a dependency required for Stage0 survival.

If telemetry, the network or External delivery is unavailable, Stage0 must continue performing its local responsibilities.

N.E.E.B.L.E.S. must not require Internet access merely to boot, validate or repair itself.

---

## Boss and module initialization

Telemetry also helps diagnose failures that occur while the N.E.E.B.L.E.S. ecosystem itself is being initialized.

This includes both Boss and modules.

For example:

```text
Boss starts
   ↓
runtime component initializes
   ↓
required dependency or contract fails
   ↓
Boss detects the problem
   ↓
External error or incompatibility event
```

A module can encounter a similar situation:

```text
module starts
    ↓
module requests or uses a dependency
    ↓
dependency is incompatible
    ↓
Boss receives the module-side failure
    ↓
External diagnostic event
```

Modules may describe their failures. They do not control whether telemetry is enabled.

---

## The External pipeline

Current runtime producers include:

- Boss;
- Stage0;
- dependency integrity handling;
- module runtime paths.

All current producers use the same Boss-controlled telemetry gate.

Conceptually:

```text
Boss / Stage0 / dependency / module
                ↓
        wants to report event
                ↓
       global telemetry gate
          ↓             ↓
        OFF             ON
         ↓               ↓
     stop here      build envelope
                         ↓
                  external.sock
                         ↓
                 External boundary
                         ↓
                configured transport
                         ↓
              telemetry infrastructure
```

The producer describes what happened.

Boss decides whether reporting is allowed.

External owns the transport boundary.

---

## Master telemetry control

Boss is the single source of truth for telemetry state.

The persistent setting is:

`telemetry.enabled`

Modules cannot enable this setting.

A module may produce an External event, but it cannot decide that global telemetry should become active.

This distinction is intentional:

> Boss governs; the producer describes; External transports.

---

## When telemetry is OFF

When telemetry is disabled:

- the global setting resolves to `false`;
- External reporting stops before the payload is constructed;
- the package builder is not executed;
- the message builder is not executed;
- no normal production External envelope is sent to `external.sock`.

If Boss cannot read the telemetry setting, the system behaves as if telemetry were disabled.

This is a privacy-safe failure mode.

Simplified:

```text
event occurs
    ↓
telemetry.enabled == false
    ↓
stop
```

No diagnostic payload needs to be built merely to discard it later.

---

## When telemetry is ON

When telemetry is enabled:

```text
event occurs
    ↓
telemetry.enabled == true
    ↓
build External envelope
    ↓
send to external.sock
    ↓
External subsystem processes the event
    ↓
configured transport may deliver it
```

Enabling telemetry does not alter how Stage0 repairs the system, how dependencies are resolved or how modules execute.

It only permits their External diagnostic reports to proceed.

---

## `external.sock`

`external.sock` is the local N.E.E.B.L.E.S. External boundary.

It is a Unix domain socket used to separate event producers from the External transport layer.

It is important to distinguish:

```text
External producer
        ↓
external.sock
```

from:

```text
remote telemetry infrastructure
```

They are not the same thing.

`external.sock` is local IPC.

The transport behind that boundary is responsible for any configured remote delivery.

This allows Boss, Stage0 and modules to use one stable External contract without needing to know how the eventual server transport is implemented.

---

## External envelope

External events use the generic envelope:

```json
{
  "type": "<String>",
  "endpoint": "<String>",
  "activate": true,
  "package": {},
  "message": {}
}
```

### `type`

Identifies the event category.

Current runtime categories include:

- `error`
- `incompatibility`
- `stage0`

### `endpoint`

Identifies the destination expected by the External transport.

The producer itself does not implement the remote transport.

### `activate`

Represents whether the envelope was constructed as an active External event.

During normal production operation, disabled telemetry returns before constructing the envelope.

### `package`

Contains producer-specific context.

For example, a module-originated error may identify its module.

### `message`

Contains structured information specific to the event category.

An incompatibility message can contain:

```text
component
current
required
message
```

A Stage0 event can contain:

```text
state
message
```

---

## Defensive boundaries

N.E.E.B.L.E.S. uses defensive layers around External events.

The production producer checks the global telemetry setting before constructing the payload.

The External receiver also rejects envelopes that declare themselves inactive.

The producer-side gate avoids unnecessary creation and delivery of diagnostic payloads.

The receiver-side check protects the External boundary against inactive envelopes reaching it.

---

## Persistence

Telemetry state is stored through the normal Boss persistent settings architecture.

Changing the switch updates the Boss setting immediately.

The value survives:

- closing Boss;
- reopening Boss;
- restarting the system.

A fresh installation begins with telemetry disabled.

---

## Boss interface

Telemetry is controlled from:

**Telemetry Settings**

The switch is applied immediately.

There is no separate Apply or Save operation.

When the state changes, Boss reports:

```text
Telemetry activated
```

or:

```text
Telemetry deactivated
```

The interface also links directly to this document so users can understand what enabling telemetry means and why the system provides it.

---

## Hotfix workflow

Telemetry is intended to shorten the time between an external ecosystem change and a N.E.E.B.L.E.S. correction.

For example:

```text
APT changes behaviour
        ↓
installer recipe begins failing
        ↓
enabled N.E.E.B.L.E.S. systems report structured errors
        ↓
creator or maintainers identify the common cause
        ↓
instaladores_amd64.json or another affected component is corrected
        ↓
hotfix is prepared
        ↓
normal N.E.E.B.L.E.S. update mechanisms distribute the correction
```

The same principle applies to:

- dependency incompatibilities;
- Stage0 recovery failures;
- Boss initialization problems;
- module initialization or runtime problems.

Telemetry provides visibility.

The N.E.E.B.L.E.S. update system provides the correction.

---

## Design principle

Telemetry follows the same responsibility separation used throughout N.E.E.B.L.E.S.:

> Boss governs; the producer describes; External transports.

Boss controls whether reporting is permitted.

Boss, Stage0, dependencies and modules describe what happened.

External provides the reporting boundary.

Telemetry infrastructure allows the creator or maintainers of N.E.E.B.L.E.S. to identify recurring ecosystem problems and prepare corrections.

No individual module is allowed to silently enable global telemetry.
