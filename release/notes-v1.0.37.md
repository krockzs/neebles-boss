# N.E.E.B.L.E.S. Boss v1.0.37 — Post-update handoff and module release-notes correction

## Scope
- Install and validate the authenticated post-update handoff helper from packaged client-data.
- Maintain the existing verified upgrade receipt and handoff barrier, then restart only `plasma-plasmashell.service` and launch the new Boss UI.
- Display the one-time Boss release-notes modal after a genuine installed-version transition.
- Display a separate module update notes dialog only after a verified module upgrade with authenticated non-empty notes. Initial installs do not show notes.
- Preserve pinned CUSTOM Esbirro and runtime material.

## Acceptance pending
- Real Boss 1.0.36 → 1.0.37 Critical Update: installed version, receipt, handoff log, scoped Plasma restart and one-time notes modal.
- Real module 1.2.3 → 1.2.4 update: notes must be included in the module payload; verify the popup, with no Boss or Plasma restart.
- Do not claim runtime acceptance from build success alone.

## Publication
GitHub Actions produces only the candidate artifact. Publish a release only after candidate verification and approval.
