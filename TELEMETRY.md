# N.E.E.B.L.E.S. Boss Telemetry

Current source line: **1.0.15**

Telemetry is an optional generic error-reporting channel.

It is not required for normal Boss operation.

When telemetry is disabled, no telemetry payload is emitted.

When enabled, Boss may produce a generic error envelope for transport through the External boundary.

Telemetry does not own application behavior, dependency resolution, lifecycle execution or recovery policy.

Transport policy remains separated from error producers.

Telemetry remains opt-in.

Boss does not require a remote collection endpoint to function.
