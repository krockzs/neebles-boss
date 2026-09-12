# N.E.E.B.L.E.S. Boss — implementation overlay

This overlay implements the architecture discussed for the Boss core:

- Small Rust director core.
- Generic `ExecutionRequest` / `ExecutionResponse` JSON contract.
- CLI module delegation: `neebles <module> [arguments...]`.
- Centralized privilege elevation for CLI operations through automatic `sudo` re-execution.
- Module-local command privilege contracts (`requires_root`).
- Generic module dependency resolver with `apt`, `snap`, and `flatpak` providers plus `args`.
- Provider bootstrap for Snap/Flatpak when missing.
- Recursive module dependencies.
- Boss-owned language selection, first-run Linux locale detection, then independent persistent config.
- Main Qt UI: Config + Modules.
- Qt tray: quick module open/enable/disable.
- Plasma launcher package: quick access beside Plasma's normal launcher.
- Boss-routed Plasma notifications with mandatory critical/fatal policy.

## Module manifest contract

Example:

```json
{
  "schema": 1,
  "name": "recovery",
  "version": "1.0.0",
  "entrypoint": "bin/neebles-recovery",
  "commands": {
    "status": { "requires_root": false },
    "restore": { "requires_root": true }
  },
  "dependencies": {
    "system": [
      {
        "name": "rsync",
        "provider": "apt",
        "package": "rsync",
        "args": [],
        "check": "rsync",
        "required": true
      }
    ],
    "modules": []
  }
}
```

## Execution request contract

```json
{
  "target": "recovery",
  "action": "restore",
  "args": ["--full", "backup.neebles"],
  "context": { "caller": "cli" }
}
```

The module owns the meaning of `action` and `args`. Boss only directs execution, privileges, dependencies, state, language, and result routing.
