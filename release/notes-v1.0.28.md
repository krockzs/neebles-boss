# N.E.E.B.L.E.S. Boss 1.0.28

Boss 1.0.28 closes the RuntimeLease materialization defect discovered during Fresh Live acceptance of Boss 1.0.27.

The 1.0.27 architecture remains intact. This release changes the generic materialization law, not module ownership, Construction ownership or package truth.

## RuntimeLease materialization closure

- Adds generic TAR hardlink support to the canonical native `.deb` materializer.
- Hardlink targets are normalized inside the archive root and may appear before or after the link entry.
- Missing, escaping or non-regular hardlink targets fail closed.
- Real filesystem hardlinks are reconstructed inside package staging.
- Layer merge preserves hardlink inode identity instead of flattening linked files into unrelated copies.
- Existing destination parents are checked component-by-component; symlink parents and non-directory parents fail closed before publication.
- Regular, contiguous and GNU sparse file-like TAR entries remain controlled file material.
- Character devices, block devices, FIFOs, unsupported global PAX entries and unknown TAR entry types fail closed.
- Symlink targets remain opaque metadata and are never dereferenced as materialization authority.

There is no Perl-specific branch in Boss. The exact certified Debian package that exposed the missing generic semantic is:

```text
perl-base_5.40.1-6+deb13u1_amd64.deb
SHA256 b795464137a0f4d443fc9284f4b93e883fb83883cb533adf300ac660807a352a
```

Its `usr/bin/perl5.40.1` TAR entry is a hardlink to `usr/bin/perl`. The controlled 1.0.28 materializer preserves shared inode identity after extraction and after layer merge.

## Local source gates

```text
module_materialization focused suite             14 / 14 GREEN
Boss library suite                               120 / 120 GREEN
Boss main suite                                  647 / 647 GREEN
domesticacion executed tests                      13 GREEN
domestic-root certification test                   1 intentionally ignored
cargo check --locked                              GREEN
cargo build --release --locked --bins             GREEN
real certified perl-base SHA256 gate              GREEN
perl/perl5.40.1 staging hardlink identity         GREEN
perl/perl5.40.1 merged hardlink identity          GREEN
destination symlink-parent escape gate            GREEN
```

The ignored domestic-root test is intentionally gated on explicit domestic root certification and is not a failing test.

## Product version alignment

The complete Boss product surface is aligned to 1.0.28:

- Rust backend package;
- Boss UI;
- graphical installer;
- auth agent;
- Launcher plugin and Launcher metadata/QML version;
- Spacer metadata;
- Tray Host.

## Release authorities

The release retains the currently certified authority identities:

```text
CUSTOM classic / Esbirro   20488f6818d5e227f043425a682115f614f4bf79
CUSTOM V2                  5f43275fdbfb1107a0a15b2a77846118b3aa59bd
Test Module                b552786af35ad4aa395942ca7d6cef0458925fc9
N.E.E.B.L.E.S. OS          5c4873a0e0ad44287eefb379a04cae6260e7d2f9
OS bootstrap SHA256        97ec550475b6a4c09830380db93d5283a0d6d8dcb81b6a3f23ec0fa9c4939839
```

The release workflow continues to use the controlled Qt 6.8.2 / Esbirro build authority and the pinned Rust release toolchain.

## Release boundary

The 1.0.28 release pipeline certifies the published Boss payload. Fresh Live acceptance and installed-system acceptance remain separate system-level gates and must be re-run with the published Boss 1.0.28 release.
