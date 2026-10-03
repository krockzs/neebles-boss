# N.E.E.B.L.E.S. Boss 1.0.19

**pkexec setuid preservation release.**

Boss 1.0.19 preserves the closed Boss architecture and fixes the pkexec permission boundary discovered during real N.E.E.B.L.E.S. OS Live acceptance.

## pkexec setuid preservation

- The certified Boss domestic runtime requires `usr/bin/pkexec` with mode `4755`.
- The installer now verifies that the staged runtime contains `pkexec`.
- After ownership normalization, the installer restores mode 4755 on pkexec and rejects the installation if `pkexec` loses the required setuid mode.
- The packaging/install contract fixture now materializes and certifies `pkexec` as `4755`.
- The runtime resolver continues to resolve `boss.pkexec` through the certified domestic authority.
- No host pkexec fallback or alternate privilege authority is introduced.

## Runtime and build provenance

Boss 1.0.19 uses:

- Rust toolchain `1.98.1`;
- controlled Qt `6.8.2` material from N.E.E.B.L.E.S. CUSTOM;
- certified CUSTOM revision `4058a8b579c56f5598e127855d5331b6fd95b567`;
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

Boss 1.0.19 contains the pkexec setuid preservation fix required by the real Live finding.

Full acceptance still requires the next N.E.E.B.L.E.S. OS Live run to demonstrate bootstrap, authorization, installation and permanent Boss runtime behavior with the certified `4755` pkexec authority.
