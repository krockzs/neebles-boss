# N.E.E.B.L.E.S. Boss v1.0.46

## Launcher/Spacer reload architecture and Tray startup

- Introduce self-reloading visual QML surfaces in preserved Plasma applet instances (Launcher/Spacer).
- Poll the installed surface payload and rebuild Loader children on stable revision changes.
- Retain existing applet IDs, panel configuration and visibility policy from Settings.
- Harden tray service startup for graphical-session availability.
- Preserve the established Boss UI Critical Update behavior.

Candidate pending Esbirro certification and installed-system testing. A follow-up v1.0.47 update will validate refresh from a source version containing the new loader architecture.
