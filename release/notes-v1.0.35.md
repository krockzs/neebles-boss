# N.E.E.B.L.E.S. Boss v1.0.35 — X11 compatibility and Critical Update validation candidate

## Scope

- Tray Host: add X11 (xcb) popup placement using current cursor screen and available screen geometry, preserving the existing Wayland LayerShellQt path.
- Plasma Launcher: disable mipmapping of the 74x74 mascot image to correct its observed X11 rendering.
- Keep the previously integrated Multi-RootFS architecture and packaging contracts.
- Synchronize version 1.0.35 across Rust, Qt clients, Installer, Auth Agent, Tray Host, Launcher plugin and Plasma metadata.

## Verification status before release

- The Tray Host source patch compiled successfully under the controlled Qt 6.8.2 sysroot and was visually verified on an installed Ryzen system running X11.
- Wayland regression testing of this new source revision is not yet evidenced.
- The installed Launcher with `mipmap: false` was visually verified; the installed QML used absolute icon paths during the experiment, while the portable repository QML retains relative paths. Exact-source launcher runtime verification remains pending.
- The 1.0.35 workflow candidate, release assets, new installation, and in-place Critical Update are NOT yet certified.

## Critical Update acceptance test

- Leave the Ryzen's installed Boss 1.0.34 unchanged until v1.0.35 is certified and published.
- Confirm automatic version detection at UI startup and/or at the five-minute polling interval.
- Initiate Critical Update from the application and validate authorization, asset checksums, installation, post-update services, UI and installed version.
- Prepare and verify a recovery procedure before executing the update.

## Publication policy

The release workflow produces a candidate artifact; it does not automatically publish a GitHub Release. Do not create or modify `release/trigger-v1.0.35` before candidate preflight approval. Publishing the release is a separate, explicitly authorized step after candidate certification.
