# N.E.E.B.L.E.S. Boss — TXN V5 ↔ Lifecycle Arsenal bridge (development candidate)

## Why

TXN V5 currently authenticates resource ownership, records service recovery, stages reversible data, and retires service units after the irreversible commit. These guarantees must remain **inside the Boss uninstall transaction**. The old direct `module_uninstall::stop_service(...)` calls are replaced with a private FireControl/Arsenal/CapabilityRegistry dispatcher.

## Exact execution path

```text
Module Governor `uninstall` (existing public module Lifecycle transition)
  -> Uninstall TXN V5 (existing coordinator)
    -> prior-state receipt captured (unchanged)
    -> TXN-scoped private Lifecycle contract and LifecycleArsenal
      -> FireControl(artillery + objective)
      -> Rust CapabilityHandler (strict typed munition)
      -> existing authenticated systemd-user adapter
    -> reversible physical staging + integrity proof (unchanged)
    -> irreversible commit marker (unchanged)
    -> TXN-scoped Arsenal service.retire (after commit only)

On pre-commit failure:
    -> verify complete prior installation and material binding
    -> reverse-order TXN-scoped Arsenal service.reactivate
    -> report precise failures rather than silently swallowing them
```

## Implemented objectives

The first adapter exposes **`boss.uninstall.systemd-user`** to the **private transaction scope only**, with these exact objectives:

- `service.quiesce` — reversible stop, TXN retains prior-state evidence.
- `service.retire` — removal of owned user service unit, after irreversible commit only.
- `service.reactivate` — recover previously active and reauthenticated original unit after certified rollback.

Munition is strictly limited to `unit`, `exec_start`, `required`, `disable`, and `timeout_ms`. It is validated by the handler and then validated again by the existing, security-hardened TXN V5 service methods: namespacing, UID/GID, exact ExecStart, effective systemd manager command, unit file ownership/mode, absence of untrusted hooks, timeout.

**Do not** publish these destructive handlers in `productive_catalog()`: that would let any module request `service.retire` via its own ordinary Lifecycle transition. These handlers are created only inside the coordinator of an actual authorized Uninstall transaction. A module does not supply this private battleplan or its Objective.

## Extension protocol for future resource types

1. Define a new contract schema adapter with precise owned-resource identity and explicit policy.
2. Implement a bounded Rust `CapabilityHandler` for the resource using preexisting Rust crates or system APIs. No arbitrary shell/command execution.
3. Register that handler **privately** in the TXN step's `LifecycleArsenal`, using an Artillery and Objective specific to its authority.
4. Pre-capture recoverability evidence. Failure before the commit marker requires reverse-order compensations.
5. Add the resource's irreversible cleanup only to post-commit retirement, never to the reversible phase.
6. Add native Rust security and rollback tests before making the adapter public to any actual module.

`systemd-system`, service-owned sockets and timers, KDE integration, udev and firewall management are **not** implemented by this first bridge. They must not be declared as supported until their own ownership, shared-resource protection and rollback semantics are certified. Generic dispatch is not permission to destroy shared host services.

## Why this is not a separate Resource Manager

The existing LifecycleArsenal, FireControl, Rust CapabilityRegistry, Lifecycle transition execution, Governor bridge and TXN V5 coordinator are reused. The `module_uninstall_arsenal.rs` file is only the private transaction/handler adapter, not another execution engine or standalone module.

## Certification gates before merging to `main`

- `cargo fmt --all -- --check`
- `cargo test --locked module_uninstall_arsenal::tests -- --nocapture`
- `cargo test --locked module_uninstall::tests -- --nocapture`
- `cargo test --locked` and `cargo check --locked`
- Source audit: the global `build_productive_catalog()` **must not** register the destructive Artillery.
- Installed-system tests: Uninstall with active service, rollback before commit, recovery of only originally active services, successful post-commit unit removal, and unrelated systemd service isolation.

This is an implementation **candidate**, not a completed release or an installed change.
