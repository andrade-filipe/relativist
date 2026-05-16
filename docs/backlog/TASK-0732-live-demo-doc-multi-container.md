# TASK-0732: Doc + live-demo updates — multi-container Horner variant

**Spec:** none (documentation only)
**Bundle:** D-017
**Priority:** P1 (operator-facing; ships with the bundle but doesn't block code)
**Status:** TODO
**Depends on:** TASK-0730 (the script must exist) ; TASK-0731 (cite the smoke test)
**Blocked by:** TASK-0730
**Estimated complexity:** S (~50 LoC bash extension + ~80 lines markdown)

## Context

`docs/demos/live_demo.md` documents the current `horner_live_demo.sh` flow (4 steps, all single-container or in-process). With the multi-container variant landing, the operator needs:
1. A documented walkthrough of the new flow ("encode locally → spin up N containers → decode locally → inspect logs").
2. An optional interactive `scripts/horner_live_distributed_demo.sh` variant that wraps `horner_distributed_demo.sh` with the same `WAIT` (press-Enter) ergonomics as `horner_live_demo.sh`, suitable for the TCC defesa stage.

## Acceptance Criteria

- [ ] `docs/demos/live_demo.md` has a new section "Variante multi-container (D-017)" explaining:
  - Diferença entre `horner_live_demo.sh` (1 container, `--workers N` interno) e `horner_distributed_demo.sh` (N containers, BSP real via TCP).
  - Quando usar cada um (defesa concisa: o de hoje; defesa que demonstra distribuição real: o novo).
  - O comando de inspeção pós-execução: `docker logs relativist-worker-{1..N}`.
  - Caveat: a primeira execução faz `docker compose up` (puxa rede, monta volumes); pode demorar 10–30s. Sugerir warm-up no pré-flight.
- [ ] (Optional, may be folded into TASK-0730) `scripts/horner_live_distributed_demo.sh` exists — wraps `horner_distributed_demo.sh` with the same banner/`WAIT` ergonomics as `horner_live_demo.sh`, designed for live audience. ~50 LoC; reuses the binary-location and pre-flight blocks from `horner_live_demo.sh` lines 80–107.
- [ ] `docs/INDEX.md` references the new doc section (if the index lists demos).
- [ ] `docs/progress.md` D-017 closure entry mentions the new script path + the new doc anchor.

## Files to Create/Modify

- `docs/demos/live_demo.md` — extend with new section; ~80 lines markdown.
- `scripts/horner_live_distributed_demo.sh` — **new file** (optional but recommended), executable.
- `docs/INDEX.md` — modify if the demos section enumerates scripts.
- `docs/progress.md` — append D-017 closure paragraph (handled by `sdd-pipeline` agent at REFACTOR stage; this task only flags the requirement).

## Doc Skeleton — `docs/demos/live_demo.md`

```markdown
## Variante multi-container (D-017)

A partir de v0.22.0, há **duas** formas de demonstrar a redução distribuída:

| Script                            | Containers | Distribuição       | Caso de uso             |
|-----------------------------------|------------|--------------------|--------------------------|
| `horner_live_demo.sh` (default)   | 1          | Threads internas   | Defesa concisa, 4 passos |
| `horner_live_distributed_demo.sh` | 1 + N      | TCP BSP real       | Demonstrar a tese        |

### Quando usar a variante multi-container

Use quando a banca pedir para ver "por trás dos panos" — cada worker tem
seu próprio container, com log persistente:

    bash scripts/horner_live_distributed_demo.sh --workers 4

### Fluxo

1. Encode local (host): JSON → `data/horner_input.bin`.
2. Coordinator container sobe (porta 9000 exposta), carrega o `.bin`.
3. N worker containers conectam via `coordinator:9000`.
4. BSP loop até normal form; coordinator escreve `data/horner_output.bin`.
5. Decode local (host) → valor numérico final.
6. Operador inspeciona logs:
       docker logs relativist-coordinator-1
       docker logs relativist-worker-1   # ... e por aí vai
7. `docker compose stop` (NÃO `down` — preserva os logs).

### Pré-flight

* `docker ps` funciona.
* `cargo build --release --bin relativist` recente.
* `docker compose --profile bench-tcp run --rm bench-tcp compute --codec horner --input '{"coeffs":[1],"x":1}' --workers 1` para aquecer a imagem (evita lag de 10–30s ao vivo).
```

## Test Expectations (for test-generator)

- Doc-only changes need no Rust tests.
- The optional `horner_live_distributed_demo.sh` can be hand-validated; no automated check beyond `shellcheck`.

## Dependencies Context

- `horner_live_demo.sh` is the structural template (banner/WAIT/clear pattern; lines 113–123).
- `horner_distributed_demo.sh` (TASK-0730) is the engine; this script only adds `WAIT` between stages.

## Notes

- Keep the new script's I/O **identical** to `horner_live_demo.sh` so the audience experience is consistent (Portuguese banners, "↪ Pressione Enter para continuar..." prompt).
- If the operator passes `--big`, propagate to `horner_distributed_demo.sh --input '<big polynomial>'` — same envelope rules apply.
- This is **the** moment to align terminology: the doc should use "multi-container" consistently (not "distributed" alone, since the in-process `--workers` path is also distributed in the BSP sense).
