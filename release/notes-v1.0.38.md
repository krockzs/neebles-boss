# N.E.E.B.L.E.S. Boss v1.0.38 — Receipt permissions and desktop handoff

## Changes
- The privileged installer validates ownership and safe filesystem nodes before acting on the desktop user's configuration tree.
- The dedicated `~/.config/neebles` directory is automatically secured to private mode 0700, including from a legitimate user-owned mode 0775. No manual chmod or alteration of the parent `~/.config` is required.
- Failures in receipt generation include a specific diagnostic and explicitly report that handoff was skipped.
- A genuine Boss upgrade creates the one-time receipt, launches the installed handoff helper, restarts only `plasma-plasmashell.service`, reopens Boss, and offers localized release notes. Those runtime effects require real-device acceptance.
- Preserves module release-notes modal functionality from 1.0.37; an actual verified module update is required to accept it.

## Acceptance (not implied by candidate build)
- Real 1.0.37 → 1.0.38 upgrade on Ryzen and VivoProBook: verified receipt, handoff log, Plasma service restart, new Boss UI and localized one-time modal.
- Real verified module update with authenticated release notes: separate modal without restarting Boss or Plasma.

GitHub Actions generates an audited candidate only. Publishing is a separate explicitly authorized step.
