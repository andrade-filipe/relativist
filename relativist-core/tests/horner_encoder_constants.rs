// CT-0714-11: SPEC-27 v3 R12' single-source-of-truth for the encoder.
//
// Mirrors `horner_oracle_constants.rs` for the encoder file. The
// HornerCodec encoder MUST source its bound checks from
// `MAX_CHURCH_NAT`, NOT a hard-coded `10_000` literal.

#[test]
fn horner_encoder_does_not_hardcode_cap() {
    let src = include_str!("../src/encoding/horner.rs");

    // Restrict inspection to production code (exclude `#[cfg(test)]`).
    let prod_src = match src.find("#[cfg(test)]") {
        Some(idx) => &src[..idx],
        None => src,
    };

    // Strip rustdoc / comment lines.
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
        "HornerCodec::encode MUST source the cap from MAX_CHURCH_NAT \
         (R12' single source of truth). Found a `10_000` / `10000` \
         literal in non-comment, non-test code:\n{code_only}"
    );

    assert!(
        code_only.contains("MAX_CHURCH_NAT"),
        "HornerCodec::encode MUST reference the MAX_CHURCH_NAT constant"
    );
}
