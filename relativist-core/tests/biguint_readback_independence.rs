// CT-0712-06: SPEC-27 v3 R14' Independence clause.
//
// `decode_biguint` MUST NOT delegate to `decode_nat`. The cross-check
// property in R16b' / T12 is meaningful only because the two readbacks
// are independent code paths. Verified by source-inspection on
// `relativist-core/src/encoding/biguint_readback.rs`: the production
// portion of the file (excluding rustdoc and the test module) MUST NOT
// contain a `decode_nat(` call.
//
// A shared `walk_church<Counter>` helper would be permitted (per R14'
// "Independence from decode_nat" clause) and would not contain the
// substring `decode_nat(` either; this check therefore tolerates the
// optional shared-helper design.

#[test]
fn decode_biguint_does_not_call_decode_nat() {
    let src = include_str!("../src/encoding/biguint_readback.rs");

    // Restrict the inspection to production code: stop at the first
    // `#[cfg(test)]` (the cross-check property tests legitimately call
    // `decode_nat`).
    let prod_src = match src.find("#[cfg(test)]") {
        Some(idx) => &src[..idx],
        None => src,
    };

    // Strip rustdoc / comment lines so documentation that names
    // `decode_nat` (e.g., "Mirrors SPEC-14 §4.4 decode_nat topology") does
    // not trigger the test failure.
    let code_only: String = prod_src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("///") && !t.starts_with("//!") && !t.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code_only.contains("decode_nat("),
        "decode_biguint MUST be standalone (R14' Independence clause). \
         Found a `decode_nat(` call in non-comment, non-test code:\n{code_only}"
    );
}
