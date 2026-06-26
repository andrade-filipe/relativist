---
title: Phase 1 — Sequential + Local (in-process)
summary: How to run the in-process baseline campaign (3 profiles + encoding, ~3800 datapoints, ~6h) with `relativist bench --mode local`.
keywords: [phase 1, local, in-process, sequential, baseline, relativist bench, profile a, profile b, profile c, church arithmetic, csv, workers, repetitions]
modules: [bench]
specs: [SPEC-09]
audience: [contributor, researcher]
status: reference
updated: 2026-06-26
---

# Phase 1 — Sequential + Local (in-process)

Phase 1 executa os benchmarks em **um unico processo**, sem TCP nem Docker. Usa o comando `relativist bench` nativo com `--mode local`. E o baseline de referencia: se Phase 1 nao mostra speedup, Phase 2/3 nao poderao melhorar (elas apenas somam overhead de rede).

## Pre-requisitos

- `relativist` instalado (`cargo build --release` ou binario em `PATH`).
- `results/` criado na raiz do repositorio.
- Maquina ociosa (sem IDE, browser ou antivirus scan ativo). Detalhes em [campaigns/v1-local-baseline.md §Pre-flight](campaigns/v1-local-baseline.md).

## Comando completo (3 perfis + encoding)

```bash
cd codigo/relativist
mkdir -p results

# Profile A (embarrassingly parallel) — EP annihilation
relativist bench \
  --benchmark ep_annihilation_con,ep_annihilation_dup,ep_annihilation \
  --sizes 100,500,1000,5000,10000,50000,100000,500000,1000000,5000000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_a_detail.csv \
  --csv-rounds results/phase1_profile_a_rounds.csv \
  --csv-summary results/phase1_profile_a_summary.csv

# Profile B (expansion + collapse) — CON-DUP expansion
relativist bench \
  --benchmark condup_expansion,mixed_net \
  --sizes 10,50,100,500,1000,5000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_b_detail.csv \
  --csv-rounds results/phase1_profile_b_rounds.csv \
  --csv-summary results/phase1_profile_b_summary.csv

# Profile C (sequential dependency) — DualTree, Erasure
relativist bench \
  --benchmark dual_tree,erasure_propagation \
  --sizes 4,6,8,10,12,14,16,18,20,22 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_c_detail.csv \
  --csv-rounds results/phase1_profile_c_rounds.csv \
  --csv-summary results/phase1_profile_c_summary.csv

# Encoding + data-bound — Church arithmetic, TreeSum
relativist bench \
  --benchmark church_add,church_mul,tree_sum \
  --sizes 10,50,100,500,1000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_encoding_detail.csv \
  --csv-rounds results/phase1_encoding_rounds.csv \
  --csv-summary results/phase1_encoding_summary.csv
```

## Dicas

- `workers 0` e adicionado automaticamente para gerar a linha `sequential`.
- `--warmup 2` descarta duas rodadas iniciais para estabilizar cache/branch predictor.
- Para runs rapidos de desenvolvimento: `--repetitions 3 --warmup 1`.

## Strict BSP (subset)

Para medir rodadas reais de cascata cross-partition (e preparar Phase 3 LAN):

```bash
relativist bench \
  --benchmark cascade_cross \
  --sizes 10,50,100,500,1000 \
  --workers 1,2,4,8 \
  --strict-bsp \
  --warmup 2 --repetitions 10 \
  --csv-detail results/phase1_strict_detail.csv \
  --csv-rounds results/phase1_strict_rounds.csv \
  --csv-summary results/phase1_strict_summary.csv
```

Para `dual_tree` pequenos (6, 10, 14) em strict mode, mesma receita trocando `--benchmark`.

Resultado esperado (teorema): `cascade_cross(N) = N` rodadas com `workers >= 2`; `dual_tree(d) = d` rodadas.

## Validacao imediata

```bash
# Nenhuma linha com correct=false
awk -F, 'NR>1 && $6=="false"' results/phase1_profile_a_detail.csv | wc -l
# DEVE imprimir 0 em todos os CSVs

# Todas as configuracoes presentes
awk -F, 'NR>1 {print $1}' results/phase1_profile_a_summary.csv | sort -u
# Deve listar: ep_annihilation, ep_annihilation_con, ep_annihilation_dup
```

Se qualquer `correct=false` aparecer, **pare e investigue** — e regressao de G1. Nao prossiga para Phase 2.

## Proximo passo

- [Phase 2 — Docker](phase-2-docker.md) para medir `tcp_localhost`.
- [Campaign v1-local-baseline](campaigns/v1-local-baseline.md) para o fluxo completo da campanha congelada.
