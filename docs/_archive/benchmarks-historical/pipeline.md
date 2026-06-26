# Pipeline Completa

Um exemplo integrado: gerar, inspecionar, reduzir de duas formas (sequencial e grid) e comparar. Util para smoke test de uma instalacao ou de uma build nova.

## 1. Smoke test minimo

```bash
# 1. Gerar rede mixed-rules com 20 pares de cada regra
relativist generate mixed-rules -n 20 -o mixed20.bin

# 2. Inspecionar a rede original
relativist inspect -i mixed20.bin
# Agents: 240, Redexes: 120

# 3. Reduzir sequencialmente
relativist reduce -i mixed20.bin -o mixed20_seq.bin

# 4. Inspecionar resultado sequencial
relativist inspect -i mixed20_seq.bin
# Agents: 80 (todas ERA), Redexes: 0, Normal Form: yes

# 5. Reduzir via grid com 4 workers
relativist local -w 4 -i mixed20.bin -o mixed20_grid.bin -m mixed20_metrics.json

# 6. Inspecionar resultado do grid
relativist inspect -i mixed20_grid.bin
# Deve ter o mesmo resultado: 80 ERA, 0 redexes
```

## 2. Pipeline de campanha para o TCC

Gera os tres perfis num so comando, pronto para analise:

```bash
mkdir -p results

# Profile A — EP-Annihilation (deveria ser embaracosamente paralelo)
relativist bench \
  --benchmark ep_annihilation,ep_annihilation_con,ep_annihilation_dup \
  --sizes 100,500,1000,5000,10000 \
  --workers 1,2,4,8 \
  --warmup 2 --repetitions 10 \
  --csv-detail results/profile_a_detail.csv \
  --csv-rounds results/profile_a_rounds.csv \
  --csv-summary results/profile_a_summary.csv

# Profile B — Expansion + Collapse
relativist bench \
  --benchmark condup_expansion,mixed_net,church_add,church_mul \
  --sizes 10,50,100,500 \
  --workers 1,2,4,8 \
  --warmup 2 --repetitions 10 \
  --csv-detail results/profile_b_detail.csv \
  --csv-rounds results/profile_b_rounds.csv \
  --csv-summary results/profile_b_summary.csv

# Profile C — Sequential Dependency
relativist bench \
  --benchmark dual_tree,erasure_propagation \
  --sizes 4,6,8,10 \
  --workers 1,2,4,8 \
  --warmup 2 --repetitions 10 \
  --csv-detail results/profile_c_detail.csv \
  --csv-rounds results/profile_c_rounds.csv \
  --csv-summary results/profile_c_summary.csv
```

Para a campanha **congelada** (com environment hygiene, CV triage e manifest), siga [campaigns/v1-local-baseline.md](campaigns/v1-local-baseline.md).
