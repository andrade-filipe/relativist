# v1_stress — Frozen stress snapshot

Frozen stress snapshot for Relativist, produced by tag `v0.10.0-bench`
on 2026-04-11 on the same hardware as `v1_local_baseline`. Extends the
local baseline sizes (ep_con up to 5 M, dual_tree up to 22) to stress
sizes (ep_con up to 50 M, dual_tree up to 25) to document the
"before" state that the ROADMAP 2.22-2.26 network overhead items
will be compared against. Five of 22 Phase 2 Docker configurations
fail as expected due to the 1 GiB frame cap under bincode v1 —
documented per row in `manifest.md`.

Do not modify. See `manifest.md` for the full provenance, campaign
knobs, failure list, speedup decomposition, and checksums. See
`USAGE_GUIDE.md` section 11.7 for reproduction instructions.
