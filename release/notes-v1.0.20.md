# N.E.E.B.L.E.S. Boss 1.0.20

**Domestic Qt runtime projection and effective runtime metadata release.**

Boss 1.0.20 preserves the closed Boss architecture and incorporates the runtime corrections discovered during real N.E.E.B.L.E.S. OS Live bootstrap acceptance.

## Domestic Qt runtime projection

- Installer and authorization agent resolve the certified `boss.qt-runtime` domestic runtime path.
- `QT_PLUGIN_PATH` is projected from the certified domestic Qt plugin directory before Qt application startup.
- Host Qt plugin paths are not used as fallback authority.
- The Boss execution authority remains technology-agnostic; Qt-specific projection remains inside the Qt consumers.

## Effective privileged runtime metadata

- Boss consumes CUSTOM revision `14f6b50d109b5305ea85ddd19c0a90ce2e1bb9e5`.
- CUSTOM now certifies effective installed runtime metadata rather than relying on raw package payload modes.
- `polkit-agent-helper-1` is certified with its effective `4755` mode.
- Existing `pkexec` setuid preservation remains part of the installation contract.
- Runtime packaging verifies certified CUSTOM material before publication.

## Runtime and build provenance

Boss 1.0.20 uses:

- Rust toolchain `1.98.1`;
- controlled Qt `6.8.2` material from N.E.E.B.L.E.S. CUSTOM;
- certified CUSTOM revision `14f6b50d109b5305ea85ddd19c0a90ce2e1bb9e5`;
- certified Boss domestic runtime material from CUSTOM;
- the existing Boss authority, Lifecycle, Registry, module IPC, Settings, Surfaces, Notifications and Domestic Construction architecture.

## Published assets

- `neebles-backend`
- `neebles-ui`
- `neebles-installer`
- `neebles-auth-agent`
- `client-data.tar.gz`
- `install.sh`
- `neebles-runtime-resolve`
- `boss-runtime.tar.gz`
- `critical-update-manifest.json`
- `bootstrap.json`
- `SHA256SUMS`

## Acceptance boundary

Boss 1.0.20 contains the domestic Qt runtime projection and effective privileged metadata required by the latest Live bootstrap findings.

Full acceptance still requires the next N.E.E.B.L.E.S. OS Live run to demonstrate bootstrap, authorization, installation and permanent Boss runtime behavior using the certified domestic runtime.
