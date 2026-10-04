# N.E.E.B.L.E.S. Boss 1.0.22

Boss 1.0.22 preserves the closed Boss architecture and carries the Tray behavior corrections prepared after real N.E.E.B.L.E.S. OS Live acceptance.

## Fixes

- The Tray popup hides when its Qt window loses active state, allowing click-outside dismissal without changing the certified popup geometry.
- StatusNotifierItem secondary activation no longer executes the primary Tray action.
- StatusNotifierItem ContextMenu no longer executes the primary Tray action.
- The final release verifier certifies the domesticated Tray Host ELF from the published client payload.
- N.E.E.B.L.E.S. CUSTOM now resolves its canonical branch as main after the repository branch normalization.

## Release discipline

The certified Esbirro workspace supplied by the pinned N.E.E.B.L.E.S. CUSTOM revision remains the Qt/CMake build authority. Host Qt/development packages are not build authority.

The release path remains:

pinned CUSTOM -> verified Esbirro workspace restore -> Boss source projection -> controlled build -> DESTDIR install stage -> ELF certification -> release materialization -> final payload verification.

A successful compilation alone is not certification.
