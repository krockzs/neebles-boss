# N.E.E.B.L.E.S. Boss v2.0.6 — Git staging permissions

## Incident

- Secrets 1.0.2 commit da8c10f0fa6e05214d41099e4650856ccfd5bb43 was fetched, but Boss Update aborted before cutover with `Update staged service directory is not safe: .../integration`.
- Boss `src/module_uninstall.rs::certify_staged_update_service_sources` correctly rejects group/world-writable directories (mode & 0o022).
- `src/modules.rs::checkout_registry_commit` invokes Git in a controlled runtime but did not set a deterministic child umask; a permissive inherited mask can produce 0775 directories.

## Fix

- Enforce process-local umask 0022 in **each controlled Git boundary command**, including init, remote, fetch, checkout and verification.
- The parent process mask and existing installations are unchanged. Retain strict candidate type/mode validation.
- Add `boss206_` tests with artificially permissive inherited mask to verify child directory modes.

## Live acceptance pending

- cargo test --locked --bin neebles-backend boss206_; cargo check --locked.
- CI Esbirro Qt/Rust build and release material certification.
- Install Boss 2.0.6, then rerun native Update of Secrets 1.0.2.
- Verify service active, unchanged Master Key, unchanged vault-general KDBX and successful Unlock after update/reboot.
- No Secrets source update and no direct /opt mutation.
