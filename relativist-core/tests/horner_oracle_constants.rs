// CT-0713-09: SPEC-27 v3 R12' / R16a' single source of truth.
//
// The Horner oracle MUST source the cap from the shared `MAX_CHURCH_NAT`
// constant, NOT a literal `10_000`. This source-inspection test prevents
// silent drift between the oracle's bound checks and the SPEC-14 R4 cap.

#[test]
fn horner_oracle_does_not_hardcode_cap() {
    let src = include_str!("../src/encoding/horner_oracle.rs");

    // Inspect production code only — exclude the `#[cfg(test)] mod tests`
    // block (test bodies legitimately use 10_000 / 10_001 literals to verify
    // boundary semantics).
    let prod_src = match src.find("#[cfg(test)]") {
        Some(idx) => &src[..idx],
        None => src,
    };

    // Strip rustdoc / comment lines so documentation referencing 10_000
    // (e.g., "the current cap is 10_000") does not trigger the test.
    let code_only: String = prod_src
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !t.starts_with("///") && !t.starts_with("//!") && !t.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        !code_only.contains("10_000") && !code_only.contains("10000"),
        "horner_serial MUST source the cap from MAX_CHURCH_NAT (R12' single source of truth). \
         Found a `10_000` / `10000` literal in non-comment, non-test code:\n{code_only}"
    );

    assert!(
        code_only.contains("MAX_CHURCH_NAT"),
        "horner_serial MUST reference the MAX_CHURCH_NAT constant"
    );
}
