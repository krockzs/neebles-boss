# N.E.E.B.L.E.S. Boss v2.0.8

## IPC execution middleware (candidate)

- Shared Boss execution policy behind the two existing sockets: `/run/neebles/neebles.sock` and `/run/neebles/modules.sock`.
- External module requests may declare `execution.async` and `execution.timeout_ms` per operation; legacy requests without metadata retain their previous behavior.
- Job IDs and authenticated status queries; execution limits and concurrency bounds.
- Explicit provider-side IPC invocation grants; `depends_install` and the `master_key=true` / `unblock=true` conditions remain independent and unchanged.
- Versioned UI, installer, Launcher, Tray, handoff, release assets and certified CUSTOM classic / Esbirro build workflow.

## Required final gates

Do not publish directly from this candidate. Check Rust, Qt 6.8.2 installed `DESTDIR` products, certified classic CUSTOM material, exact eleven assets/SHA256SUMS, governed update and installed behavior. A job timeout must never be described as a successful cancellation of a side effect. The private Secrets socket and consumer adaptations are separate follow-up work.
