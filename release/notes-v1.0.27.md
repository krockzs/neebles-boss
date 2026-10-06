# N.E.E.B.L.E.S. Boss 1.0.27

Boss 1.0.27 closes the current source line after the Point 1 / Point 2 module-material work, the CUSTOM raw-transport correction discovered during Fresh Live, and the Config / Features integration.

## CUSTOM raw-material transport

This release hardens immutable CUSTOM raw transport while preserving package membership, SHA256 integrity, Essential/module-delta ownership and MaterialBinding semantics.

- raw URLs are constructed with segment-aware `url::Url` semantics;
- CUSTOM revisions are validated as exact 40-character hexadecimal commits;
- empty, current-directory, parent-directory and control-character path segments are rejected;
- literal `%xx`, spaces, Unicode, backslashes and reserved URL characters remain filename data rather than URL structure;
- encoded slash/traversal and query/fragment reinterpretation are rejected fail-closed.

Transport certification previously completed for this source line includes:

- 91 / 91 declared Essential + Test Module package URLs resolving from immutable CUSTOM revision;
- all 8 literal `%3a` package payloads downloading with exact declared SHA256;
- old-path HTTP 404 versus corrected-path HTTP 200 control probe;
- 94 printable ASCII filename-character cases GREEN;
- 512 literal `%00` through `%FF` upper/lower cases GREEN;
- 33 ASCII controls rejected fail-closed;
- 151,739 sensitive-token single/pair/triple combinations round-trip exactly;
- 16 / 16 directed URL transport tests GREEN.

## Config / Features source closure

Boss 1.0.27 additionally carries the closed Config / Features architecture:

- Surface schema 2 typed `require` contracts;
- strict `active` and `open` requirement resolution;
- persistent Surface presentation model with backend requirement revalidation;
- persistent Surface action routing by owner module + item id + action;
- module-scoped Surface translations;
- dynamic Config -> Features materialization and reconciliation;
- Modules retained as the administrative surface rather than feature configuration;
- bounded Launcher and Tray presentation;
- generic Lifecycle -> Module IPC execution;
- governed Test Module notification button and stateful notification switch;
- switch state commits only after successful Lifecycle transition;
- Test Module Tray SurfaceContent remains module-declared and Boss-governed.

## Product version alignment

The complete Boss product surface is aligned to 1.0.27:

- Rust backend package;
- Boss UI;
- Installer;
- authorization agent;
- Launcher QML + metadata;
- Plasma Launcher plugin;
- Spacer metadata;
- Tray Host.

Protocol and ABI identities are intentionally independent and are not rewritten merely because the product release version advances.

## Controlled build authority

Boss publication remains governed by the release workflow and Esbirro construction law.

- Rust release toolchain: `1.98.1`;
- Rust release build: `cargo build --release --locked --bins`;
- static bootstrap resolver: musl target;
- Boss Qt consumers are built from projected controlled Boss source inside the restored Qt 6.8.2 Esbirro sysroot;
- Qt outputs pass configure/build -> DESTDIR install/stage -> installed-artifact audit -> release materialization -> final release verification;
- a host build-tree executable is not release authority.

Certified CUSTOM classic / Esbirro build revision:

`20488f6818d5e227f043425a682115f614f4bf79`

Current CUSTOM V2 module-material authority at release preparation:

`5f43275fdbfb1107a0a15b2a77846118b3aa59bd`

Final Test Module source revision:

`b552786af35ad4aa395942ca7d6cef0458925fc9`

Verified N.E.E.B.L.E.S. OS integration source:

`5c4873a0e0ad44287eefb379a04cae6260e7d2f9`

Verified N.E.E.B.L.E.S. OS bootstrap SHA256:

`97ec550475b6a4c09830380db93d5283a0d6d8dcb81b6a3f23ec0fa9c4939839`

## Validation boundary

The latest closed backend source baseline is 647 / 647 tests GREEN with cargo check GREEN.

Release publication still requires the controlled release workflow to restore the sealed Esbirro workspace, build the release artifacts, certify staged Qt/ELF/runtime material, materialize the exact payload, verify the final asset set and publish the resulting release.

Fresh Live and installed-system acceptance remain separate system-level gates and must be re-run with the published Boss 1.0.27 release.
