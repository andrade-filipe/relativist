---
title: File formats
summary: Reference for Relativist net file formats — .bin (bincode v2), .ic text DSL, and .json — plus load/save APIs and wire-encoding notes.
keywords: [file format, .bin, .ic, .json, bincode, text DSL, serialization, net]
modules: [io, net]
specs: [SPEC-12, SPEC-02, SPEC-18]
audience: [user, contributor, llm]
status: reference
updated: 2026-06-26
---

# File formats

Relativist persists Interaction Combinator nets (`relativist_core::net::Net`) in
three on-disk formats, dispatched by file extension. `.bin` and `.ic` carry the
same information — the trade-off is size/speed versus human-readability. `.json`
is recognized but **not yet implemented** (load/save return a descriptive error).

Source of truth: `relativist-core/src/io/` (`binary.rs`, `text_dsl.rs`,
`types.rs`, `mod.rs`). Requirements: `docs/specs/SPEC-12-user-io.md`.

## format-overview

| Extension | Format | Module | Status | Use for |
|-----------|--------|--------|--------|---------|
| `.bin` | Binary, bincode v2 (varint, little-endian) | `io/binary.rs` | supported | benchmarks, reduction, TCP transport, large nets |
| `.ic` | Human-readable text DSL | `io/text_dsl.rs` | supported | debugging, tests, didactic examples |
| `.json` | JSON | — | **not yet supported** | reserved (returns error) |

Format detection is by extension only (`io::types::detect_format`); any other
extension yields `"unknown file extension"`.

## load-and-save-api

The format-dispatching entry points live in `io/mod.rs`:

```rust
use relativist_core::io::{load_net_from_file, save_net_to_file};

let net = load_net_from_file(Path::new("net.bin"))?; // detects .bin/.ic/.json
save_net_to_file(&net, Path::new("net.ic"))?;        // format from extension
```

Per-format helpers (re-exported from `io`):

| Format | Load | Save | In-memory bytes |
|--------|------|------|-----------------|
| `.bin` | `load_bin(path)` | `save_bin(&net, path)` | `serialize_net` / `deserialize_net` |
| `.ic` | `load_ic(path)` | `save_ic(&net, path)` | `parse_ic(&str)` / `format_ic(&net)` |

`load_net_from_file` / `save_net_to_file` on a `.json` path return
`RelativistError::Config("JSON format not yet supported; use .bin or .ic")`.

## bin-binary-bincode-v2

Compact binary form of a serialized `Net`. Smallest footprint and O(N) reads;
not inspectable with `cat`.

- **Codec:** bincode v2 via `protocol::bincode_v2` = `bincode::config::standard()`:
  little-endian byte order, **varint** integer encoding, no embedded schema.
- **Payload:** the full dense `Net` (`agents`, `ports`, `redex_queue`,
  `next_id`, `root`, `free_list`). `freeport_redirects` is `#[serde(skip)]`.
- **Wire compatibility:** `.bin` shares the bincode v2 codec with the TCP wire
  format (SPEC-18). See [protocol-version-compatibility](#protocol-version-compatibility);
  files written by a newer schema are not readable by older binaries.

```bash
relativist generate ep-annihilation -n 10000 -o net.bin
```

## ic-text-dsl

Line-oriented, human-readable DSL. Verified against the parser in
`io/text_dsl.rs`.

### ic-grammar

```
file        ::= line*
line        ::= comment | agent_decl | wire_decl | root_decl | blank
comment     ::= '#' ...                       ; whole-line, after trim
agent_decl  ::= 'agent' IDENT SYMBOL
wire_decl   ::= 'wire' port_ref port_ref
root_decl   ::= 'root' port_ref               ; at most one
port_ref    ::= IDENT '.' PORT_NAME | 'free(' INT ')'
PORT_NAME   ::= 'principal' | 'left' | 'right' | 'p0' | 'p1' | 'p2'
SYMBOL      ::= 'CON' | 'DUP' | 'ERA'
```

- **Tokenizing:** whitespace-split, leading/trailing whitespace trimmed; blank
  lines and `#`-prefixed lines ignored.
- **`IDENT`:** arbitrary agent name (e.g. `a0`, `x`), unique per net. The
  serializer emits names as `a<id>` (the live agent's numeric id).
- **`SYMBOL`:** `CON` (arity 2), `DUP` (arity 2), `ERA` (arity 0).
- **`PORT_NAME`:** port 0 = `principal`/`p0`; port 1 = `left`/`p1`;
  port 2 = `right`/`p2`. The aliases `p0`/`p1`/`p2` and the named forms are
  interchangeable on input; the serializer always writes `principal`/`left`/`right`.
- **Free ports:** `free(N)` — **parentheses**, integer N ≥ 0 (not `free<N>`).

### ic-parsing-rules

Parsing is two-pass: pass 1 collects `agent` declarations, pass 2 resolves
`wire`/`root` references. Validation errors (with line numbers):

- duplicate agent name; unknown symbol; `agent` without exactly name+symbol.
- `wire` reference to an unknown agent or unknown port name.
- ERA agent with an auxiliary port (`port > 0`) — ERA has no aux ports (R9).
- self-loop wire — a port wired to itself (R58).
- free-to-free wire — at least one endpoint must be an agent port (R59).
- more than one `root` declaration (R54). With no `root`, `net.root` is `None`.

### ic-example

CON-CON annihilation (one redex, principal-to-principal):

```
# CON-CON annihilation
agent a CON
agent b CON
wire a.principal b.principal
wire a.left  free(0)
wire a.right free(1)
wire b.left  free(2)
wire b.right free(3)
root a.principal
```

```bash
relativist generate ep-annihilation -n 3 -o net.ic
cat net.ic
```

## json

`.json` is a recognized `NetFormat` (extension detection succeeds) but neither
load nor save is implemented. Both paths return a descriptive
`RelativistError::Config`. Use `.bin` or `.ic`. (Note: metrics output via
`io::write_metrics` does emit JSON/CSV, but that is `GridMetrics`, not a `Net`.)

## compactsubnet-wire-encoding

`.bin` on disk always uses the full dense `Net`. The TCP path uses a compressed
wire form, `CompactSubnet` (`partition/compact.rs`), to keep partitions under the
256 MiB protocol frame cap (the **L6 mitigation**). Under `ContiguousIdStrategy`
the last worker's subnet spans the full `max_id + 1` range even when few agents
are live; the dense `Vec<Option<Agent>>` / `Vec<PortRef>` layout would blow the
cap. `CompactSubnet` serializes only live agents and non-`DISCONNECTED` ports
plus the arena sizes needed to rebuild the dense layout on the receiver, applied
via serde `serialize_with`/`deserialize_with` on `Partition::subnet` — in memory
the subnet stays a `Net`; only the wire bytes are compacted.

Round-trip invariant: `Net -> CompactSubnet -> Net` preserves `agents`, `ports`,
`redex_queue`, `next_id`, and `root` byte-for-byte. The `free_list` (recycled-id
ledger, SPEC-22) is carried verbatim and integrity-checked on inflate
(`validate_free_list`); its omission previously caused `next_id` divergence
between coordinator and worker (QA-D009-001), which the v7 suffix fixes.

## protocol-version-compatibility

`PROTOCOL_VERSION` (`protocol::coordinator`, currently **7**) governs the
coordinator/worker handshake (SPEC-06). Peers must match: a lower
`Register.protocol_version` is rejected with
`RegisterNack { reason: ProtocolVersionMismatch }`. Each schema-touching change
bumps the version, and the predecessor rejects newer payloads
(`UnsupportedVersion`) — no silent reinterpretation.

| Version | Spec driver | Change |
|---------|-------------|--------|
| 1 | SPEC-06 v1 | Initial wire: bincode v1, frame header v1, no compression. |
| 2 | SPEC-18 | Wire format v2: bincode v2 (varint), compact `PortRef`, LZ4 threshold (>1 MB), frame header v2. Intentional break. |
| 3 | SPEC-18 R28 | Amendment to wire format v2. |
| 4 | SPEC-20 | Elastic-grid fields added to wire messages (TASK-0417). |
| 5 | SPEC-22 §3.1 R9a | `Net.free_list: Vec<AgentId>` added to serialized layout (D-009). v4 nets lack it → `UnsupportedVersion`. |
| 6 | SPEC-21 §3.7 R37c | Streaming pull-dispatch: `RequestWork` + `NoMoreWork` `Message` variants. |
| 7 | SPEC-19 §3.4 R35a | `CompactSubnet` wire encoding gains a `free_list` suffix (D-011, commit `c4c80b8`). REJECT-v6 policy. |

Persisted `.bin` files from frozen baselines (e.g.
`reproduce_article/results/locked/v1_local_baseline/*.bin`) become unreadable by
newer binaries; this is acceptable because frozen baselines do not feed later
code — regenerate via `relativist generate` to get the current schema.

## conversion

`relativist generate ... -o <name>.<ext>` picks the format from the suffix. Core
subcommands (`reduce`, `local`, `inspect`, `coordinator`, `worker`) accept either
`.bin` or `.ic` as input:

```bash
relativist generate ep-annihilation -n 5 -o net.ic
relativist reduce -i net.ic -o net_reduced.bin
```

## references

- Binary format: `relativist-core/src/io/binary.rs`; codec `protocol/bincode_v2.rs`
- Text DSL: `relativist-core/src/io/text_dsl.rs`
- Format detection / summary: `relativist-core/src/io/types.rs`
- Dispatch + metrics output: `relativist-core/src/io/mod.rs`
- Compact wire form (L6): `relativist-core/src/partition/compact.rs`
- Requirements: `docs/specs/SPEC-12-user-io.md`; net model: `SPEC-02`;
  wire format v2: `SPEC-18`
