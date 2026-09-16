# N.E.E.B.L.E.S. Telemetry

N.E.E.B.L.E.S. Telemetry is a diagnostic system designed to help maintain the N.E.E.B.L.E.S. ecosystem when software outside N.E.E.B.L.E.S. changes.

Its purpose is operational. It is not intended for advertising, profiling, analytics, or measuring how a user uses the operating system.

When enabled by the user, telemetry allows defined N.E.E.B.L.E.S. diagnostic events to be sent through the External subsystem toward telemetry infrastructure operated by the creator or maintainer of N.E.E.B.L.E.S.

The objective is simple:

> Detect early when an installer, dependency, compatibility contract, Boss initialization path, Stage0 recovery path, or module stops working as expected, so the maintainer can identify the cause and prepare a hotfix.

Telemetry is controlled by the persistent Boss setting:

```text
telemetry.enabled
```

The default value is:

```text
false
```

A fresh N.E.E.B.L.E.S. installation therefore starts with telemetry disabled.

---

# 1. Why N.E.E.B.L.E.S. needs telemetry

N.E.E.B.L.E.S. owns its own orchestration, contracts, modules, settings, recovery logic and installer abstraction.

It does **not** own Debian, APT, dpkg, curl, Git, third-party libraries, external repositories, package naming rules, command-line syntax, or the future behaviour of those projects.

That creates a permanent maintenance problem.

An installer recipe can be correct today and fail tomorrow even if the N.E.E.B.L.E.S. source code itself has not changed.

For example, an installer definition may currently resolve to something conceptually equivalent to:

```text
sudo apt install <package>
```

Today that syntax may be valid.

A future version of APT could change an argument, remove an option, alter a package name, change dependency behaviour, or return a new error for a previously valid operation.

N.E.E.B.L.E.S. must therefore assume that external software will evolve.

Telemetry exists so the maintainer can discover that the assumptions used by N.E.E.B.L.E.S. are no longer true.

The intended maintenance cycle is:

```text
External software changes
        ↓
N.E.E.B.L.E.S. executes one of its own known procedures
        ↓
Boss detects a defined technical problem
        ↓
Telemetry reports the diagnostic event
        ↓
N.E.E.B.L.E.S. maintainer identifies the cause
        ↓
Hotfix is prepared
        ↓
Normal N.E.E.B.L.E.S. update mechanisms distribute the correction
```

Telemetry does not itself repair the machine from the remote server.

Telemetry provides the signal that tells the maintainer that something in the ecosystem needs attention.

---

# 2. The installer dictionary

N.E.E.B.L.E.S. does not scatter hardcoded installation commands throughout Boss and its modules.

Installer behaviour is described through an installer dictionary.

For the current amd64 architecture, the relevant dictionary is:

```text
instaladores_amd64.json
```

The bundled copy is located at:

```text
/opt/neebles/client/config/installers/instaladores_amd64.json
```

A validated cached copy can also exist at:

```text
/opt/neebles/shared/cache/installers/instaladores_amd64.json
```

The dictionary describes installer operations, commands, argument flows, privilege requirements, expected behaviour and other information required to convert an abstract N.E.E.B.L.E.S. request into a real system command.

Conceptually, Boss can request:

```text
installer: apt
operation: install
packages: [...]
```

The installer subsystem then resolves the correct command from the dictionary.

This means Boss does not need to know every package-manager command directly.

That separation is intentional because external tools can change without forcing a redesign of Boss.

If an installer command changes, a hotfix may only need to update the installer dictionary or the affected contract.

Telemetry helps reveal when such a hotfix has become necessary.

---

# 3. The three telemetry categories

The current External diagnostic architecture distinguishes three important categories:

```text
error
incompatibility
stage0
```

They represent different technical situations.

The distinction matters because each category tells the maintainer something different about the type of failure.

---

# 4. `error`: the operation itself failed

An `error` means N.E.E.B.L.E.S. attempted to execute an operation and that execution failed.

This is the category used when the operation itself no longer behaves as expected.

## Example: APT nomenclature changes

Imagine the installer dictionary contains a recipe that resolves to:

```text
sudo apt install --some-option example-package
```

At the time the recipe was created, APT accepts:

```text
--some-option
```

Later, a new APT version removes or renames that option.

The flow becomes:

```text
Boss
  ↓
requests apt/install
  ↓
instaladores_amd64.json resolves the operation
  ↓
Boss executes the resulting APT command
  ↓
APT rejects an argument
  ↓
operation fails
  ↓
External type: error
```

The important fact is:

```text
The operation itself failed.
```

That is an `error`.

Current error messages can contain:

```text
code
message
```

Module-originated errors may additionally identify the source module through:

```text
package.module
```

From the maintainer's point of view, an `error` means:

> N.E.E.B.L.E.S. attempted an operation that should have worked according to its current installer or runtime contract, but execution failed.

Typical investigation questions are:

- Did APT change an argument?
- Did a package change name?
- Did an external tool remove an option?
- Did a repository alter expected behaviour?
- Did a command-line interface change?
- Did a module runtime operation fail?

A likely correction may be an updated installer recipe, Boss change, or module hotfix.

---

# 5. `incompatibility`: the operation may work, but the resulting system no longer satisfies the contract

An `incompatibility` is different from an execution error.

The command itself may have completed successfully.

The problem appears when N.E.E.B.L.E.S. verifies the resulting state and discovers that the installed component no longer satisfies the required contract.

## Example: a Linux upgrade changes a dependency version

Imagine a module requires:

```text
libexample >=5.0,<6.0
```

The machine currently has:

```text
libexample 5.4
```

Everything is compatible.

The user then performs:

```text
sudo apt upgrade
```

or:

```text
sudo apt full-upgrade
```

The upgrade succeeds and replaces the dependency with:

```text
libexample 6.1
```

APT worked.

Linux is not broken.

But the current N.E.E.B.L.E.S. contract still requires:

```text
>=5.0,<6.0
```

Boss detects:

```text
current: 6.1
required: >=5.0,<6.0
```

The flow becomes:

```text
System upgrade
      ↓
Dependency version changes
      ↓
Boss or module preflight
      ↓
Installed version is queried
      ↓
current = 6.1
required = >=5.0,<6.0
      ↓
Contract is not satisfied
      ↓
External type: incompatibility
```

Current incompatibility messages can contain:

```text
component
current
required
message
```

For example:

```text
component: libexample
current: 6.1
required: >=5.0,<6.0
message: Installed version does not satisfy the required dependency contract
```

From the maintainer's point of view, an `incompatibility` means:

> The surrounding Linux ecosystem moved outside the compatibility contract currently known by N.E.E.B.L.E.S.

The appropriate hotfix may involve adapting Boss, updating a module, changing a dependency contract, supporting a newer dependency, changing an installer definition, or releasing a compatibility update.

---

# 6. A current N.E.E.B.L.E.S. compatibility example

Stage0 currently knows version contracts for system components such as:

```text
libqt6quick6 >=6.5,<7.0
```

and:

```text
liblayershellqtinterface6 >=6.5,<7.0
```

Imagine the operating system later installs a version outside one of those ranges.

APT may still work. The package may still exist. The system upgrade may complete successfully.

But N.E.E.B.L.E.S. verifies the installed version afterwards.

If the installed version no longer satisfies the contract, the condition is an incompatibility.

The distinction is:

```text
ERROR
"I could not execute what N.E.E.B.L.E.S. asked me to execute."

INCOMPATIBILITY
"I executed successfully, but the resulting component
does not satisfy what N.E.E.B.L.E.S. requires."
```

---

# 7. `stage0`: N.E.E.B.L.E.S. is checking and repairing its own environment

The third category belongs to the N.E.E.B.L.E.S. initialization and recovery path itself.

Stage0 is the early preflight and recovery phase of Boss.

Its responsibility is not simply to check whether the environment is healthy.

Its model is:

```text
check
  ↓
detect problem
  ↓
repair when possible
  ↓
verify again
  ↓
continue only from a known state
```

Stage0 exists so N.E.E.B.L.E.S. can attempt to recover from missing or invalid system dependencies before the rest of the ecosystem continues.

This is why Stage0 is part of telemetry.

A Stage0-related diagnostic means the N.E.E.B.L.E.S. ecosystem encountered an important condition while establishing or repairing its own operating environment.

---

# 8. What Stage0 currently does

Stage0 begins by publishing its runtime state as:

```text
service: service.stage0
ready: false
status: checking
```

The current high-level flow is:

```text
Stage0 starts
    ↓
publish "checking"
    ↓
attempt installer dictionary refresh
    ↓
resolve critical system dependencies
    ↓
preflight installed modules
    ↓
publish "ready"
```

When successful:

```text
ready: true
status: ready
```

If the initialization path is marked as failed:

```text
ready: false
status: failed
```

The runtime state is stored at:

```text
/run/neebles/stage0.json
```

---

# 9. Stage0 recovery does not depend on Internet access

Stage0 is designed so that remote availability is not required for basic local recovery.

N.E.E.B.L.E.S. keeps local installer information.

The bundled installer dictionary is:

```text
/opt/neebles/client/config/installers/instaladores_amd64.json
```

A validated cache can exist at:

```text
/opt/neebles/shared/cache/installers/instaladores_amd64.json
```

During Stage0, Boss attempts to refresh the installer dictionary.

That refresh is non-blocking.

If the remote source cannot be reached, Stage0 continues using local recovery data.

Conceptually:

```text
Remote installer information available?
              ↓
       yes           no
        ↓             ↓
use/update cache   continue locally
```

Telemetry must never become a dependency required for Stage0 survival.

A network failure or telemetry failure must not prevent N.E.E.B.L.E.S. from attempting local initialization and recovery.

---

# 10. Stage0 and the tools used for recovery

Stage0 defines critical dependencies required by the current N.E.E.B.L.E.S. environment.

Current examples include:

```text
git
curl
libnotify-bin
libqt6quick6
liblayershellqtinterface6
```

For a system package, Stage0 can express the installation request through the local installer abstraction:

```text
installer: apt
operation: install
```

The installed state is then verified through:

```text
installer: dpkg-query
operation: check_installed
```

When a version contract exists, N.E.E.B.L.E.S. can also query the installed version and compare it against the required Debian-version range.

The key architectural rule is:

```text
N.E.E.B.L.E.S. does not assume that a repair succeeded
just because an installer command returned successfully.
```

The real flow is:

```text
Dependency missing or invalid
      ↓
Installer recipe resolved
      ↓
APT repair attempted
      ↓
Installed state queried
      ↓
Version queried when required
      ↓
Contract evaluated again
```

Only the verified final state matters.

---

# 11. Example: Stage0 detects a broken installer recipe

Imagine `curl` is missing.

Stage0 attempts to repair the environment:

```text
Stage0 starts
      ↓
curl is missing
      ↓
Boss requests apt/install for curl
      ↓
instaladores_amd64.json resolves the operation
      ↓
APT command executes
      ↓
APT rejects the current syntax
      ↓
External type: error
```

This tells the maintainer that the self-recovery path attempted a known installer recipe and the recipe itself failed.

That is a strong indication that the installer definition or external command behaviour may need a hotfix.

---

# 12. Example: Stage0 detects an incompatibility

Another case:

```text
Stage0 starts
      ↓
required Qt library exists
      ↓
installed version is checked
      ↓
version is outside the supported range
      ↓
repair/update is attempted
      ↓
version is verified again
      ↓
contract is still not satisfied
      ↓
External type: incompatibility
```

This tells the maintainer that the package exists and the repair path ran, but the resulting environment is still outside the supported N.E.E.B.L.E.S. contract.

---

# 13. Why Stage0 telemetry matters

Stage0 is one of the first parts of the ecosystem able to reveal that the assumptions built into N.E.E.B.L.E.S. no longer match the real operating system.

A runtime module error and an early Stage0 initialization failure are not the same thing.

Stage0 provides diagnostic context that the problem occurred while N.E.E.B.L.E.S. was starting, checking, repairing, verifying, or preflighting modules.

This helps the maintainer determine whether the failure affects the ecosystem before it even reaches its normal ready state.

A Stage0 failure can therefore reveal that the current release can no longer establish the environment it expects.

That is exactly the kind of condition that may require a rapid hotfix.

---

# 14. Modules use the same diagnostic architecture

N.E.E.B.L.E.S. modules can also have runtime errors and compatibility contracts.

For example:

```text
Module A requires Module B >= 2.0
```

but the installed module is:

```text
Module B 1.7
```

Boss checks the contract.

If the required version cannot be satisfied, the module path can generate an incompatibility event containing:

```text
component
current
required
message
```

A module can also produce an `error` event when one of its runtime operations fails.

This makes it possible to distinguish problems related to:

```text
Boss
Stage0
system dependencies
module runtime
module compatibility
```

---

# 15. Modules cannot enable telemetry

A module may describe a diagnostic event.

A module may **not** enable global telemetry.

The global setting belongs to Boss.

The rule is:

```text
Boss governs.
Producer describes.
External transports.
```

Conceptually:

```text
Module detects problem
        ↓
Module produces diagnostic event
        ↓
Boss telemetry gate
   ↓             ↓
 OFF             ON
  ↓               ↓
stop          continue
```

A module cannot silently turn telemetry on.

---

# 16. Where telemetry goes

When telemetry is enabled, allowed External diagnostic events may be transported to telemetry infrastructure operated by the creator or maintainer of N.E.E.B.L.E.S.

The reason for that server is not to control the user's computer.

Its purpose is to make ecosystem failures visible to the maintainer.

The intended feedback loop is:

```text
N.E.E.B.L.E.S. installation
          ↓
technical problem occurs
          ↓
error / incompatibility / stage0 diagnostic
          ↓
External telemetry
          ↓
N.E.E.B.L.E.S. telemetry infrastructure
          ↓
maintainer sees the diagnostic condition
          ↓
root cause is investigated
          ↓
hotfix is prepared
          ↓
normal N.E.E.B.L.E.S. update mechanisms distribute the correction
```

Telemetry reports.

The maintainer diagnoses.

The update system distributes the correction.

These responsibilities remain separate.

---

# 17. Example: installer nomenclature changes

Consider a fictional installer recipe that produces:

```text
sudo apt install --some-option example-package
```

At release time, APT supports:

```text
--some-option
```

Months later, APT removes or renames the option.

The current installer dictionary is now obsolete.

The next affected machine executes:

```text
Boss
 ↓
resolve apt/install
 ↓
construct arguments from instaladores_amd64.json
 ↓
execute APT
 ↓
APT reports "unknown option"
 ↓
operation fails
 ↓
External type: error
```

The telemetry infrastructure begins receiving that diagnostic condition.

The maintainer can identify:

```text
The installer recipe is obsolete.
```

A hotfix may simply update:

```text
instaladores_amd64.json
```

No redesign of Boss is required.

This is one of the principal reasons the installer dictionary and telemetry system complement each other.

---

# 18. Example: an OS upgrade creates a compatibility problem

Consider a different scenario.

The user executes:

```text
sudo apt full-upgrade
```

The command succeeds.

A library moves from:

```text
5.8
```

to:

```text
6.0
```

A N.E.E.B.L.E.S. module currently requires:

```text
>=5.0,<6.0
```

Boss checks:

```text
current: 6.0
required: >=5.0,<6.0
```

The system upgrade itself did not fail.

The installed system simply moved outside the compatibility contract known by N.E.E.B.L.E.S.

The correct diagnostic category is:

```text
incompatibility
```

The resulting hotfix may involve a module update, contract update, dependency adaptation, or Boss update depending on the real cause.

---

# 19. Example: Stage0 cannot heal the environment

Consider another situation.

N.E.E.B.L.E.S. starts.

Stage0 detects that a required dependency is missing or invalid.

It attempts recovery using the installer subsystem.

The installer executes, but after verification the dependency still does not satisfy the expected contract.

The flow becomes:

```text
Stage0 checking
      ↓
dependency missing or invalid
      ↓
local installer recipe selected
      ↓
repair attempted
      ↓
dependency verified again
      ↓
still invalid
      ↓
Stage0 cannot establish the required environment
      ↓
diagnostic telemetry
```

This tells the maintainer:

> The N.E.E.B.L.E.S. self-recovery path encountered a condition that the current release cannot solve.

That is precisely the kind of problem that may justify a rapid hotfix.

---

# 20. The three signals from the maintainer's perspective

## `error`

Meaning:

```text
N.E.E.B.L.E.S. attempted an operation and execution failed.
```

Typical question:

```text
Did a command, argument, installer, package name or external interface change?
```

## `incompatibility`

Meaning:

```text
The operation may work, but the installed component
does not satisfy the N.E.E.B.L.E.S. contract.
```

Typical question:

```text
Did Debian, a dependency or a module move to a version
that the current ecosystem does not support?
```

## `stage0`

Meaning:

```text
The N.E.E.B.L.E.S. ecosystem is in its own initialization,
preflight and recovery phase.
```

Typical question:

```text
Did an operating-system or dependency change break
the assumptions required for N.E.E.B.L.E.S. to initialize itself?
```

---

# 21. The hotfix objective

The operational objective of telemetry is to reduce the time between:

```text
external ecosystem changes
```

and:

```text
N.E.E.B.L.E.S. understands and supports the new situation
```

Without telemetry:

```text
External software changes
      ↓
N.E.E.B.L.E.S. breaks somewhere
      ↓
maintainer may not know
      ↓
manual user report
      ↓
investigation begins
```

With telemetry enabled:

```text
External software changes
      ↓
N.E.E.B.L.E.S. detects a defined failure
      ↓
structured diagnostic reaches telemetry infrastructure
      ↓
maintainer identifies the failure
      ↓
hotfix can be prepared
```

This is especially useful for installer definitions because they are intentionally externalized and replaceable.

A command-line change can often be corrected by updating a recipe instead of rewriting the orchestration architecture.

---

# 22. Privacy-safe default

Telemetry is disabled by default:

```text
telemetry.enabled = false
```

When disabled, the production External path stops before constructing the diagnostic package and message payload.

Conceptually:

```text
Event detected
      ↓
telemetry.enabled?
      ↓
    false
      ↓
stop
```

The package builder is not executed.

The message builder is not executed.

No normal production External envelope is sent to `external.sock`.

If Boss cannot read the telemetry setting, it resolves the state as disabled.

Failure therefore defaults to no transmission.

---

# 23. When telemetry is enabled

When the user explicitly enables telemetry:

```text
Event detected
      ↓
telemetry.enabled?
      ↓
     true
      ↓
build diagnostic envelope
      ↓
external.sock
      ↓
External boundary
      ↓
configured telemetry transport
      ↓
N.E.E.B.L.E.S. telemetry infrastructure
```

Enabling telemetry does not change Stage0 recovery behaviour.

It does not alter dependency resolution.

It does not change installer execution.

It only permits External diagnostic reporting to proceed.

---

# 24. `external.sock`

N.E.E.B.L.E.S. producers do not need to implement remote transport directly.

They use the local External boundary:

```text
/run/neebles/external.sock
```

Conceptually:

```text
Boss / Stage0 / module
          ↓
     External event
          ↓
    external.sock
          ↓
 External subsystem
          ↓
 configured transport
```

`external.sock` is local Unix IPC.

It is not the remote telemetry server itself.

This boundary allows the transport implementation to evolve without coupling Boss, Stage0 or modules directly to the remote protocol.

---

# 25. External envelope

The generic External envelope has the form:

```json
{
  "type": "<String>",
  "endpoint": "<String>",
  "activate": true,
  "package": {},
  "message": {}
}
```

## `type`

Identifies the event category.

Current examples:

```text
error
incompatibility
stage0
```

## `endpoint`

Identifies the destination expected by the External transport.

## `activate`

Indicates an active External envelope.

During normal production operation, disabled telemetry returns before the envelope is constructed.

## `package`

Carries producer-specific context.

A module-originated error may identify its source module here.

## `message`

Carries the structured diagnostic content specific to the event.

For example, an incompatibility may include:

```text
component
current
required
message
```

A Stage0 event may include:

```text
state
message
```

---

# 26. Persistent user control

Telemetry belongs to the normal Boss persistent settings architecture.

The key is:

```text
telemetry.enabled
```

The Boss interface exposes the control through:

```text
Telemetry Settings
```

The switch is immediate.

There is no separate Apply or Save action.

Its value persists across:

```text
Boss close/open
system restart
```

until the user changes it again.

Boss can report:

```text
Telemetry activated
```

or:

```text
Telemetry deactivated
```

when the state changes.

---

# 27. What telemetry does not do

Telemetry does not mean that the N.E.E.B.L.E.S. server remotely controls the user's computer.

Telemetry does not make the remote server part of Stage0 recovery.

Telemetry does not allow a module to bypass the Boss master switch.

Telemetry does not replace local recovery.

Telemetry does not automatically create a hotfix.

Telemetry does not automatically install a hotfix.

The responsibilities remain separate:

```text
Stage0 / Boss / modules
    detect and describe

Telemetry / External
    report

Maintainer
    diagnoses and prepares correction

N.E.E.B.L.E.S. update system
    distributes correction
```

---

# 28. Final architecture

Telemetry exists because N.E.E.B.L.E.S. cannot control the evolution of every external component it depends on.

The architecture assumes that external tools, packages, versions and command-line interfaces will eventually change.

Instead of pretending that installer recipes and compatibility contracts are permanent, N.E.E.B.L.E.S. provides a diagnostic path for discovering when those assumptions stop being true.

The complete idea is:

```text
N.E.E.B.L.E.S. owns its contracts
        ↓
external ecosystem changes
        ↓
Boss detects a defined condition
        ↓
ERROR / INCOMPATIBILITY / STAGE0
        ↓
user-enabled telemetry
        ↓
diagnostic reaches N.E.E.B.L.E.S. telemetry infrastructure
        ↓
maintainer identifies the cause
        ↓
hotfix is prepared
        ↓
normal N.E.E.B.L.E.S. update mechanisms distribute the correction
        ↓
N.E.E.B.L.E.S. adapts
```

That is the purpose of N.E.E.B.L.E.S. Telemetry.

It is not intended to observe the user.

It is intended to observe when the technical assumptions required by the N.E.E.B.L.E.S. ecosystem stop being true.
