# Security Policy

## Scope and context

Relativist is a research artifact from an academic thesis (TCC). It is **not
hardened for hostile or open-internet deployment** and ships with that caveat:
plain TCP by default, no per-session strong auth, no reconnect tolerance (see
`docs/ROADMAP.md` §2.21 / SPEC-24, which scope WAN hardening as future work). If
you run a coordinator/worker grid, run it on a trusted network (LAN, VPN, or
`localhost`), not on the public internet.

That said, correctness and memory-safety bugs in the reducer, protocol, or CLI
are real and we want to know about them.

## Supported versions

Active development happens on `v2-development`. Security fixes target the latest
development line; there is no long-term-support branch. The frozen `v1` tag
(`v0.10.0-bench`) is an archival research snapshot and will not receive fixes.

| Version / branch        | Supported           |
|-------------------------|---------------------|
| `v2-development` (HEAD)  | ✅ yes               |
| `main`                  | ✅ yes (integration) |
| `v1-feature-complete`   | ❌ frozen archive    |

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.**

Use either channel:

1. **GitHub private vulnerability reporting** — the preferred path. On the repo,
   go to **Security → Report a vulnerability** (GitHub Security Advisories). This
   keeps the report private until a fix is ready.
2. **Email** — **filipeandrade.work@gmail.com** with subject `[SECURITY]
   relativist`.

Please include:

- a description of the issue and its impact,
- steps to reproduce (a minimal `.bin`/`.ic` net or command sequence is ideal),
- affected version/commit, and
- any suggested remediation.

## What to expect

As a solo-maintained academic project, response is best-effort:

- **Acknowledgement:** within ~7 days.
- **Assessment & fix plan:** communicated once the report is triaged.
- **Disclosure:** coordinated — we will agree on a timeline before any public
  disclosure, and credit you in the advisory/changelog unless you prefer to
  remain anonymous.

## Out of scope

- Denial of service from intentionally pathological/non-terminating nets — the
  model is defined for **terminating** nets (premise P6); resource exhaustion on
  adversarial input is a known boundary, not a vulnerability.
- Anything requiring a deployment posture the project explicitly does not claim
  to support (e.g. exposing a worker directly to the public internet).
