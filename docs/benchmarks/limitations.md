# Limitacoes Conhecidas (L1–L7)

Sete itens L apareceram durante as campanhas de Phase 1/2 e alimentam decisoes de v2. Este documento e a referencia rapida; o historico detalhado fica em `docs/PHASE1-FINDINGS.md` e `docs/PHASE2-FINDINGS.md`.

## L1 — Sem break-even em memoria compartilhada — ABERTO (v2)

Overhead de distribuicao excede o ganho paralelo para todas as configuracoes testadas in-process. Break-even so e esperado em maquinas separadas por rede (Phase 3 LAN).

Ver `docs/ROADMAP.md` Secao 2.40 (break-even analysis: c_o/c_r ≈ 2.2 observado; precisa < 0.50 para w=2).

## L2 — Loop BSP colapsado em uma rodada — RESOLVIDO em v0.10.0-bench

**Sintoma.** Ate v0.9.x, `run_grid` em `src/merge/grid.rs` rodava `reduce_all(&mut merged_net)` na fase RESOLVE BORDERS apos cada merge. Isso esgotava completamente a fila — incluindo cascatas cross-partition recem-criadas — entao todo run terminava em **exatamente uma rodada BSP**, independente da topologia.

**Problema.** Tornava ilusoria a medicao de custo por rodada e divergia do spec (SPEC-09: `Rounds = d` para DualTree).

**Fix.** Modo opt-in `strict_bsp=true` no `GridConfig`. Substitui `reduce_all` por `reduce_border_once` em `src/reduction/engine.rs`: fila atual processada exatamente uma vez, novas cascatas ficam enfileiradas para a proxima rodada. Default continua lenient (`false`) — zero regressao nos 643+ testes.

Uso: `--strict-bsp` na CLI. Detalhes em `specs/SPEC-05-merge.md` secao "Lenient vs Strict BSP".

## L3 — Round-robin partitioning only — ABERTO (v2)

Nao ha particionamento topology-aware. Planejado para v2 (ROADMAP 2.4+).

## L4 — Star topology, coordinator unico — ABERTO (v2)

Escalabilidade limitada pela banda de merge do coordinator. Tree/ring/gossip topologies em ROADMAP 2.30+.

## L5 — Readback exponencial — ABERTO (v2)

Resultados de `church exp` nao decodificam de volta para inteiro por conta de DUP sharing ciclico. A reducao em si e correta (forma normal valida). Limitacao do decoder atual (`decode_shared_chain`). Detalhes em `specs/SPEC-14-encoding.md`.

## L6 — Teto de payload do protocolo (256 MiB) — RESOLVIDO

**Sintoma.** Em v0.9.0, `DEFAULT_MAX_PAYLOAD_SIZE` em `src/protocol/frame.rs` era 256 MiB. Bloqueava 4 das 40 configuracoes de Phase 2: `dual_tree=22 w=1` e `ep_annihilation_con=5M w={1,2,4}`.

**Causa raiz (dupla).**

1. `ContiguousIdStrategy` atribuia IDs altos ao ultimo worker, forcando `Vec<PortRef>` do tamanho total mesmo com poucos slots vivos.
2. O cap de 256 MiB era guard-rail anti-DoS sem contrapartida na propriedade de confluencia.

**Fix (duas partes ortogonais).**

- **CompactSubnet** (`src/partition/compact.rs`) + `serialize_with`/`deserialize_with` em `Partition::subnet`: serializa apenas agentes vivos como `(id, agent, [ports; 3])` e reconstroi arena denso no receptor. Roundtrip preserva `agents`, `ports`, `redex_queue`, `next_id` e `root` byte-por-byte.
- **Cap elevado para 1 GiB.**

**Pos-fix.** Phase 2 roda 40/40 com G1 = 100%, ganhos de 40–100% de speedup onde padding era dominante (`results/post_fix/B3_comparison.md`).

## L7 — Shutdown race do coordinator — MITIGADO no driver

**Sintoma.** `docker compose up --abort-on-container-exit --exit-code-from coordinator` mata o coordinator com SIGTERM (depois SIGKILL, exit 137) assim que o primeiro worker sai — antes de o coordinator persistir `metrics.json` e `output.bin`. Reducao completa, mas artefatos nao saem.

**Mitigacao.** `scripts/bench_docker_resume2.sh::run_docker_cycle()` usa `docker compose up -d` + `docker wait relativist-coordinator-1` ate o coordinator sair sozinho. Nao precisa de flag de abort. Um SIGTERM handler interno no coordinator e um hardening opcional registrado em ROADMAP 2.x.

---

## Verificacao G1 completa (opcional, overnight)

A baseline `v1_local_baseline` usa **abordagem A** por padrao para `condup_expansion` em 10k e 50k: `--skip-g1` desliga o isomorfismo estrutural (`nets_isomorphic`, O(N!) backtracking) e mantem o **weak check** — igualdade de contagem de agentes, redexes e totais por regra entre sequencial e grid.

Weak check detecta qualquer divergencia de tamanho na normal form, mas nao prova identidade ponto-a-ponto de topologia.

Se quiser **fortalecer** com abordagem B (nao substituir), rode com a maquina ociosa:

```bash
mkdir -p results/optional

# condup_expansion(10000) — varias horas
./target/release/relativist bench \
    --benchmark condup_expansion --sizes 10000 \
    --workers 2 --repetitions 1 --warmup 0 --mode local \
    --csv-detail results/optional/condup_10k_fullg1_detail.csv

# condup_expansion(50000) — potencialmente intratavel (>12h)
./target/release/relativist bench \
    --benchmark condup_expansion --sizes 50000 \
    --workers 2 --repetitions 1 --warmup 0 --mode local \
    --csv-detail results/optional/condup_50k_fullg1_detail.csv
```

Interpretacao:

- `correct=true` → reforca a baseline (nao substitui; run separado tem estado inicial ligeiramente diferente por shuffle de `--warmup 0`).
- **Nao completa em 12h** → documente o wall-clock e reporte no TCC como **evidencia empirica de intratabilidade** — justifica a escolha da abordagem A.
- `correct=false` → **regressao critica**, abrir issue.

`--repetitions 1` e deliberado. Abordagem B e prova topologica unica, nao medicao temporal.
