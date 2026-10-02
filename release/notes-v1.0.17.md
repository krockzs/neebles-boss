# N.E.E.B.L.E.S. Boss 1.0.17

**Bootstrap Domestic Runtime bridge release.**

Boss 1.0.17 preserves the closed Boss architecture established in 1.0.15 and fixes the bootstrap execution boundary discovered during real N.E.E.B.L.E.S. OS Live acceptance.

## Bootstrap domestic execution

- Bootstrap installer execution now crosses the certified temporary domestic runtime authority instead of attempting to execute a release-domesticated ELF directly from the host.
- The authorization agent uses the same temporary runtime authority during bootstrap.
- `neebles-runtime-resolve` supports the controlled external-bootstrap execution path required before the permanent Boss runtime exists.
- Canonical installed PT_INTERP paths are relocated through the temporary runtime root during bootstrap.
- External bootstrap execution is restricted to `domestic-runtime.json` beneath the bootstrap `runtime-authority` staging.
- The external ELF must be a sibling inside the same bootstrap staging root.
- Permanent Boss runtime authority cannot use the external-bootstrap execution mode.
- No host runtime fallback, alternate authority, Qt world or second execution engine is introduced.

## Nested bootstrap argument boundary

- External bootstrap execution treats the first argument separator as the resolver control boundary.
- Additional separators after that boundary are preserved as opaque child-process arguments.
- This fixes the installer to auth-agent to Polkit bootstrap chain discovered during real Live acceptance.
- Regression coverage verifies missing, single and nested child separators.

## Authority and security boundary

The bootstrap bridge remains a temporary exception over the existing domestic authority machinery. After installation, Boss continues to use the permanent domestic runtime contract.

The final release gate certified:

- temporary bootstrap authority execution: GREEN;
- external ELF outside bootstrap staging rejection: GREEN;
- permanent runtime external-bootstrap rejection: GREEN;
- domestic runtime transport certification: GREEN;
- Cargo format/check and Rust library suite: GREEN;
- Boss diff validation: GREEN;
- OS bootstrap syntax and diff validation: GREEN.

## Runtime and build provenance

Boss 1.0.17 continues to use:

- Rust toolchain `1.98.1`;
- controlled Qt `6.8.2` material from N.E.E.B.L.E.S. CUSTOM;
- certified Boss domestic runtime material from CUSTOM;
- the existing generic Boss authority, Lifecycle, Registry, module IPC, Settings, Surfaces, Notifications and Domestic Construction architecture.

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

Boss 1.0.17 contains the bootstrap/runtime fixes required by the real Live findings from 1.0.15.

Full acceptance still requires the next N.E.E.B.L.E.S. OS Live run to demonstrate the complete bootstrap, installer, authorization-agent, installation and permanent-runtime path.
