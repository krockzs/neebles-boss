# N.E.E.B.L.E.S. Boss 1.0.29

Boss 1.0.29 closes the next Fresh Live integration frontier discovered after Boss 1.0.28 successfully passed the previously failing package and RuntimeLease materialization path.

The 1.0.28 hardlink materialization closure remains intact. This release changes runtime ownership and Surface presentation wiring; it does not change CUSTOM package truth, MaterialBinding ownership or module technology semantics.

## Persistent Tray runtime ownership

- Tray Manager remains a desktop-session presentation coordinator and no longer directly owns governed Tray provider Construction or RuntimeLease creation.
- Tray provider start, stop and reconcile are requested through the persistent Boss runtime.
- Module runtime deactivation is likewise delegated to the persistent Boss process that owns runtime state.
- Persistent Boss receives the certified desktop runtime identity and Tray socket through `runtime.env`.
- Governed Construction remains module-declared and continues to use authenticated MaterialBinding + RuntimeLease material.
- Provider processes still enter the desktop session through the existing session authority and kernel peer-identity contracts.

## Persistent IPC confinement

- `tray-provider-start`, `tray-provider-stop` and `module-runtime-deactivate` validate installed module identity before runtime birth, probing or teardown.
- Invalid or escaping module identities fail closed before runtime state can be touched.
- Three dedicated dispatcher tests certify those boundaries.

## Surface consumer closure

- Boss Config no longer rejects QML array-like `QVariantList` values with JavaScript `Array.isArray()` checks.
- Nested module SurfaceContent is consumed through its actual array-like interface.
- Launcher height now follows real dynamic module content instead of clipping Surface controls below a fixed viewport.
- Launcher Surface action command construction no longer falls through JavaScript automatic semicolon insertion after `return`.
- The post-install enable execution window is increased from 5 seconds to a bounded 600 seconds so legitimate first RuntimeLease composition is not killed prematurely.

## Fresh Live evidence leading to this release

Published Boss 1.0.28 reached all of the following before the new defect surfaced:

```text
Test Module immutable source checkout       GREEN
Preinstall                                  GREEN
Essential packages                         59 / 59
module delta packages                      32 / 32
MaterialBinding schema 2                   ACTIVE
native Debian/TAR hardlink materialization GREEN
module enabled state                       TRUE
```

The remaining failure was downstream: Tray provider birth and Surface consumer presentation.

## Local source gates

```text
git diff --check                            GREEN
bash -n scripts/install.sh                  GREEN
Boss library suite                         120 / 120 GREEN
Boss main suite                            650 / 650 GREEN
persistent runtime IPC confinement tests     3 / 3 GREEN
domesticacion executed tests                13 GREEN
domestic-root certification test             1 intentionally ignored
cargo check --locked                        GREEN
```

The ignored domestic-root test remains intentionally gated on explicit domestic root certification and is not a failure.

## Product version alignment

The complete Boss product surface is aligned to 1.0.29:

- Rust backend package;
- Boss UI;
- graphical installer;
- auth agent;
- Launcher plugin and Launcher metadata/QML version;
- Spacer metadata;
- Tray Host.

## Release authorities

```text
CUSTOM classic / Esbirro   20488f6818d5e227f043425a682115f614f4bf79
CUSTOM V2                  5f43275fdbfb1107a0a15b2a77846118b3aa59bd
Test Module                b552786af35ad4aa395942ca7d6cef0458925fc9
N.E.E.B.L.E.S. OS          5c4873a0e0ad44287eefb379a04cae6260e7d2f9
OS bootstrap SHA256        97ec550475b6a4c09830380db93d5283a0d6d8dcb81b6a3f23ec0fa9c4939839
```

The release workflow retains the certified CUSTOM classic / Esbirro revision, controlled Qt 6.8.2 build authority and pinned Rust 1.98.1 release toolchain.

## Release boundary

Boss 1.0.29 publication certifies the Boss payload only. Fresh Live must now be repeated with the same existing ISO consuming published Boss 1.0.29. Installed-system acceptance remains a later independent gate.

