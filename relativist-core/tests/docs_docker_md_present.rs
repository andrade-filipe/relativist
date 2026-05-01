//! IT-0609-04 — `docs/DOCKER.md` exists and documents the hybrid coordinator.
//!
//! This is a cargo-level file-existence + keyword grep test (TEST-SPEC-0609
//! implementation note: "This test CAN be a cargo `#[test]`"). It runs on
//! every `cargo test` invocation and catches a missing or stub `docs/DOCKER.md`
//! at PR time without requiring Docker.
//!
//! Floor delta: **+1 default** (this is one of the +1 tests from TASK-0609).
//!
//! Spec: TEST-SPEC-0609 IT-0609-04.

use std::fs;
use std::path::PathBuf;

/// Resolve the repo root from the test binary's `CARGO_MANIFEST_DIR`.
///
/// `relativist-core` lives at `<repo>/relativist-core/`; the docs directory
/// lives at `<repo>/docs/`. Walking one level up gives us the repo root.
fn repo_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("relativist-core has a parent directory (repo root)")
        .to_path_buf()
}

/// IT-0609-04: `docs/DOCKER.md` exists and documents the hybrid coordinator.
///
/// Acceptance criteria (TEST-SPEC-0609 §IT-0609-04):
///   1. File exists at `docs/DOCKER.md`.
///   2. File length >= 30 lines.
///   3. Contains "hybrid coordinator" (case-insensitive).
///   4. Contains a reference to "D-006" or "post-D-006" or
///      "coordinator reduces a local partition".
///   5. Contains "bench-tcp".
///   6. Contains "CHUNK_SIZE", "MAX_PENDING_LIFETIME", "RECYCLE_POLICY"
///      (env-var parameterisation).
///   7. Contains a troubleshooting section marker ("troubleshoot" or
///      "common issues" or "known limitations", case-insensitive).
#[test]
fn docker_md_exists_and_documents_hybrid_coordinator() {
    let docker_md = repo_root().join("docs").join("DOCKER.md");

    assert!(
        docker_md.exists(),
        "IT-0609-04: docs/DOCKER.md must exist at {docker_md:?}"
    );

    let content = fs::read_to_string(&docker_md)
        .unwrap_or_else(|e| panic!("IT-0609-04: failed to read docs/DOCKER.md: {e}"));

    let lines = content.lines().count();
    assert!(
        lines >= 30,
        "IT-0609-04: docs/DOCKER.md must have >= 30 lines (got {lines})"
    );

    let lower = content.to_lowercase();

    assert!(
        lower.contains("hybrid coordinator"),
        "IT-0609-04: docs/DOCKER.md must contain 'hybrid coordinator' \
         (documents the post-D-006 coordinator change)"
    );

    let has_d006_ref = lower.contains("d-006")
        || lower.contains("post-d-006")
        || lower.contains("coordinator reduces a local partition");
    assert!(
        has_d006_ref,
        "IT-0609-04: docs/DOCKER.md must reference 'D-006', 'post-D-006', \
         or 'coordinator reduces a local partition'"
    );

    assert!(
        content.contains("bench-tcp"),
        "IT-0609-04: docs/DOCKER.md must contain 'bench-tcp' \
         (documents the new compose profile)"
    );

    assert!(
        content.contains("CHUNK_SIZE"),
        "IT-0609-04: docs/DOCKER.md must document the CHUNK_SIZE env var"
    );

    assert!(
        content.contains("MAX_PENDING_LIFETIME"),
        "IT-0609-04: docs/DOCKER.md must document the MAX_PENDING_LIFETIME env var"
    );

    assert!(
        content.contains("RECYCLE_POLICY"),
        "IT-0609-04: docs/DOCKER.md must document the RECYCLE_POLICY env var"
    );

    let has_troubleshoot = lower.contains("troubleshoot")
        || lower.contains("common issues")
        || lower.contains("known limitations");
    assert!(
        has_troubleshoot,
        "IT-0609-04: docs/DOCKER.md must contain a troubleshooting or \
         known-limitations section"
    );
}
