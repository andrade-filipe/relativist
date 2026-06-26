---
title: CLI reference
summary: Authoritative reference for the relativist binary — all 13 subcommands, their flags, and minimal example invocations, authored from config.rs.
keywords: [cli, command, flag, subcommand, coordinator, worker, local, reduce, inspect, generate, compute, bench, validate, update, completions, encoders, codecs, decode, transport, streaming, delta-mode, elastic, tls, token]
modules: [config, commands]
specs: [SPEC-07, SPEC-12, SPEC-13]
audience: [user, contributor, llm]
status: reference
updated: 2026-06-26
---

# CLI reference

Authoritative list of every subcommand and flag of the `relativist` binary,
authored from `relativist-core/src/config.rs` (clap definitions) and
`relativist-core/src/commands.rs` (behavior). For end-to-end tutorials see the
guides; this page is a reference, not a tutorial.

```
relativist --version          # version
relativist --help             # global help
relativist <CMD> --help       # per-subcommand help
```

Exit codes (SPEC-13 R17): `0` success, `1` config error, `2` comms error,
`3` internal error.

## subcommands

| Subcommand            | Purpose                                                              |
|-----------------------|---------------------------------------------------------------------|
| [`coordinator`](#coordinator) | Run as TCP coordinator: load net, partition, distribute, merge.     |
| [`worker`](#worker)           | Run as worker: connect to a coordinator, reduce assigned partitions.|
| [`local`](#local)             | In-process grid simulation (N workers, no TCP).                     |
| [`reduce`](#reduce)           | Purely local sequential reduction (`reduce_all`), no partitioning.  |
| [`inspect`](#inspect)         | Print summary statistics of an IC net file.                        |
| [`generate`](#generate)       | Generate a workload net and save to file.                          |
| [`compute`](#compute)         | Encode arithmetic / registry codec, reduce, decode result.         |
| [`bench`](#bench)             | Run the benchmark suite (SPEC-09).                                 |
| [`validate`](#validate)       | Validate benchmark campaign CSV outputs.                           |
| [`update`](#update)           | Check for and install the latest release.                          |
| [`completions`](#completions) | Generate shell completion scripts.                                 |
| [`encoders`](#encoders) (`codecs`) | List registered encoders/codecs.                              |
| [`decode`](#decode)           | Decode a bincode `.bin` (reduced net) back to codec JSON.          |

Shared option: `--log-format <text|json>` is accepted by `coordinator`,
`worker`, and `local` (default: auto-detect from TTY).

## coordinator

Run as TCP coordinator: load a net, partition it, distribute to workers, and
merge the reduced result.

```bash
relativist coordinator -w 4 -i net.bin -b 0.0.0.0:9000 -o reduced.bin
```

Core:

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --workers <N>` | required (`>=1`) | Workers to wait for before starting. |
| `-i, --input <PATH>` | required | Input net file (`.bin`, bincode `Net`). |
| `-b, --bind <ADDR>` | `127.0.0.1:9000` | Bind address; accepts `IP:PORT`, `HOST:PORT`, or `tailscale[:PORT]` (auto-resolves Tailscale IPv4). |
| `-o, --output <PATH>` | none | Write the reduced net (`.bin`). |
| `-m, --metrics <PATH>` | none | Write execution metrics (`.json` or `.csv`). |
| `--strategy <NAME>` | `round-robin` | Partitioning strategy (only `round-robin` in v1). |
| `--max-rounds <N>` | unlimited | Max grid rounds. |
| `--strict-bsp` | `false` | Strict BSP mode (border cascades not reduced at coordinator). |
| `--log-format <text\|json>` | TTY auto | Log output format. |

Security (SPEC-10):

| Flag | Default | Description |
|------|---------|-------------|
| `--token <auto\|BASE64>` | none | `auto` to generate, or a base64 token. |
| `--token-file <PATH>` | `./relativist-token` | Where to write the generated token. |
| `--tls-cert <PATH>` | none | TLS cert (PEM); requires `--tls-key`. Feature `tls` only. |
| `--tls-key <PATH>` | none | TLS key (PEM); requires `--tls-cert`. Feature `tls` only. |

Transport (SPEC-17/18):

| Flag | Default | Description |
|------|---------|-------------|
| `--transport <tcp\|unix>` | `tcp` | Transport backend (`unix` = UDS, Unix only). |
| `--socket-path <PATH>` | none | UDS path (only with `--transport unix`). |
| `--no-tcp-nodelay` | off | Disable TCP_NODELAY (enable Nagle). NODELAY is on by default. |
| `--send-buffer <BYTES>` | `4194304` | SO_SNDBUF. |
| `--recv-buffer <BYTES>` | `4194304` | SO_RCVBUF. |
| `--keepalive <SECS>` | `30` | TCP keepalive idle; `0` disables. |
| `--compression-threshold <BYTES>` | `1024` | LZ4 frame threshold; `0` compresses every frame. |
| `--use-zero-copy` | `false` | Request rkyv archive on hot-path msgs (effective only with `--features zero-copy`). |

Delta + streaming (SPEC-19/21):

| Flag | Default | Description |
|------|---------|-------------|
| `--delta-mode` | `false` | Delta-only BSP protocol (stateful workers); auto-enables coordinator-free rounds. |
| `--chunk-size <N>` | `10000` | Streaming agents per `AgentBatch`; `4294967295` disables streaming. |
| `--streaming-strategy <round-robin\|fennel>` | `round-robin` | Partition strategy (`fennel` uses `--fennel-alpha`). |
| `--fennel-alpha <F>` | `1.0` if fennel | Fennel balance factor (with `--streaming-strategy fennel`). |
| `--dispatch-mode <auto\|push\|pull>` | `auto` | Pull-dispatch mode. |
| `--max-pending-lifetime <N>` | `16` | Max batches a forward-reference may stay unresolved. |

Elastic grid (SPEC-20):

| Flag | Default | Description |
|------|---------|-------------|
| `--hybrid` | off | Coordinator also acts as a worker (`WorkerId 0`). |
| `--elastic-departure` | off | Recover partitions from departing workers (auto-enables `--retain-partitions`). |
| `--retain-partitions` | off | Retain partitions on departure. |
| `--elastic-join` | off | Allow dynamic worker joins (auto-enabled by `--hybrid`/`--elastic-departure`). |
| `--checkpoint-partitions` | off | Persist retained partitions. |
| `--initial-wait-timeout <SECS>` | `30` | Initial wait for worker connections. |
| `--join-window-min-ms <MS>` | `50` | Min join-window duration. |
| `--join-window-max-ms <MS>` | `500` | Max join-window duration. |
| `--solo-budget <N>` | `10000` | Max interactions per solo-reducing batch. |

## worker

Run as worker: connect to a coordinator and reduce assigned partitions.

```bash
relativist worker -c 192.168.1.100:9000 --token "<base64>"
```

| Flag | Default | Description |
|------|---------|-------------|
| `-c, --coordinator <HOST:PORT>` | required | Coordinator address (`IP:PORT` or `HOST:PORT`). |
| `--token <BASE64>` | none | Auth token; must match the coordinator. |
| `--daemon` | `false` | Reconnect after each job (SPEC-16). |
| `--log-format <text\|json>` | TTY auto | Log output format. |
| `--transport <tcp\|unix>` | `tcp` | Transport backend. |
| `--socket-path <PATH>` | none | UDS path (with `--transport unix`). |
| `--no-tcp-nodelay` | off | Disable TCP_NODELAY. |
| `--send-buffer <BYTES>` | `4194304` | SO_SNDBUF. |
| `--recv-buffer <BYTES>` | `4194304` | SO_RCVBUF. |
| `--keepalive <SECS>` | `30` | TCP keepalive idle; `0` disables. |
| `--compression-threshold <BYTES>` | `1024` | LZ4 frame threshold; `0` compresses every frame. |
| `--use-zero-copy` | `false` | rkyv archive (requires `--features zero-copy`). |
| `--tls-ca <PATH>` | none | CA cert (PEM) verifying the coordinator. Feature `tls` only. |

## local

In-process grid simulation: full BSP cycle across N simulated workers, no TCP.

```bash
relativist local -w 2 -i net.bin -o reduced.bin -m metrics.json
```

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --workers <N>` | required (`>=1`) | Simulated workers. |
| `-i, --input <PATH>` | required | Input net file (`.bin`). |
| `-o, --output <PATH>` | none | Write reduced net. |
| `-m, --metrics <PATH>` | none | Write metrics (`.json`/`.csv`). |
| `--strategy <NAME>` | `round-robin` | Partitioning strategy. |
| `--max-rounds <N>` | unlimited | Max grid rounds. |
| `--strict-bsp` | `false` | Strict BSP mode. |
| `--log-format <text\|json>` | TTY auto | Log output format. |

Also accepts the delta/streaming/elastic flags shared with `coordinator`:
`--delta-mode`, `--chunk-size`, `--streaming-strategy`, `--fennel-alpha`,
`--dispatch-mode`, `--max-pending-lifetime`, `--hybrid`, `--elastic-departure`,
`--elastic-join`, `--initial-wait-timeout`, `--solo-budget`. Note:
`--delta-mode` is currently rejected on `local` (needs a coordinator runtime,
SPEC-19 R20) — omit it for the v1 full-partition path.

## reduce

Purely local sequential reduction: calls `reduce_all` directly, no
partitioning. Prints interactions and final agent count.

```bash
relativist reduce -i net.bin -o reduced.bin
```

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input <PATH>` | required | Input net file. |
| `-o, --output <PATH>` | none | Write reduced net. |

## inspect

Inspect an IC net file: prints live agent count, per-symbol counts (CON, DUP,
ERA), redex count, and whether the net is in normal form.

```bash
relativist inspect -i net.bin
```

| Flag | Default | Description |
|------|---------|-------------|
| `-i, --input <PATH>` | required | Net file to inspect. |

## generate

Generate a workload net and save it. Output format is inferred from the
extension (`.bin` bincode, `.ic` text).

```bash
relativist generate dual-tree -n 1000 -o out.bin
```

| Arg / flag | Default | Description |
|------------|---------|-------------|
| `<EXAMPLE>` | required | Example net (positional, value-enum; see below). |
| `-n, --size <N>` | required | Problem size (pairs, depth, or items, per example). |
| `-o, --output <PATH>` | required | Output path (`.bin` or `.ic`). |

Example values (`<EXAMPLE>`):

| Value | Description |
|-------|-------------|
| `ep-annihilation` | N ERA-ERA annihilation pairs (Profile A). |
| `ep-annihilation-con` | N CON-CON annihilation pairs (Profile A). |
| `ep-annihilation-dup` | N DUP-DUP annihilation pairs (Profile A). |
| `con-dup-expansion` | N CON-DUP commutation pairs (Profile B). |
| `dual-tree` | Dual tree of depth D (Profile B/C). |
| `mixed-rules` | ERA-ERA + CON-CON + CON-DUP in thirds (Profile C). |
| `erasure-propagation` | Chain of N CON agents with ERA at head (Profile C). |
| `tree-sum` | N items summed via Church add (left-fold chain). |
| `sum-of-squares` | 1^2 + 2^2 + ... + N^2 via Church add chain. |

## compute

Encode arithmetic or a registry codec, reduce (locally or distributed), and
decode the result. Three mutually-exclusive modes:

- **Legacy (SPEC-14):** positional `<op> <a> <b>` Church arithmetic.
- **Registry via `--encoder <name>`** with `--input <json>`.
- **Registry via `--codec <name>`** with `--input <json>` (same registry;
  `--encoder` and `--codec` are mutually exclusive).

```bash
relativist compute add 3 5
relativist compute --codec horner --input '{"coeffs":[1,2,3],"x":2}'
relativist compute --encoder horner --input '{...}' --encode-only -o encoded.bin
```

| Arg / flag | Default | Description |
|------------|---------|-------------|
| `<OPERATION>` | none | Legacy op (positional): `add`, `mul`, or `exp`. |
| `<A> <B>` | none | Legacy operands (positional `u64`). |
| `--encoder <NAME>` | none | Registry encoder name (e.g. `horner`, `church_add`). Conflicts with `--codec`. |
| `--codec <NAME>` | none | Alternate spelling of `--encoder`. Conflicts with `--encoder`. |
| `--input <JSON>` | none | Encoder input as JSON; required with `--encoder`/`--codec`. |
| `--workers <N>` | none (`>=1`) | Distributed reduction via grid; omit for local `reduce_all`. |
| `-o, --output <PATH>` | none | Write the reduced net (or un-reduced net with `--encode-only`). |
| `-m, --metrics <PATH>` | none | Write metrics JSON (legacy distributed path). |
| `--encode-only` | `false` | Stop after encode; write the un-reduced net. Requires `--output` and `--encoder`/`--codec`. |

Notes: `compute exp` may not decode (cyclic DUP sharing yields a non-canonical
Church normal form). Run `encoders list` for available codec names.

## bench

Run the benchmark suite (SPEC-09). Prints a summary table and optionally writes
CSV outputs.

```bash
relativist bench --benchmark dual_tree,church_add --workers 1,2,4 --csv-summary summary.csv
```

Selection / execution:

| Flag | Default | Description |
|------|---------|-------------|
| `--benchmark <LIST>` | all | Comma-separated benchmark IDs (snake_case, or `all`). |
| `--sizes <LIST>` | per-benchmark | Comma-separated problem sizes. |
| `--workers <LIST>` | `1,2,4,8` | Comma-separated worker counts. |
| `--mode <MODE>` | `local` | `sequential`, `local`, `tcp-localhost`, or `tcp-network`. |
| `--warmup <N>` | `2` | Warmup runs (discarded). |
| `--repetitions <N>` | `5` | Timed repetitions. |
| `--max-rounds <N>` | unlimited | Grid round limit. |
| `--strict-bsp` | `false` | Strict BSP mode. |
| `--skip-g1` | `false` | Skip O(N!) graph isomorphism; use symbol-count check (marked "G1 weak"). |

Benchmark IDs (`--benchmark`): `ep_annihilation`, `ep_annihilation_con`,
`ep_annihilation_dup`, `condup_expansion`, `dual_tree`, `tree_sum`,
`tree_sum_balanced`, `mixed_net`, `erasure_propagation`, `church_add`,
`church_mul`, `cascade_cross`, `church_sum_of_squares`, `all`.

CSV outputs:

| Flag | Default | Description |
|------|---------|-------------|
| `--csv-detail <PATH>` | none | Per-repetition detail CSV. |
| `--csv-rounds <PATH>` | none | Per-round CSV. |
| `--csv-summary <PATH>` | none | Aggregated summary CSV. |
| `--csv-sparse <PATH>` | none | Sparse-construction-memory sub-CSV (SPEC-09 §3.4.5; pairs with `--representation sparse`). |

Streaming / arena (SPEC-21/22):

| Flag | Default | Description |
|------|---------|-------------|
| `--chunk-size <N>` | eager (none) | Streaming chunk size; when set, routes through the chunked path. |
| `--max-pending-lifetime <N>` | `16` | Pending-store memory bound. |
| `--recycle-policy <P>` | `disable-under-delta` | `disable-under-delta` or `border-clean`. |
| `--representation <R>` | `dense` | `dense` or `sparse`. |

Stress-curve campaign (D-014):

| Flag | Default | Description |
|------|---------|-------------|
| `--campaign <stress-curve>` | none | Select a named campaign; consumes the flags below. |
| `--workload <W>` | none | `ep_annihilation`, `dual_tree`, or `condup_expansion` (required with the campaign). |
| `--env <in-process\|docker-tcp>` | none | Campaign environment (`docker-tcp` is driven by the bash orchestrator). |
| `--n-seq <LIST>` | canonical sweep | Comma-separated N override. |
| `--reps <N>` | `1` | Repetitions per N. |

## validate

Validate benchmark campaign CSV outputs (data-quality checks). Fails (exit `1`)
if any hard check does not pass.

```bash
relativist validate --detail results/detail.csv --summary results/summary.csv --rounds results/rounds.csv
```

| Flag | Default | Description |
|------|---------|-------------|
| `--detail <PATH>` | `results/detail.csv` | Detail CSV. |
| `--summary <PATH>` | `results/summary.csv` | Summary CSV. |
| `--rounds <PATH>` | `results/rounds.csv` | Rounds CSV. |

## update

Check for and install the latest release (SPEC-15 R19). Uses `gh` (private
repos) or `curl` (public), verifies the SHA256 checksum, and installs to the
canonical dir (`%LOCALAPPDATA%\relativist\bin` on Windows, `~/.relativist/bin`
on Unix), adding it to PATH if needed.

```bash
relativist update --check     # check only
relativist update             # download and install
```

| Flag | Default | Description |
|------|---------|-------------|
| `--check` | `false` | Only check for a new version; do not install. |

## completions

Generate a shell completion script to stdout (SPEC-15 R20).

```bash
relativist completions bash > ~/.bash_completion.d/relativist
relativist completions zsh  > ~/.zfunc/_relativist
relativist completions fish > ~/.config/fish/completions/relativist.fish
relativist completions powershell >> $PROFILE
```

| Arg | Description |
|-----|-------------|
| `<SHELL>` | `bash`, `zsh`, `fish`, or `powershell` (positional). |

## encoders

List registered encoders/codecs with their descriptions (SPEC-27 R22). Alias:
`codecs` (terminological symmetry with `compute --codec`).

```bash
relativist encoders list
relativist codecs list      # alias, same handler
```

| Action | Description |
|--------|-------------|
| `list` | List all registered encoders with descriptions (alphabetical). |

Default registry codecs: `church_add`, `church_exp`, `church_mul`,
`church_sum_of_squares`, `horner`.

## decode

Decode a bincode `.bin` file (typically the reduced net written by a
coordinator) back into the codec's JSON representation (D-017). Mirrors the
post-reduce decode stage of `compute --codec`. Auto-discovers the net root when
`root = None`. Prints to stdout unless `--output` is given.

```bash
relativist decode --codec horner -i reduced.bin
relativist decode --codec horner -i reduced.bin -o result.json
```

| Flag | Default | Description |
|------|---------|-------------|
| `--codec <NAME>` | none | Codec name (e.g. `horner`). Conflicts with `--encoder`. |
| `--encoder <NAME>` | none | Alias of `--codec`. Conflicts with `--codec`. |
| `-i, --input <PATH>` | required | bincode `.bin` net to decode. |
| `-o, --output <PATH>` | stdout | Write JSON result instead of printing. |
