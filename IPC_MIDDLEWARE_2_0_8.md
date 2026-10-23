# N.E.E.B.L.E.S. Boss 2.0.8 — IPC Execution Middleware (CANDIDATE)

## Architecture

The administrative socket `/run/neebles/neebles.sock` retains its JSON/EOF protocol, while authenticated module sessions remain on `/run/neebles/modules.sock` with 4-byte length-prefixed JSON. Both use `src/ipc_middleware.rs` for execution policy, bounded worker concurrency, job IDs and status retention.

The execution contract is optional and per request:

```json
"execution": {"async": false, "timeout_ms": 12000}
```

- `async: false` (finite): await response until `timeout_ms`.
- `async: true`: acknowledge immediately with a `job_id`, then query status.
- Default if `execution` is omitted: **unchanged legacy synchronous behavior**. This does not retroactively change existing CLI/QT timeouts or older module messages.
- `NEEBLES_IPC_MAX_TIMEOUT_MS`: limit per explicitly parameterized request, default `300000`, valid setting 1–3600000. These are POLICY limits, not consumer-specific hardcoded delays.
- `NEEBLES_IPC_MAX_ACTIVE_JOBS`: max concurrent bounded/async jobs, default 32; valid 1–256.
- `NEEBLES_IPC_MAX_RETAINED_JOBS`: max retained job IDs/results, default 256; valid 1–4096.
- `NEEBLES_IPC_MAX_OWNER_JOBS`: simultaneous jobs per authenticated owner, default 8; valid 1–64.
- `NEEBLES_IPC_MAX_JOB_RESULT_BYTES`: limit for retained job results, default 1048576; valid 1–16777216. Oversized asynchronous results are replaced with an explicit error rather than retained in memory.
- `NEEBLES_IPC_MAX_INFLIGHT_REQUESTS`: max concurrent module request envelopes, default 64; valid 1–256.

Completed jobs remain discoverable for 10 minutes unless Boss restarts. A terminal result is collected **once** on status query. These jobs live only in Boss memory; they do not survive a Boss restart. Timeout signals the end of the **wait**, not a safe cancellation of an already-started side effect. A worker finishing after the deadline reports `completed_late`; a still-running job beyond its deadline reports `deadline_exceeded_running`. Callers must not blindly retry non-idempotent operations.

### Administrative neebles.sock

```json
{"target":"boss","action":"version","execution":{"async":true,"timeout_ms":30000}}
```

The target/action syntax is whatever installed Boss already recognizes; `version` is illustrative and must be replaced with a real action. To query an actual job ID:

```json
{"target":"ipc.middleware","action":"status","args":["job-UUID"]}
```

Existing leases/subscriptions remain streaming operations, not jobs. Boss's internal Qt defaults are centralized in `ui/client/bosscontroller.cpp`; they retain the old values until intentionally changed. The *module* invocation timeout is forwarded from the administrative execution options to the existing router.

### Authenticated modules.sock

Runtime register/publisher authentication is unchanged, including SO_PEERCRED, installed executable identity and session ownership. New **additive protocol-v1** messages:

```json
{"type":"execute","id":"call-1","module":"neebles-connect","session_id":"authenticated-session","target":"neebles-secrets","contract":"commands","endpoint":"example","args":[],"payload":null,"context":{},"execution":{"async":true,"timeout_ms":300000}}
```

Boss returns `execute_reply` with a `response` object: on async acceptance `{"job_id":"job-...","state":"running"}`, on finite completion the existing `ModuleMessage::Response` represented as JSON, or structured rejection. Later:

```json
{"type":"job_query","id":"poll-1","module":"neebles-connect","session_id":"authenticated-session","job_id":"job-UUID"}
```

Boss returns `job_snapshot`. Runtime session identity is mandatory and job status is scoped to it; unrelated modules cannot fetch results. **Secrets and Connect have not been adapted**; this is the Boss-side protocol only.

### Permission model

`depends_install` is **not** an IPC grant. A provider MUST grant specific logical endpoints in its authenticated installed requirements snapshot, projected from the corresponding Boss Registry selection when the provider is installed/updated:

```json
"ipc_invoke_allow": {"neebles-connect": ["commands:example"]}
```

Legacy installed providers have no such grant (empty by default). Cross-module calls are therefore denied until an explicitly governed provider release supplies one. The existing contract resolver still enforces installed declarative endpoint existence, runtime advertisement and privilege requirements. The caller's `context.caller` and reserved origin identifiers cannot be trusted and are overwritten by Boss for module requests.

For new consumers, adding different `async` and `timeout_ms` values **does not require recompiling Boss**. To give a consumer NEW access to a protected provider endpoint, an authorized Registry/provider update IS required; that is intentional least privilege, not an arbitrary timeout restriction.

### Out of scope of the Boss candidate

Secrets's private socket and KeePassXC operations are NOT intercepted or changed. Native module API integration, cancellation propagated into provider code, UI job controls, durable retry after Boss restart, and Secrets/Connect adaptations are separate controlled phases.

### Mandatory certification before release

1. Static source hash check on Ryzen base `73019e0` (no host changes overwritten).
2. `cargo check --locked --all-targets` and `cargo test --locked` with the controlled Rust 1.98.1 toolchain; differentiate pre-existing rustfmt failures from new ones.
3. Test legacy admin and runtime IPC, rejects for cross-module calls without grants, valid installed-grant routing, multiple finite+async jobs, job isolation and timeout without side-effect cancellation assumptions.
4. Build Rust/Qt via certified CUSTOM classic/Esbirro and verify component hashes for the final release.
5. Update/publish only through approved Boss process; no manual edits in `/opt`, no vault removal, no running module changes during QA until explicit approval.

## Contract protection — Secrets runtime variables (F02)

The Boss Registry entry `neebles-connect.depends_install.neebles-secrets.variables`
requires **both** `master_key = "true"` and `unblock = "true"`. This is an
existing governor contract, not an IPC timeout or invocation permission.
It MUST NOT be removed, renamed, replaced by presence-only, or conflated
with `ipc_invoke_allow`. The registry test asserts both values while leaving
room for future independent variables; the test no longer requires zero.

The consumer's subsequent runtime behavior after Secrets locks is a separate
state-governance decision. This F02 change does not modify the Registry or
any runtime publication code.
