# N.E.E.B.L.E.S. Notifications

Notifications are directed by the Boss backend. Modules report events through Boss (`neebles notify ...` or an `ExecutionRequest` targeting `boss.notifications`). Boss applies notification policy and presents notifications to Plasma through the `org.freedesktop.Notifications` D-Bus interface using zbus.

Normal notifications respect `normal_notifications`. Critical and fatal notifications are always emitted.
