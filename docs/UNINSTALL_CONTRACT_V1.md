# N.E.E.B.L.E.S. Boss — Uninstall checklist v1 (1.0.52 development)

**Status:** source-level candidate, NOT yet compiled, published or installed/certified. Based on Boss source SHA `4c729c4d72ee7dbdf8c25a1501826f557d8678e0`. The principal application remains under Lifecycle/Governor. `uninstall` is **optional** in schema-4 `manifest.json` for backward compatibility.

## Declaration

```json
{
  "uninstall": {
    "schema": 1,
    "hardcoded": {
      "session_service": "neebles-test-module-session-controller.service",
      "cache_directory": "cache"
    },
    "main": "governor",
    "extras": [
      {"id":"session", "type":"service", "provider":"systemd-user", "name":"$hardcoded.session_service", "required":false, "stop":{"disable":true,"timeout_ms":5000}, "verify":"inactive", "remove":true},
      {"id":"tray", "type":"governed", "ref":"manifest.tray", "required":false, "verify":"terminated"}
    ],
    "cleanup": [
      {"id":"cache", "type":"directory", "scope":"module-data", "path":"$hardcoded.cache_directory", "policy":"always"},
      {"id":"settings", "type":"settings", "ref":"manifest.settings", "policy":"user-choice"}
    ],
    "failure": {"unknown_process_owner":"abort", "resource_still_running":"abort", "cleanup_error":"abort"}
  }
}
```

- `hardcoded` is an arbitrary string-to-string map. **Lifecycle's existing resolver** expands `$hardcoded.key` and `$module.id` in supported Uninstall fields; `$$` is a literal dollar. This is not shell expansion, JavaScript or environment expansion.
- `main: governor` is required. Lifecycle retains its existing `hardcoded.governor.uninstall` binding; this checklist does **not** reimplement Governor.
- `extras` is a checklist of optional or required resources. Each must have a unique `id`. The **current built-in adapters** are `governed: manifest.tray` and `service: systemd-user`. The schema deliberately rejects unknown adapters; an arbitrary process pathname is NOT enough to prove process ownership. Future registered governed resources require a validated Boss adapter/identity, never a `pkill` fallback.
- `service: systemd-user`: only namespaced `neebles-<module-id>-*.service` unit names are accepted. Boss inspects a regular, desktop-user-owned unit under that user's `.config/systemd/user` and checks its `ExecStart` belongs to the installed module. Boss switches to the certified desktop UID/GID for `/usr/bin/systemctl --user`. It disables and stops the unit with bounded observation; it removes the owned file and reloads systemd **only after the governed runtime has stopped**. Missing optional units are valid; unknown active units fail closed.
- `cleanup: directory`: **only** relative normal paths beneath `/opt/neebles/shared/data/<module-id>`; symlinks and overlapping targets are rejected. Declared directories are staged and restored if subsequent uninstall operations abort before commit. Policy `always` or `user-choice` selects removal.
- `cleanup: settings`: `manifest.settings`, `user-choice` only; existing Boss settings staging and checkbox behavior are retained. Do **not** separately delete settings inside the contract executor.
- `failure`: schema 1 requires `abort`. No `ignore`, arbitrary root commands, runtime executable killing by basename, unscoped paths, or arbitrary system service control.

## Runtime teardown and Race fix

The authenticated leader of a process group can exit before its children. The 1.0.52 candidate tracks `AuthenticatedLeader`, `DrainingWithoutLeader` and `Quiescent` while waiting. The drain state only grants **read-only observation** within the existing 5-second grace window. **It never grants signal authority without the exact authenticated leader.** A persistent or uninspectable group remains an error; SIGKILL is permitted only with authenticated ownership. Regression tests exercise leader-first teardown, unowned group timeout and observation errors.

## Certification / remaining risks

1. `cargo test --locked module_uninstall::tests` and `cargo test --locked tray_shutdown_grace_regressions` on Ryzen (Rust toolchain was unavailable in the artifact-preparation environment).
2. Compile the **entire Boss**, then verify Module schema 4 with/without optional `uninstall`.
3. Verify the desktop systemd user manager and optional session controller unit on installed NEEBLES OS; no commands in this package have stopped it.
4. Exercise clean first-attempt Uninstall with Tray active; separately with preserve/delete Settings; reinstall between trials; inspect all processes, runtime marker and service unit.
5. Verify malformed/out-of-scope resources abort without killing unrelated processes.
6. **Preexisting limitation not fixed here:** Boss config cleanup uses multiple writes; a mid-cleanup I/O failure is not fully transactional. Final deletion of staged resources may likewise fail after the active module has left its installation directory. Certify/repair these before claiming a final stable release.
7. The new checklist only implements adapters **actually supported** in this candidate. Additional independent process families need governed runtime registration; merely listing their paths is not sufficient for authentication.

**Versioning:** The captured Test Module was already **1.2.7**, so its next development candidate is **1.2.8**, not 1.2.5. Boss source remains declared `1.0.51` until compilation/certification, when all official release surfaces and the build workflow can consistently move to `1.0.52`.
