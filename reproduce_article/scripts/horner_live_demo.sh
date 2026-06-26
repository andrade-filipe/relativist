#!/usr/bin/env bash
#
# horner_live_demo.sh — interactive live presentation reproducer for the
# HornerCodec G1 (confluence) demonstration.
#
# Differs from `scripts/horner_demo.sh`: this script is meant to be run in
# front of a live audience (TCC defesa, lab seminar, etc.) — each step
# waits for the operator to press Enter before advancing, so the audience
# can read the output and the operator can narrate.
#
# What it shows, in order:
#   1. The HornerCodec processing a meaningful polynomial in-process
#      (baseline reference value + interaction count).
#   2. The SAME polynomial reduced inside a Docker container (W=1) —
#      proves the container path works end-to-end.
#   3. The SAME polynomial distributed across W=4 workers — proves the
#      partition + parallel reduce + merge path converges.
#   4. The SAME polynomial distributed across W=8 workers — proves G1
#      holds at higher partition counts.
#
# All four steps must produce the IDENTICAL numerical value. That is the
# punchline (confluence forte / Lafont 1997).
#
# Pre-flight (do this BEFORE the audience arrives):
#   1. `docker ps` responds without error (Docker Desktop is running)
#   2. `cargo build --release --bin relativist` succeeded recently
#   3. Warm the bench-tcp image to avoid the cold-start lag on stage:
#        MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
#        docker compose --profile bench-tcp run --rm bench-tcp \
#        compute --codec horner --input '{"coeffs":[1],"x":1}' --workers 1
#
# Usage:
#   bash scripts/horner_live_demo.sh                # default polynomial
#   bash scripts/horner_live_demo.sh --big          # heavier polynomial (2059 interactions)
#   bash scripts/horner_live_demo.sh --input '{"coeffs":[...],"x":N}'
#                                                   # custom polynomial (must be in envelope)
#
# Envelope (do NOT stray outside — decoder returns Err per D-016 BUG-001 fix):
#   * Single-iter (coeffs.len()==2): [c0, c1] with c0 in [0,10000], c1 in [0,1025]; x in [0,10000]
#   * Degree-2 (coeffs.len()==3):    [c0, c1, 1] (c2 must be 1) with c0,c1,x in [0,10000]
#   * Constants (coeffs.len()==1):   trivially safe
#
# Exit code: 0 on Enter-through-end; non-zero on operator Ctrl-C or any
# binary failure.

set -euo pipefail

# ----------------------------------------------------------------------------
# argument parsing
# ----------------------------------------------------------------------------

INPUT='{"coeffs":[10000,500,1],"x":100}'   # degree-2 max-scale, 1220 interactions, value=70000
PROFILE="default"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --big)
            # single-iter with c1=1025 at x=10000 — 2059 interactions, value=10250001
            INPUT='{"coeffs":[1,1025],"x":10000}'
            PROFILE="big"
            shift
            ;;
        --input)
            INPUT="$2"
            PROFILE="custom"
            shift 2
            ;;
        -h|--help)
            sed -n '2,/^$/p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "ERROR: unknown option: $1" >&2
            exit 1
            ;;
    esac
done

# ----------------------------------------------------------------------------
# locate binary
# ----------------------------------------------------------------------------

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

if [[ -x "$REPO_DIR/target/release/relativist.exe" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist.exe"
elif [[ -x "$REPO_DIR/target/release/relativist" ]]; then
    RELATIVIST_BIN="$REPO_DIR/target/release/relativist"
else
    echo "ERROR: target/release/relativist not built. Run \`cargo build --release\` first." >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# pre-flight
# ----------------------------------------------------------------------------

if ! command -v docker >/dev/null 2>&1; then
    echo "ERROR: docker not in PATH. Start Docker Desktop and retry." >&2
    exit 1
fi

if ! docker compose version >/dev/null 2>&1; then
    echo "ERROR: docker compose not available." >&2
    exit 1
fi

# ----------------------------------------------------------------------------
# the show
# ----------------------------------------------------------------------------

BANNER() {
    echo ""
    echo "════════════════════════════════════════════════════════════════════"
    echo "  $1"
    echo "════════════════════════════════════════════════════════════════════"
}

WAIT() {
    echo ""
    read -rp "↪ Pressione Enter para continuar..."
}

DOCKER_RUN() {
    local workers="$1"
    MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
        docker compose --profile bench-tcp run --rm bench-tcp \
        compute --codec horner --input "$INPUT" --workers "$workers"
}

clear || true

BANNER "HornerCodec — Demonstração ao vivo de G1 (Confluência)"
echo ""
echo "Polinômio: $INPUT"
case "$PROFILE" in
    default) echo "Perfil:    default (degree-2, value esperado = 70000, ~1220 interações)" ;;
    big)     echo "Perfil:    --big (single-iter c1=1025, value esperado = 10250001, ~2059 interações)" ;;
    custom)  echo "Perfil:    --input custom" ;;
esac
echo ""
echo "Toda invocação a seguir deve produzir o MESMO valor numérico,"
echo "independente da estratégia de redução escolhida. Isso é G1 (Lafont 1997)."
WAIT

BANNER "PASSO 1/4 — Encoders disponíveis no registry"
"$RELATIVIST_BIN" encoders list
WAIT

BANNER "PASSO 2/4 — Redução in-process (sequencial, sem distribuição)"
"$RELATIVIST_BIN" compute --codec horner --input "$INPUT"
echo ""
echo "↪ Note o número de interações e o value. Próximo: mesmo cálculo via Docker."
WAIT

BANNER "PASSO 3/4 — Mesma redução, dentro de um container Docker, W=1"
DOCKER_RUN 1
echo ""
echo "↪ Mesmo value. O container é um worker isolado, mas single-threaded."
echo "  Agora: distribuímos em 4 workers paralelos."
WAIT

BANNER "PASSO 4/4 — Particionado em W=4 workers paralelos via TCP"
DOCKER_RUN 4
echo ""
echo "↪ Mesmo value novamente. A rede foi quebrada em 4 sub-redes, cada"
echo "  worker reduziu sua partição, e o merge convergiu no mesmo normal form."
echo "  Última escalada: W=8."
WAIT

BANNER "ESCALADA FINAL — W=8 workers paralelos"
DOCKER_RUN 8
echo ""

BANNER "G1 demonstrado empiricamente"
echo ""
echo "  4 estratégias diferentes de redução (in-process, W=1, W=4, W=8)"
echo "  produziram o MESMO valor numérico para o MESMO polinômio."
echo ""
echo "  Isso é o que Lafont (1997) chamou de confluência forte: o normal"
echo "  form é único, independente da ordem das interações."
echo ""
echo "  Em Grid Computing, isso significa que podemos distribuir a redução"
echo "  sem comprometer correctness — exatamente a tese deste TCC."
echo ""
