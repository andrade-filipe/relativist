# Campanha `v1_local_baseline` (congelada)

Passo-a-passo operacional para rodar a campanha unificada Phase 1 + Phase 2 que gera o snapshot congelado `results/locked/v1_local_baseline/`. Esta e a referencia que a Phase 3 LAN vai subtrair para isolar o custo de rede — a qualidade dos dados aqui determina a validade de toda conclusao downstream.

- **Quem deve rodar:** o autor (Filipe), em maquina unica, sem carga de fundo.
- **Reproducao em outra maquina:** use `scripts/reproduce_local_baseline.sh` (ver §5).
- **Tempo total estimado:** 6-9 horas unattended (Phase 1 ~4-6 h + Phase 2 ~1.5-3 h).

Planeje rodar durante a noite ou durante um dia em que a maquina possa ficar dedicada.

## 1. Pre-flight checklist

Cada item e uma condicao necessaria para a reprodutibilidade do snapshot.

### 1.1 Estado do repositorio

```bash
cd codigo/relativist

git describe --tags --exact-match   # Deve imprimir: v0.10.0-bench
git status --short                  # Deve ser vazio
git log -1 --oneline                # Anote o SHA completo
```

Se `git describe` imprime outra coisa, faca `git checkout v0.10.0-bench`. Se `git status` lista arquivos modificados, faca stash ou commit — **nao rode a campanha com working copy sujo**.

### 1.2 Build release warning-free

```bash
cargo build --release 2>&1 | tee /tmp/build.log
grep -i warning /tmp/build.log               # Deve ser vazio
ls -lh target/release/relativist.exe         # (Windows) ou target/release/relativist
```

Se aparecer qualquer warning, **pare** e corrija antes. Binario com warnings indica codigo que pode mudar silenciosamente em atualizacoes futuras e contamina a rastreabilidade do snapshot.

### 1.3 Toolchain

```bash
rustc --version                # Anote para o manifest
cargo --version
docker --version               # Necessario para Phase 2
docker info | grep -i "memory\|cpus"
```

### 1.4 Environment hygiene (Windows 11)

| Item | Como configurar | Por que |
|---|---|---|
| Power plan | Control Panel -> Power Options -> **High performance** (ou Ultimate) | Evita throttling por tempo ocioso |
| Windows Update | Pause 1 week | Evita reboot inesperado + downloads no meio |
| Antivirus | Pausar scan agendado | Scans saturam I/O e distorcem wall-clock |
| Browsers | Fechar Chrome/Firefox/Edge | Cada aba e um processo V8 com GC aleatorio |
| IDE | Fechar VS Code/IntelliJ | `rust-analyzer` reindexa em background |
| Sleep | System -> Power -> Screen and sleep -> Never | Suspensao aborta a campanha |
| Notificacoes | Focus assist -> Alarms only | Reduz ruido sistemico |

### 1.5 Diretorio de destino

```bash
ls results/locked/v1_local_baseline/
# Esperado: README.md manifest.md  (apenas templates)

# Se houver phase1_*.csv ou raw/ de runs anteriores, mova para backup:
# mv results/locked/v1_local_baseline/phase1_* /tmp/old_snapshot/
```

## 2. Executar Phase 1 (4-6 h)

Phase 1 roda os 12 benchmarks em modo `local` (in-process, sem Docker) a 10 repeticoes cada, mais o pass strict-BSP para `cascade_cross` e `dual_tree`. Saida em `phase1_{lenient,strict}_{detail,rounds,summary}.csv` + logs em `raw/phase1/`.

```bash
cd codigo/relativist

echo "Phase 1 start: $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
bash scripts/bench_phase1_locked.sh 2>&1 | tee -a /tmp/v1_baseline.log
echo "Phase 1 end:   $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
```

O driver e idempotente sobre raw/*.log individuais, mas **nao** sobre os CSV agregados (reescritos ao fim). Ele imprime uma linha por benchmark:

```
[HH:MM:SS] LENIENT ep_annihilation (workers=1,2,4,8 reps=10)
```

### 2.1 Validacao obrigatoria antes de Phase 2

```bash
# (a) Zero correct=false
awk -F, 'NR>1 && $6=="false"' results/locked/v1_local_baseline/phase1_lenient_detail.csv | wc -l
awk -F, 'NR>1 && $6=="false"' results/locked/v1_local_baseline/phase1_strict_detail.csv | wc -l
# Ambos DEVEM imprimir 0

# (b) Row counts
wc -l results/locked/v1_local_baseline/phase1_lenient_detail.csv
wc -l results/locked/v1_local_baseline/phase1_strict_detail.csv
wc -l results/locked/v1_local_baseline/phase1_lenient_summary.csv
wc -l results/locked/v1_local_baseline/phase1_strict_summary.csv

# (c) Todos os benchmarks presentes
awk -F, 'NR>1 {print $1}' results/locked/v1_local_baseline/phase1_lenient_summary.csv | sort -u
# Deve listar: cascade_cross, church_add, church_mul, condup_expansion,
#              dual_tree, ep_annihilation, ep_annihilation_con, ep_annihilation_dup,
#              erasure_propagation, mixed_net, tree_sum, tree_sum_balanced
```

Se qualquer validacao falhar, **nao prossiga**. Investigue o `raw/phase1/<bench>.log` correspondente, corrija e re-rode Phase 1 inteira. E mais barato descartar 4-6 h agora do que contaminar o snapshot.

## 3. Executar Phase 2 (1.5-3 h)

Phase 2 roda 8 combinacoes `(benchmark × tamanho)` sobre TCP-localhost via Docker Compose, com 10 repeticoes × 4 contagens de workers + 8 baselines sequenciais nativos. Total: 40 datapoints.

### 3.1 Pre-flight Docker

```bash
docker info | head                        # Sem erro
docker ps                                 # Sem containers relativist-*
docker system prune -f                    # Limpa layers pendurados
docker compose build worker coordinator   # Pre-cache (opcional)
```

### 3.2 Comando

```bash
cd codigo/relativist
echo "Phase 2 start: $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
bash scripts/bench_phase2_locked.sh 2>&1 | tee -a /tmp/v1_baseline.log
echo "Phase 2 end:   $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
```

O driver usa `docker compose up -d` -> `docker wait coordinator` -> parse `metrics.json` -> proxima repeticao. Veja [limitations.md L7](../limitations.md#l7) para a motivacao.

### 3.3 Validacao imediata

```bash
cd codigo/relativist

# Zero correct=false
awk -F, 'NR>1 && $6=="false"' results/locked/v1_local_baseline/phase2_detail.csv | wc -l  # = 0

# Contagens
wc -l results/locked/v1_local_baseline/phase2_detail.csv
# Esperado: 329 (8 sequencial + 8*4*10 Docker + 1 header)
wc -l results/locked/v1_local_baseline/phase2_summary.csv
# Esperado: 41

# Todas as 8 combinacoes bench*size
awk -F, 'NR>1 {print $1 "_" $2}' results/locked/v1_local_baseline/phase2_summary.csv | sort -u | wc -l  # = 8
```

Se qualquer config travar (timeout 1800 s), o driver para (set -e). O `raw/phase2/metrics_<bench>_<size>_w<w>_r<rep>.json` do config ofensor fica no disco para forensica. Re-rodar apenas o config faltante exige editar o driver; se faltar tempo, rode Phase 2 inteira.

## 4. Pos-campanha: manifest, CV triage, freeze commit

### 4.1 Preencher `manifest.md`

O template tem placeholders `<FILL>` para ~15 campos. Colete:

```bash
cd codigo/relativist

# Provenance
git rev-parse v0.10.0-bench                  # Commit SHA
rustc --version                              # Toolchain
# Start/end: extraia de /tmp/v1_baseline.log

# Hardware (Windows 11)
systeminfo | grep -E "OS Name|OS Version|System Model"
wmic cpu get name
wmic cpu get NumberOfCores,NumberOfLogicalProcessors
wmic memorychip get capacity
docker --version

# Checksums
for f in results/locked/v1_local_baseline/phase*_*.csv; do
    sha256sum "$f"
done

# Row counts
for f in results/locked/v1_local_baseline/phase{1,2}_*.csv; do
    printf "%-60s %s\n" "$(basename "$f")" "$(wc -l < "$f")"
done
```

Edite `manifest.md` substituindo cada `<FILL>`.

### 4.2 Triagem CV

```bash
python3 scripts/cv_triage.py
# Gera results/locked/v1_local_baseline/cv_triage.md com rows CV > 0.15
# Revise e ajuste dispositions (keep/rerun/exclude).
```

### 4.3 Freeze commit

```bash
cd codigo/relativist
git add results/locked/v1_local_baseline/

git commit -m "data: freeze v1_local_baseline snapshot

Phase 1: <N> rows lenient + <M> rows strict, 0 correct=false.
Phase 2: 329 rows detail (40 configs * 10 reps + 8 sequencial), 0 correct=false.
Binary: tag v0.10.0-bench (commit <SHA>).
Hardware: <MODEL>, <CORES>c/<THREADS>t, <RAM>GB, Windows 11.
Campaign window: <START> .. <END>.
sha256 checksums in manifest.md."

git push origin main
```

### 4.4 Bump do submodule no TCC

```bash
cd ..   # Sai do submodule
git add codigo/relativist
git commit -m "Sync Relativist submodule: v1_local_baseline snapshot frozen"
git push origin main
```

## 5. Reproduzir em outra maquina (opcional)

```bash
cd codigo/relativist
git fetch origin && git checkout v0.10.0-bench
cargo build --release
bash scripts/reproduce_local_baseline.sh
```

O script roda Phase 1 + Phase 2 em `results/reproduction/<data>/` e gera `comparison.md` contra a referencia. Wall-clock diverge (hardware diferente), mas colunas estruturais **devem casar exatamente**.

## 6. Troubleshooting

| Sintoma | Diagnostico | Acao |
|---|---|---|
| Phase 1 loga `correct=false` | Bug de correctness (G1 falhou) | Pare, abra issue, cheque `raw/phase1/<bench>.log` |
| Phase 2 trava em um config | Timeout 1800 s ou Docker Desktop travado | `docker ps`; se container zumbi, `docker compose down --remove-orphans` e re-rode Phase 2 |
| `docker wait` exit != 0 | Coordinator crashou | Veja `raw/phase2/metrics_*.json`: se ausente, crash; se presente e `correct=false`, bug de redex |
| CV > 0.30 em varios configs | Maquina com carga de fundo | Rode Phase 1 de novo em ambiente mais limpo — `keep` individual nao resolve padrao sistemico |
| Wall-clock 2-3x maior | Power plan voltou para Balanced ou CPU throttling | Confira Control Panel -> Power Options; se quente, aguarde esfriar |
| Phase 1 leva > 8 h | `condup_expansion` em 10k/50k com G1 full | Confirme que driver usa `--skip-g1` nesses sizes (default) |
| manifest.md mal preenchido | Valores perdidos | `git checkout results/locked/v1_local_baseline/manifest.md` recupera o template |
