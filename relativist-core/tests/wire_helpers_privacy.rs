// CT-0711-05: SPEC-27 v3 R13a' obligation 3 (privacy).
//
// `wire_add_into` and `wire_mul_into` MUST be `pub(crate)` only — NOT
// re-exported via `relativist_core::encoding::*`. HornerCodec (TASK-0714)
// lives in the same crate (`relativist-core::encoding::horner`) and can
// call the helpers directly; external crates MUST NOT have visibility.
//
// Approach: source-inspection on `encoding/mod.rs`. A compile_fail doc-test
// would require generating doc-tests on a pub(crate) item, which rustdoc
// does not honor. A standard integration test that imports the helpers
// would fail to compile and break the build, so we verify privacy by
// asserting the helper names do not appear in the public re-export list.

#[test]
fn wire_helpers_not_in_public_re_exports() {
    let src = include_str!("../src/encoding/mod.rs");

    // Look only at lines starting with `pub use` (re-exports).
    let pub_uses: Vec<&str> = src
        .lines()
        .filter(|l| l.trim_start().starts_with("pub use "))
        .collect();

    let combined = pub_uses.join("\n");

    assert!(
        !combined.contains("wire_add_into"),
        "wire_add_into MUST remain pub(crate) — not re-exported. \
         Found in: {combined}"
    );
    assert!(
        !combined.contains("wire_mul_into"),
        "wire_mul_into MUST remain pub(crate) — not re-exported. \
         Found in: {combined}"
    );
}
