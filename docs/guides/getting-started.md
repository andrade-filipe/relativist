---
title: Getting started
summary: Install the relativist binary (source, Docker, or release) and learn the minimal IC vocabulary the guide trail assumes.
keywords: [getting started, install, build from source, docker, release, cargo, interaction combinators, agent, active pair, normal form, CON, DUP, ERA]
modules: [io, reduction]
specs: [SPEC-07, SPEC-12]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# Getting started

Install `relativist` and learn the minimal Interaction Combinator (IC) vocabulary
the rest of the trail assumes. For the theory in depth see
[../theory/interaction-combinators.md](../theory/interaction-combinators.md); this
guide does not duplicate it.

## install

Pick **one** option — all yield the same binary.

### build from source

Requires Rust 1.75+ (stable toolchain).

```bash
# From a published crate / git:
cargo install --git https://github.com/andrade-filipe/relativist

# Or a local checkout (this repo):
cd codigo/relativist
cargo build --release
# Binary: target/release/relativist  (target\release\relativist.exe on Windows)
```

### docker

```bash
docker pull ghcr.io/andrade-filipe/relativist
docker run --rm ghcr.io/andrade-filipe/relativist --version
```

Volume-mounted usage (mount nets in/out) is covered in
[distributed-tcp.md](distributed-tcp.md).

### release binary

Download for your platform from
<https://github.com/andrade-filipe/relativist/releases>:

- **Linux (Debian/Ubuntu):** `relativist-vX.Y.Z-x86_64.deb` → `sudo dpkg -i <file>.deb`
- **Linux (portable):** `...-x86_64-unknown-linux-gnu.tar.gz` → extract, put on `PATH`
- **Windows:** `...-x86_64-pc-windows-msvc.exe` (direct) or `.zip` (extract the `.exe`)

> **Windows SmartScreen.** The `.exe` is unsigned, so Windows may warn "Windows
> protected your PC". Right-click the `.exe` → Properties → check *Unblock* → OK;
> or click *More info* → *Run anyway*. See
> [../reference/troubleshooting.md](../reference/troubleshooting.md#windows-smartscreen).

After release, `relativist update` self-installs the latest version (see
[../reference/cli.md](../reference/cli.md#update)).

## verify the install

```bash
relativist --version      # relativist 0.9.0 or newer
relativist --help         # global help; `relativist <CMD> --help` per subcommand
```

If you built from source, run the test suite:

```bash
cargo test                # expect 690+ passing
```

## minimal IC concepts

`relativist` reduces **IC nets** (Lafont, 1997). The bare minimum to follow the
guides:

- **Agent** — a node with one *principal* port plus auxiliary ports. Three symbols:

  | Symbol  | Name        | Ports                     |
  |---------|-------------|---------------------------|
  | γ (CON) | Constructor | 1 principal + 2 auxiliary |
  | δ (DUP) | Duplicator  | 1 principal + 2 auxiliary |
  | ε (ERA) | Eraser      | 1 principal + 0 auxiliary |

- **Active pair (redex)** — two agents linked **principal-to-principal**. This is
  the only thing that reacts; the six interaction rules each rewrite one active
  pair.
- **Normal form** — a net with no active pairs left (the final result).

Strong confluence (disjoint redexes reduce in any order to the same result) is
what makes distributed reduction correct. The full symbol model, the six rules,
and the confluence argument live in
[../theory/interaction-combinators.md](../theory/interaction-combinators.md#the-six-rules).

---

**Next →** [first-reduction.md](first-reduction.md): generate a net, inspect it,
and reduce it sequentially.
