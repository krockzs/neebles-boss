# N.E.E.B.L.E.S. Boss v1.0.39 — Isolated desktop upgrade handoff

- Launch post-upgrade handoff through a separate transient `systemd --user` service, outside the retiring Boss UI cgroup.
- Fail closed if the helper is not executing in its expected dedicated service.
- Preserve authenticated upgrade receipt, verified old-UI D-Bus identity and pidfd termination, Plasma-only restart and automatic Boss relaunch.
- Add durable transition logging for diagnosing real-device behavior.
- Preserve the existing Boss and module release-notes dialog functionality.

The CI candidate alone cannot certify the on-device behavior. Validate a real 1.0.38 → 1.0.39 upgrade before marking desktop handoff accepted.
