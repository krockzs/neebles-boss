# N.E.E.B.L.E.S. Notifications

Notifications are directed by the Boss backend. Modules report events through Boss (`neebles notify ...` or an `ExecutionRequest` targeting `boss.notifications`). Boss applies notification policy and presents notifications to Plasma through the `org.freedesktop.Notifications` D-Bus interface using zbus.

Normal notifications respect `normal_notifications`. Critical and fatal notifications are always emitted.


## Current status — 2026-10-04

The Notifications front remains **GREEN / CLOSED** as part of the closed Boss contract. Current Tray behavior work does not change notification policy or the `org.freedesktop.Notifications` transport contract. Do not couple future Tray context-menu behavior to notification semantics; they remain separate Boss surfaces/contracts.
