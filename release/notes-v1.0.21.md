# N.E.E.B.L.E.S. Boss 1.0.21

Boss 1.0.21 preserves the closed Boss architecture and fixes two defects discovered during real N.E.E.B.L.E.S. OS Live acceptance.

## Fixes

- Boss client version validation now runs after the staged client has been atomically published to its permanent path, matching the domestic runtime interpreter path embedded in the backend.
- Plasma BossEvents artifacts are now published only from a CMake DESTDIR install stage instead of directly from the CMake build tree.
- Both BossEvents ELF artifacts must retain an ORIGIN-relative RPATH/RUNPATH.
- Release verification rejects BossEvents ELF artifacts that retain launcher build-tree RPATH/RUNPATH.
- Release materialization consumes an already installed and certified Launcher Plugin stage.

## Release discipline

The certified Esbirro workspace supplied by the pinned N.E.E.B.L.E.S. CUSTOM revision is the Qt/CMake build authority. The release workflow restores the sealed workspace snapshots and performs the Qt builds inside build_sysroot_6.8.2; host runner Qt/development packages are not build authority.

The release path is:

pinned CUSTOM -> verified Esbirro workspace restore -> Boss source projection -> configure/build inside build_sysroot_6.8.2 -> DESTDIR install stage -> ELF certification -> release materialization -> final payload verification.

A successful compilation alone is not certification.
