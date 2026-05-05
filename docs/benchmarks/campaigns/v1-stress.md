# Campanha `v1_stress`

Campanha de stress que estende `v1_local_baseline` para sizes maiores (`ep_annihilation_con` ate 50 M, `dual_tree` ate `d=25`). O objetivo e produzir o dado **"antes"** dos itens 2.22-2.26 do [ROADMAP](../../ROADMAP.md) (otimizacoes de overhead de rede): mesma maquina, mesmo binario `v0.10.0-bench`, apenas sizes maiores.

- **Quem deve rodar:** o autor, apos terminar uma reproducao limpa de `v1_local_baseline` e **antes** de implementar qualquer item 2.22-2.26. As medicoes "antes / depois" so sao comparaveis na mesma maquina sob a mesma hygiene.
- **Saida:** `results/extended/v1_stress/` (nao em `results/locked/`).
- **Tempo total estimado:** 4-6 h unattended (Phase 1 ~1-2 h + Phase 2 ~3-4 h).

## Por que stress e campanha separada de `v1_local_baseline`

1. `v1_local_baseline` e snapshot congelado versionado como referencia cientifica da Phase 3 LAN. Nao pode crescer depois de congelado.
2. A campanha de stress explora regimes onde a heuristica de hardware comeca a quebrar (U-series throttling, 1 GiB frame cap, Docker WSL2 memory pressure). Alguns configs **podem falhar** — esperado e documentado, nao e bug. Congelar falhas dentro da baseline principal contaminaria Phase 3.
3. A campanha usa **5 repeticoes** (nao 10), porque o custo por repeticao e maior; 10 empurraria wall-clock para 8-12 h, antieconomico para o proposito "dado antes de otimizacao".

## Layout

| Fase | Benchmarks × sizes | Workers | Reps | Observacao |
|---|---|---|---|---|
| Phase 1 stress (in-process) | `ep_annihilation_con × {10M,20M,50M}`, `dual_tree × {23,24,25}` | seq + local {1,2,4,8} | 5 | Sem Docker |
| Phase 2 stress (Docker) | `ep_annihilation_con × {10M,20M}`, `dual_tree × {23,24,25}` | {1,2,4,8} | 5 | Completo |
| Phase 2 stress (Docker) | `ep_annihilation_con × {50M}` | {4,8} apenas | 5 | w=1/w=2 puladas: particao > 1 GiB cap. [L6-related](../limitations.md#l6) |

## Diferencas vs. `bench_phase2_locked.sh`

O script `bench_phase2_stress_locked.sh` corrige o bug de shutdown do Docker Compose observado em 20 M no smoke test de 2026-04-11:

- `bench_phase2_locked.sh` usa `docker compose up --abort-on-container-exit --exit-code-from coordinator`. Em sizes de stress o coordinator demora mais para flushar `metrics.json` do que os workers demoram para sair; o `--abort-on-container-exit` SIGKILLa o coordinator no meio do flush e `metrics.json` nunca chega ao disco.
- `bench_phase2_stress_locked.sh` usa `docker compose up -d` + `docker wait <coordinator_id>` + `docker compose down`, deixando o coordinator sair naturalmente.

Detalhado no [item L7 de limitations.md](../limitations.md#l7).

## 1. Pre-flight checklist

Identico a [v1-local-baseline §1](v1-local-baseline.md#1-pre-flight-checklist) com duas excecoes: Phase 1 stress **nao** precisa de Docker (mas Phase 2 stress precisa), e a campanha roda em `results/extended/v1_stress/` (nao em `results/locked/`).

### 1.1 Estado do repositorio

```bash
cd codigo/relativist

git describe --tags --exact-match   # Deve imprimir: v0.10.0-bench
git status --short                  # Vazio
git log -1 --oneline                # Anote o SHA
```

A campanha **tem que rodar** contra a mesma tag de `v1_local_baseline`, senao as comparacoes "stress vs baseline" misturam binarios diferentes.

### 1.2 Environment hygiene (Windows)

```bash
powercfg /getactivescheme
# Output deve conter: (Desempenho Maximo) ou (Ultimate Performance)

# Se Balanced, ative Ultimate Performance:
powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61
powercfg /setactive <GUID_novo_impresso_acima>
powercfg /getactivescheme
```

Fechar antes do kick: IDEs, browsers, qualquer app com tray de sincronizacao (Dropbox, OneDrive, Google Drive). Pausar Windows Update.

### 1.3 Build release

```bash
cargo build --release
ls -la target/release/relativist.exe
```

### 1.4 Docker Desktop (so para Phase 2 stress)

```bash
docker compose ps      # Header + linhas vazias
docker compose build   # Pre-build fora do wall-clock
```

### 1.5 Espaco em disco

```bash
df -h .     # Precisa de ~3-5 GB livres para raw/phase2/metrics_*.json
```

## 2. Executar Phase 1 stress (1-2 h)

```bash
cd codigo/relativist
./scripts/bench_phase1_stress_locked.sh
```

O script:

1. Detecta o binario em `target/release/relativist.exe`.
2. Cria `results/extended/v1_stress/` e `raw/phase1/`.
3. Roda `ep_annihilation_con` em `10M,20M,50M` com workers `1,2,4,8` (+ sequential auto-adicionado).
4. Roda `dual_tree` em `23,24,25` com workers `1,2,4,8`.
5. Concatena em `phase1_stress_{detail,rounds,summary}.csv`.

Saida esperada ao final:

```
[HH:MM:SS] === Phase 1 Stress Campaign complete ===
[HH:MM:SS] Detail:  N rows -> .../phase1_stress_detail.csv
[HH:MM:SS] Rounds:  M rows -> .../phase1_stress_rounds.csv
[HH:MM:SS] Summary: K rows -> .../phase1_stress_summary.csv
```

Validacao rapida:

```bash
# Nenhum correct=false
awk -F, 'NR>1 && $6=="false"' results/extended/v1_stress/phase1_stress_detail.csv
# (output vazio)

# 2 benches * 3 sizes * 5 modos (seq + {1,2,4,8}) = 30 linhas + header
wc -l results/extended/v1_stress/phase1_stress_summary.csv
```

## 3. Executar Phase 2 stress (3-4 h)

Com Docker Desktop rodando:

```bash
cd codigo/relativist
./scripts/bench_phase2_stress_locked.sh
```

O script:

1. Roda `docker compose build` (a menos que `--skip-build`).
2. Gera baselines sequenciais nativos (fora do Docker) para cada `bench × size` distinto — usados para speedup vs sequential.
3. Para cada `bench × size × workers`:
   - Copia input para `data/input.bin`.
   - `docker compose up -d --scale worker=W`.
   - `docker wait coordinator` (sai naturalmente e flusha `metrics.json`).
   - Le `data/metrics.json`, valida G1 com `inspect`.
   - `docker compose down --remove-orphans`.
4. Escreve `phase2_stress_{detail,rounds,summary}.csv`.

Saida esperada:

```
[HH:MM:SS] ==========================================
[HH:MM:SS]   Phase 2 Stress Campaign Complete
[HH:MM:SS] ==========================================
[HH:MM:SS] Start: 2026-04-11 HH:MM:SS -0300
[HH:MM:SS] End:   2026-04-11 HH:MM:SS -0300
[HH:MM:SS] Output files:
[HH:MM:SS]   .../phase2_stress_detail.csv  (N rows)
[HH:MM:SS]   .../phase2_stress_summary.csv (M rows)
[HH:MM:SS]   .../phase2_stress_rounds.csv  (K rows)
```

**Configs esperados:** 6 `bench × size` com 4 workers + 1 `bench × size` (ep_con=50M) com 2 workers = 24 + 2 = **26 configs Docker**. Mais 7 baselines sequenciais. Total: **33 linhas de summary + header = 34**.

Validacao:

```bash
# Correct=false indica regressao critica
awk -F, 'NR>1 && $6=="false"' results/extended/v1_stress/phase2_stress_detail.csv

# Configs que pularam (exit != 0) aparecem com all_correct=false no summary.
# Espera-se zero falhas sob v0.10.0-bench, mas se houver, cheque o log raw.

# 26 configs * 5 reps + 7 seq * 5 reps + header = 166
wc -l results/extended/v1_stress/phase2_stress_detail.csv
```

## 4. Pos-campanha: manifest

Copie e adapte o template de `v1_local_baseline`:

```bash
cp results/locked/v1_local_baseline/manifest.md \
   results/extended/v1_stress/manifest.md
```

Edite `results/extended/v1_stress/manifest.md` para refletir:

- **Status:** COMPLETE ou documentar quais configs falharam
- **Campaign knobs:** 5 reps (nao 10), sizes diferentes
- **Secao "Differences from v1_local_baseline":** (a) sizes maiores, (b) 5 reps, (c) shutdown fix do Docker, (d) esta e a medicao "antes" dos itens ROADMAP 2.22-2.26
- **Checksums:** gere sha256 dos CSVs novos

```bash
cd results/extended/v1_stress
sha256sum phase1_stress_*.csv phase2_stress_*.csv > checksums.sha256
cat checksums.sha256
```

## 5. Troubleshooting

| Sintoma | Diagnostico | Acao |
|---|---|---|
| `ep_annihilation_con=50M w=4/w=8` falha com `metrics.json` ausente | Coordinator SIGKILL por OOM (WSL2 15 GiB, 50 M agentes ~3-4 GB de particao bincode v1) | Documente no manifest. Mostre que com 2.23 (wire compaction) o footprint cai <1 GB |
| `dual_tree=25 w=1` falha por frame cap | Particao > 1 GiB sob bincode v1 | Esperado — w=1 concentra tudo em uma particao. Documente |
| `docker wait` trava | Coordinator preso — provavel deadlock do protocolo | `docker compose logs coordinator` em outra shell; se em `reduce_all` ou `collect`, aguarde ou Ctrl+C |
| Wall-clock Phase 1 > 4 h | CPU throttling por calor | Interrompa, deixe esfriar 30 min, re-rode do zero |
| Phase 1 stress `correct=false` | Bug novo de correctness em sizes grandes | **Pare e investigue**. Se reproducivel, L-item critico: a baseline local nao esta mais valida nessa faixa |
