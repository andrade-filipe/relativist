# TEST-SPEC-0358: `TransportConfig.use_zero_copy` + `--use-zero-copy` CLI flag

**Task:** TASK-0358
**Spec:** SPEC-18 §3.9 (R36, R37)
**Generated:** 2026-04-16
**Baseline before this task:** 899 lib (default) / 923 lib (`--features zero-copy`, post-TASK-0357).

---

## Scope note

R36 mandates the `use_zero_copy: bool` field on `TransportConfig`,
default `false`. R37 mandates the matching `--use-zero-copy` CLI flag
on both `CoordinatorArgs` and `WorkerArgs`. Per TASK-0358's design
rationale, the field and flag are **unconditional** (NOT
`#[cfg(feature = "zero-copy")]`-gated) so that exhaustive `match`
arms compile in both builds and operators get a uniform CLI surface;
the `cfg(not(feature = "zero-copy"))` build emits a startup warning
when the flag is set but does not refuse to parse.

UT-01, UT-02, UT-04, UT-05 follow the exact pattern of the existing
`cli_compression_threshold_flag_threads_*` tests in `config.rs`.
UT-03 is the wire-effect integration test required by TASK-0358's
acceptance criteria final bullet.

---

## UT-0358-01: `--use-zero-copy` flag threads through CoordinatorArgs

**Target file:** `relativist-core/src/config.rs` (test module).
**Feature gate:** none (CLI flag is unconditional per TASK-0358 design).
**R-mapping:** R37 (coordinator).

```rust
#[test]
fn cli_use_zero_copy_flag_threads_through_coordinator() {
    let cli = Cli::parse_from(&[
        "relativist", "coordinator",
        "--bind", "127.0.0.1:9000",
        "--workers", "2",
        "--use-zero-copy",
    ]);
    match cli.command {
        Commands::Coordinator(args) => {
            assert!(args.use_zero_copy, "CLI flag must parse to true");
            let cfg = build_coordinator_config(&args).expect("build_cfg");
            assert!(cfg.transport.use_zero_copy,
                "TransportConfig.use_zero_copy must propagate from CLI");
        }
        _ => panic!("expected Coordinator"),
    }
}
```

---

## UT-0358-02: `--use-zero-copy` flag threads through WorkerArgs

**Target file:** `relativist-core/src/config.rs` (test module).
**Feature gate:** none.
**R-mapping:** R37 (worker).

```rust
#[test]
fn cli_use_zero_copy_flag_threads_through_worker() {
    let cli = Cli::parse_from(&[
        "relativist", "worker",
        "--coordinator", "127.0.0.1:9000",
        "--use-zero-copy",
    ]);
    match cli.command {
        Commands::Worker(args) => {
            assert!(args.use_zero_copy);
            let cfg = build_worker_config(&args).expect("build_cfg");
            assert!(cfg.transport.use_zero_copy,
                "TransportConfig.use_zero_copy must propagate from CLI");
        }
        _ => panic!("expected Worker"),
    }
}
```

---

## UT-0358-03: `--use-zero-copy` is unconditional CLI surface (parses in both builds)

**Target file:** `relativist-core/src/config.rs` (test module).
**Feature gate:** none — runs in both builds.
**R-mapping:** R37 (cfg-independent CLI surface per TASK-0358 design).

```rust
/// SPEC-18 R37 + TASK-0358 design: `--use-zero-copy` is recognized in
/// both default and `--features zero-copy` builds. In the default
/// build, setting it logs a startup warning (verified separately) but
/// does not refuse to parse — operators get a uniform surface.
#[test]
fn cli_use_zero_copy_parses_in_default_build() {
    let cli = Cli::try_parse_from(&[
        "relativist", "coordinator",
        "--bind", "127.0.0.1:9000",
        "--workers", "1",
        "--use-zero-copy",
    ]);
    assert!(cli.is_ok(),
        "--use-zero-copy must parse cleanly in BOTH builds; got: {:?}",
        cli.err()
    );
}
```

---

## UT-0358-04: `TransportConfig.use_zero_copy` defaults to `false`

**Target file:** `relativist-core/src/protocol/config.rs` (test module).
**Feature gate:** none.
**R-mapping:** R36 (default false).

```rust
#[test]
fn transport_config_use_zero_copy_defaults_to_false() {
    let cfg = TransportConfig::default();
    assert!(!cfg.use_zero_copy,
        "R36: TransportConfig.use_zero_copy MUST default to false");
}
```

**Note:** if the existing `test_transport_config_defaults` already
exists, extend it in place; otherwise add as a sibling. For the count
below, we count this as **+1** new test.

---

## UT-0358-05: CLI `--use-zero-copy` default is `false` when flag absent

**Target file:** `relativist-core/src/config.rs` (test module).
**Feature gate:** none.
**R-mapping:** R37 (default-off CLI surface).

```rust
#[test]
fn cli_use_zero_copy_default_is_false() {
    let cli = Cli::parse_from(&[
        "relativist", "coordinator",
        "--bind", "127.0.0.1:9000",
        "--workers", "2",
    ]);
    match cli.command {
        Commands::Coordinator(args) => {
            assert!(!args.use_zero_copy,
                "CLI default must be false");
            let cfg = build_coordinator_config(&args).expect("build_cfg");
            assert!(!cfg.transport.use_zero_copy,
                "TransportConfig default must be false");
        }
        _ => panic!("expected Coordinator"),
    }
}
```

---

## UT-0358-06: wire-effect integration — `use_zero_copy = true` produces FLAG_ARCHIVED frames

**Target file:** `relativist-core/src/protocol/frame.rs` (test module)
or new `relativist-core/src/protocol/zero_copy_tests.rs` per TASK-0359
recommendation — developer chooses; this test belongs with TASK-0358's
wire-effect integration acceptance bullet.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R36, R37 (end-to-end CLI → wire effect).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn integration_use_zero_copy_true_produces_archive_flagged_frames() {
    // Build a TransportConfig as if `--use-zero-copy` were passed, then
    // call send_frame_v2 with that config's flag and inspect the wire.
    let mut cfg = TransportConfig::default();
    cfg.use_zero_copy = true;
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    send_frame_v2(&mut tx, &msg, cfg.compression_threshold, cfg.use_zero_copy)
        .await.expect("send");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse");
    assert_eq!(header.flags & FLAG_ARCHIVED, FLAG_ARCHIVED,
        "with cfg.use_zero_copy = true the wire MUST carry FLAG_ARCHIVED");
}
```

---

## UT-0358-07: wire-effect integration — `use_zero_copy = false` does NOT set FLAG_ARCHIVED

**Target file:** same module as UT-06.
**Feature gate:** `#[cfg(all(test, feature = "zero-copy"))]`.
**R-mapping:** R36 (default off → no archive flag).

```rust
#[cfg(feature = "zero-copy")]
#[tokio::test]
async fn integration_use_zero_copy_false_does_not_set_archive_flag() {
    let cfg = TransportConfig::default(); // use_zero_copy = false
    let (mut tx, mut rx) = tokio::io::duplex(64 * 1024);
    let msg = Message::AssignPartition {
        round: 1,
        partition: sample_partition_with_borders(),
    };
    send_frame_v2(&mut tx, &msg, cfg.compression_threshold, cfg.use_zero_copy)
        .await.expect("send");
    drop(tx);
    use tokio::io::AsyncReadExt;
    let mut header_buf = [0u8; FRAME_HEADER_SIZE];
    rx.read_exact(&mut header_buf).await.expect("read header");
    let header = FrameHeader::from_bytes(&header_buf).expect("parse");
    assert_eq!(header.flags & FLAG_ARCHIVED, 0,
        "with cfg.use_zero_copy = false the wire MUST NOT carry FLAG_ARCHIVED");
}
```

---

## Test count

| Test | Default build | `--features zero-copy` build |
|------|---------------|------------------------------|
| UT-0358-01..05 | ✅ runs (5 tests) | ✅ runs (5 tests) |
| UT-0358-06..07 | ⏭ skipped (cfg-gated) | ✅ runs (2 tests) |
| **Total new tests** | **+5** | **+7** |

---

## Adversarial probes (QA candidates for Stage 5)

| # | Scenario | Why dangerous | Stage |
|---|----------|---------------|-------|
| QA-0358-A | `--use-zero-copy` + `--no-use-zero-copy` (last wins?) | clap default behavior; verify last-flag-wins semantics | QA |
| QA-0358-B | `--use-zero-copy=false` (explicit) | clap parses `bool` flags differently; verify `default_value_t = false` semantics | QA |
| QA-0358-C | Default-features build with `--use-zero-copy` set | startup warning emitted (per TASK-0358 acceptance bullet); verify via `tracing-test` capture | QA |
| QA-0358-D | Migration smoke: every existing call site of `send_frame_with_threshold` is either migrated to `send_frame_v2` or deliberately documented | catches missed migrations | QA / static |
| QA-0358-E | `Cli::try_parse_from` with `--zero-copy` (typo, missing `--use-`) | clap rejects with "unexpected argument"; sanity probe | QA |
| QA-0358-F | `TransportConfig` builder pattern (if exists) preserves `use_zero_copy = false` default | regression-guard on builder defaults | QA |

---

## Hard-to-write-deterministically tests (FLAGGED)

- **QA-0358-C (startup warning capture)** is **brittle** — depends on
  `tracing-test` or similar layer being wired in. NOT in this
  TEST-SPEC; flagged for QA discussion.

---

## Acceptance gate

1. Default build: `cargo test --workspace` count: 899 → **904** (+5: UT-01..05).
2. Feature build: `cargo test --workspace --features zero-copy` count:
   923 → **930** (+7: UT-01..07).
3. `cargo clippy --workspace --all-targets -- -D warnings` clean both ways.
4. `cargo fmt --check` clean.
5. CLI flag parses in BOTH builds; default is `false` in BOTH builds.
6. `TransportConfig.use_zero_copy: bool` field exists (not `Option<bool>`,
   not feature-gated).

---

## Out of scope

- T11-T14 end-to-end suite → TEST-SPEC-0359.
- Startup warning emission verification (deferred — QA-0358-C above).
- Migration of every `send_frame_with_threshold` call site to
  `send_frame_v2` — TASK-0358 recommends but does not mandate; QA
  probe.
