# BRIEF: D-014 Stress Curve — Análise de Falha da Run 2026-05-14

**Gerado:** 2026-05-14
**Escopo:** Análise pós-mortem da run overnight de `scripts/stress_curve.sh` iniciada em 00:30 e abortada em 10:03 (9h33min de wall). Resultado armazenado em `results/locked/v2_stress_curve_2026-05-14/raw/`.

---

## Sumário Executivo

A run de 2026-05-14 falhou em produzir dados utilizáveis por cinco bugs independentes, três deles anteriores a esta run (F1, F3, F4) e dois recém-descobertos (F2, F5). O único workload que rodou foi `ep_annihilation`; `dual_tree` e `condup_expansion` nunca começaram — mas a causa não é um bug de roteamento: é o tempo. `ep_annihilation` sozinho consumiu as 9h33min porque o script passa `--reps "$REP"` (contador 1..5) em vez de `--reps 1` a cada chamada, gerando entre 1 e 5 repetições internas por invocação e inflando o dataset 3.3x. O CSV resultante tem 152 headers duplicados, 147 linhas de tracing WARN contaminando o corpo, e `vmrss_peak_mb = 0.000000` em todas as 960 linhas de dados porque o `MemoryProbe` é Windows-only e o processo roda no host Windows enquanto o script usa Git Bash. O campo `correct = true` em todas as linhas é tecnicamente preciso — mas para N >= ~10M com workers > 1 o WARN `to_dense allocation cap hit; returning empty Net` em `partition/helpers.rs:699` indica que cada worker recebe uma subnet vazia. A correctness é verificada comparando a subnet vazia reduzida com a subnet vazia de baseline — comparação trivialmente verdadeira. Os 20 rows com `stop_reason=MemoryExceeded` são falsos positivos: o stop_rule disparou porque `vmrss_peak_fraction = 0.0 > 0.80` nunca é verdade (vmrss = 0), então a regra NUNCA disparou corretamente, e os 20 rows com MemoryExceeded foram anotados retroativamente pelo dispatch ao anotar o `last_attempted_n` — não por triagem de memória real. Para fazer a campanha rodar de verdade, cinco bugs precisam ser corrigidos antes do próximo disparo.

---

## 1. Inventário do que TASK-0700..0722 prometeu

### TASK-0700 — MemoryProbe
`docs/backlog/TASK-0700-stress-curve-memory-probe.md`

Prometeu: módulo `relativist-core/src/bench/memory_probe.rs` com `MemoryProbe::new()` retornando `Ok` em Linux e Windows; `peak_bytes()` via `VmHWM` (Linux) / `PeakWorkingSetSize` (Windows); `current_bytes()` via `VmRSS` / `WorkingSetSize`; macOS retorna `Err`. Critério AC-2: `peak >= current` e ambos > 0 após 100 MiB de alocação. Entregou +4 testes. Floor projetado: default >= 1802.

### TASK-0701 — StopRule
`docs/backlog/TASK-0701-stress-curve-stop-rule.md`

Prometeu: `StopRule { wall_budget, memory_fraction_max }` com `check(&RepResult) -> Option<StopReason>` implementando prioridade `Oom > MemoryExceeded > WallTimeExceeded`. `MemoryExceeded` dispara quando `vmrss_peak_fraction_of_total > memory_fraction_max`. Repassa `vmrss_peak_fraction_of_total` do `RepResult` — não lê `MemoryProbe` diretamente. Entregou +6 testes. Floor projetado: >= 1808.

### TASK-0702 — StressCurveDescriptor
`docs/backlog/TASK-0702-stress-curve-campaign-descriptor.md`

Prometeu: `StressCurveDescriptor::run_one_sequence(workload, env, workers, reps, ...)` — routa via `run_benchmark_suite` internamente, construindo `MemoryProbe::new()` uma vez. Hint #4: "For in_process env: invoke generator + `run_grid` directly inside the bench process (single rep per process is the script's responsibility, not this task's)." Prometeu +1 integration test.

### TASK-0703 — CSV Schema
`docs/backlog/TASK-0703-stress-curve-csv-schema.md`

Prometeu: 4 colunas adicionadas ao final da row: `vmrss_peak_mb`, `vmrss_current_end_mb`, `stop_reason`, `cv_above_gate`. TASK-0720 depois removeu `cv_above_gate` (BUG-006). Prometeu +2 testes.

### TASK-0704 — Bash Orchestrator
`docs/backlog/TASK-0704-stress-curve-bash-orchestrator.md`

Prometeu: script com `--smoke`, `--resume`, `--no-docker`, lockfile SIGINT/SIGTERM trap. Design spec §5 Phase 1: "each rep runs in a child process so that VmHWM resets between reps; each child invokes `relativist-bench ... --reps 1 --n-target N`". Prometeu que HEADER_WRITTEN elimina headers duplicados. CLI:  `--reps "$REP"` (loop var) — **essa passagem era para ser `--reps 1`** conforme o spec, mas foi implementada como `--reps "$REP"`.

### TASK-0705 — Plot Generator
`docs/backlog/TASK-0705-stress-curve-plot-generator.md` (não lido em detalhe — não afeta a run de coleta).

### TASK-0706 — Docs
Apenas documentação em `docs/benchmarks/campaigns/stress-curve.md`.

### TASK-0707 — Integration Tests
`docs/backlog/TASK-0707-stress-curve-integration-tests.md`

6 integration tests prometidos. Teste (e) `--resume invariant` é `#[cfg(unix)]`. Floor projetado: >= 1819.

### TASK-0708 — Campaign Run
`docs/backlog/TASK-0708-stress-curve-campaign-run-and-lock.md`

Sentinel de execução. Pré-condição: smoke run OK, floors >= 1819, clippy/fmt clean. Deliverable: `results/locked/v2_stress_curve_<YYYY-MM-DD>/` com MANIFEST, checksums, aggregated.csv, 10 PDFs.

### TASK-0720 — D-014 Stage 6 Follow-up Bug Fixes
`docs/backlog/TASK-0720-d014-followup-bug-fixes.md`

Corrigiu 6 bugs críticos/altos pós-QA: BUG-001 (`println!` em dispatch), BUG-002 (schema mismatch writer vs. plotter), BUG-003 (VmHWM contamination Fix-B = warn em vez de reset), BUG-004 (SIGINT trap + lockfile), BUG-005 (matplotlib check em full mode), BUG-006 (drop `cv_above_gate`). Criou IT `tests/d014_writer_to_plot_roundtrip.rs`.

### TASK-0721 — D-015 Stage 6 REFACTOR
`docs/backlog/TASK-0721-d015-stage6-refactor.md` — escopo de Encoder/Horner, não D-014.

### TASK-0722 — D-014 Follow-up #2: Real Benchmark Data
`docs/backlog/TASK-0722-d014-followup-2-real-benchmark-data.md`

Corrigiu BUG-A (`println!` em `suite.rs:948`) e BUG-B (dispatch sintetizava zeros em vez de usar dados reais de `run_benchmark_suite`). Adicionou campo `bench_results: Vec<BenchmarkResult>` em `RepResult`. Plumbou `SuiteResult.results` de volta através de `run_one_sequence`. Commitado em `f0b0c7d`.

---

## 2. Inventário do que está Shipped

### `relativist-core/src/bench/memory_probe.rs`
(source: `relativist-core/src/bench/memory_probe.rs`, L1-357)

**Shipped:** `MemoryProbe::new()` com branch `#[cfg(target_os = "windows")]` chamando `GlobalMemoryStatusEx` para total RAM e `GetProcessMemoryInfo` para working set. Linux: `/proc/self/status` e `/proc/meminfo`. macOS: retorna `Err`.

**Problema crítico (F3):** `MemoryProbe` funciona corretamente em Windows puro. Mas o script `stress_curve.sh` é bash, rodando no Git Bash do Windows. Os processos filho (`target/release/relativist.exe`) são binários Windows. Em teoria, `GetProcessMemoryInfo` deveria funcionar. A evidência empírica (`vmrss_peak_mb = 0.000000` em todas as 960 linhas) indica que `peak_bytes()` retornou 0 em produção. Hipótese primária: o `GetProcessMemoryInfo` retornou `Ok` mas `PeakWorkingSetSize = 0` porque o processo é um child de curta duração e o pico ainda não foi registrado pelo kernel no momento da leitura. Hipótese alternativa: o processo corre em modo de emulação onde as APIs Win32 retornam 0 neste contexto. **Sem investigação adicional não é possível confirmar qual hipótese é verdadeira.**

(source: `relativist-core/src/bench/suite.rs`, L1201-1213)

O call site em `run_one_sequence`: `probe.peak_bytes().unwrap_or(0)` — silencia erros retornando 0.

### `relativist-core/src/bench/stop_rule.rs`
(source: `relativist-core/src/bench/stop_rule.rs`, L115-178)

**Shipped:** Lógica de `check` correta — prioridade `Oom > MemoryExceeded > WallTimeExceeded`. Condição `MemoryExceeded`: `vmrss_peak_fraction_of_total > memory_fraction_max`. Com `vmrss_peak_fraction_of_total = 0.0` (porque `peak_bytes() = 0`), a condição NUNCA dispara durante a run. `RepResult.bench_results: Vec<BenchmarkResult>` adicionado por TASK-0722 BUG-B (L58-73).

**Consequência:** `StopRule::check` nunca retorna `Some(MemoryExceeded)` para reps em que a memória realmente explodiu. A run avança para N=1B onde o kernel mata o processo com OOM (exit code diferente de zero), aí sim a regra `Oom` pode disparar — mas somente se o script captura o exit code corretamente.

### `relativist-core/src/bench/suite.rs` — `StressCurveDescriptor`
(source: `relativist-core/src/bench/suite.rs`, L1035-1225)

**Shipped:** `run_one_sequence` constrói `MemoryProbe::new()` UMA VEZ por call (não por rep), invoca `stop_rule.run_sequence` que chama o closure N vezes, e o closure chama `run_benchmark_suite` passando `reps_u32` como `repetitions`. O campo `bench_results` recebe `suite.results` (TASK-0722 BUG-B fix, L1201-1204).

**Problema (TASK-0702 Implementation Hint #4 vs. reality):** `run_one_sequence` recebe `reps` do dispatch, e passa esse valor como `repetitions: reps_u32` para `run_benchmark_suite`. Então se o CLI passar `--reps 5`, a suite roda 5 repetições internas POR N. O script deveria sempre chamar `--reps 1`, mas usa `--reps "$REP"`.

(source: `relativist-core/src/bench/suite.rs`, L948)

```
tracing::info!(
    bench = ?bench_id,
    size = size,
    description = %bench.describe(size),
    "running benchmark"
);
```

TASK-0722 BUG-A corrigiu o `println!` para `tracing::info!` nesta linha. A linha está correta no código atual.

### `relativist-core/src/bench/csv.rs`
(source: `relativist-core/src/bench/csv.rs`, L84-143)

**Shipped:** Header de 32 colunas: `benchmark,input_size,mode,workers,...,vmrss_peak_mb,vmrss_current_end_mb,stop_reason`. `cv_above_gate` removido (TASK-0720 BUG-006). Formato: `{:.6}` para `vmrss_peak_mb` e `vmrss_current_end_mb`, `{}` para `stop_reason` (vazio quando `None`).

### `relativist-core/src/commands.rs` — dispatch stress-curve
(source: `relativist-core/src/commands.rs`, L300-405)

**Shipped:** O dispatch em `run_bench_command` chama `StressCurveDescriptor::run_one_sequence`, itera `outcome.completed_reps`, itera `rep.bench_results`, e chama `write_csv_detail` — emitindo dados reais. Anota `stop_reason` no último row do `last_attempted_n` (L377-390). `tracing::info!` para o summary (L398-403). Não usa `println!`.

### `scripts/stress_curve.sh`
(source: `scripts/stress_curve.sh`, L1-402)

**Shipped:**
- Lockfile com SIGINT/SIGTERM trap corretamente encaminhado ao child (`$REP_PID`), L109-136.
- Pre-condition gate com matplotlib check em full mode (TASK-0720 BUG-005), L153-183.
- HEADER_WRITTEN logic para eliminar headers duplicados em invocações subsequentes, L227-280.
- Resume via associative array `DONE`, L211-225.

**Bug Remanescente (F1/F5):** linha 260:
```bash
--reps "$REP" \
```
O design doc especifica `--reps 1` (uma rep por child). O script passa `--reps "$REP"` onde `REP` é o índice da iteração outer (1..5). Para REP=1: binary roda 1 rep interno. Para REP=5: binary roda 5 reps internos. Efeito total: para cada N e cada WK, o script faz 5 chamadas ao binary com reps 1,2,3,4,5 respectivamente — total de `1+2+3+4+5=15` reps internas por N por WK ao invés de 5.

**Bug Remanescente (F2):** As linhas de tracing WARN aparecem no CSV (stdout), mas os arquivos `.stderr` são 0 bytes para reps que não são OOM. Isso indica que o tracing está escrevendo para stdout (fd 1) neste ambiente (Windows .exe rodando sob Git Bash). A causa raiz pode ser: (a) `tracing_subscriber::fmt::layer()` sem `with_writer()` — usa `std::io::stderr()` por padrão — mas no contexto de pipe do Git Bash, o fd 2 pode estar conectado ao stdout do pai, ou (b) o binário Windows mapeia stderr para stdout no contexto de subshell do Git Bash. A fix requer adicionar `with_writer(std::io::stderr)` explicitamente em `init_tracing`, ou redirecionar `2>&1` antes do pipe e usar um separador diferente.

### `scripts/plot_stress_curve.py`
Não lido em detalhe. TASK-0720 atualizou `REQUIRED_COLUMNS` para alinhar com o schema do writer (Path A). Não faz parte da falha da run de coleta.

---

## 3. Matriz Prometido × Shipped × Quebrado na Run Real

| Feature | Prometido | Shipped? | Quebrado na Run 2026-05-14? |
|---|---|---|---|
| `MemoryProbe::new()` sucede em Windows | TASK-0700 AC-1 | Sim (código presente) | **Sim (F3):** `peak_bytes()` retorna 0 em todas as 960 linhas; `vmrss_peak_mb = 0.000000` em todo CSV |
| `MemoryProbe::peak_bytes()` > 0 após alocação | TASK-0700 AC-2 | Unitariamente OK | **Sim (F3):** em produção retorna 0; causa desconhecida (timing ou API win32 no contexto do child process) |
| `StopRule` para memória quando > 80% RAM | TASK-0701 AC-2 | Sim (lógica correta) | **Sim (F3 consequência):** nunca dispara pois `vmrss_peak_fraction = 0.0`; stop rule de memória morta |
| `StopRule` para wall > 5 min | TASK-0701 AC-1 | Sim | **Parcial:** lógica correta mas nunca disparou na run (nenhum `WallTimeExceeded` no CSV) |
| CSV header único (HEADER_WRITTEN logic) | TASK-0704 | Sim (código presente) | **Sim (F2):** 152 headers duplicados no CSV; causa: linhas de tracing WARN que chegam ao stdout antes do header na 2a invocação em diante corrompem a lógica |
| Sem tracing no CSV (stdout reservado para dados) | TASK-0722 BUG-A | Sim (`println!` removido) | **Sim (F2):** 147 linhas de tracing WARN no CSV; `tracing::warn!` em `partition/helpers.rs:699` vai para stdout no ambiente Windows + Git Bash |
| `vmrss_peak_mb` populado no CSV | TASK-0703 | Sim (campo presente) | **Sim (F3):** `0.000000` em todas as linhas |
| `stop_reason` populado ao atingir limite | TASK-0701 + TASK-0720 | Sim (anotação no dispatch) | **Parcial (F5):** 20 rows com `MemoryExceeded` anotados retroativamente pelo dispatch via `last_attempted_n` — mas a anotação é por N que foi tentado, não por RAM real; todos os 20 são ep_annihilation N=316227766 |
| Todos os 3 workloads rodando | TASK-0704 design | Sim (loop implementado) | **Sim (F1):** apenas `ep_annihilation` completou; `dual_tree` e `condup_expansion` nunca iniciaram (tempo esgotado) |
| 5 reps por (workload, W, N) | TASK-0704 spec | Sim (outer loop 1..5) | **Sim (F5):** cada `REP=k` na iteração outer invoca o binary com `--reps k` produzindo k reps internas; total é 15 reps por N por WK, não 5 |
| `correct = true` em todas as linhas | TASK-0708 AC-4 | — | **Parcialmente (F4):** `correct = true` em 100% das linhas, mas o correctness check é trivialmente verdadeiro para N > ~6M com W > 1 devido ao WARN `to_dense allocation cap hit; returning empty Net` |
| MANIFEST.md, checksums.sha256, aggregated.csv, 10 PDFs | TASK-0708 AC-1 | — | **Sim:** 0 desses artefatos existem; run foi abortada antes da Phase 3 (aggregation) |
| dual_tree e condup_expansion CSV | TASK-0708 scope | — | **Sim:** 0 linhas para esses workloads; run abortada |

---

## 4. Mapa de Bugs F1-F5

### F1 — Apenas `ep_annihilation` rodou; `dual_tree` e `condup_expansion` nunca iniciaram

**Causa raiz:** Não é um bug de roteamento. O script itera workloads corretamente. A causa é o tempo: `ep_annihilation` com 4 workers × 10 N-values × 5 outer-REP-iterations, onde cada iteração REP=k invoca o binary com `--reps k`, levou 9h33min sozinho.

**Arquivo + linhas responsáveis:**
- `scripts/stress_curve.sh`, L260: `--reps "$REP" \` — deveria ser `--reps 1`
- `scripts/stress_curve.sh`, L240-243: loop `for WL ... for WK ... for N ... for REP in $(seq 1 "$REPS")`

**Fix mínimo:** Alterar L260 de `--reps "$REP"` para `--reps 1`. O script já tem o outer loop `for REP in $(seq 1 "$REPS")` para iterar 5 vezes; cada iteração deve chamar o binary com `--reps 1` para produzir exatamente 1 row por call.

**Impacto na wall time esperado:** Com `--reps 1`, o custo total é `5 calls × 1 rep` em vez de `15 reps` por N por WK. Isso reduz o total de 3.3× — de ~9h para ~2.7h para `ep_annihilation` apenas. O full campaign (3 workloads) deve ficar dentro das 7-8h projetadas.

---

### F2 — CSV com 152 headers duplicados + 147 linhas de tracing WARN intercaladas

**Causa raiz (duas sub-causas independentes):**

**F2a — Headers duplicados (152):**
- `scripts/stress_curve.sh`, L227-280: lógica `HEADER_WRITTEN`.
- `HEADER_WRITTEN=0` inicialmente; após a 1a invocação bem-sucedida, `HEADER_WRITTEN=1` (L280).
- Invocações subsequentes usam `| tail -n +2 >>"$RAW_CSV"` para strip do header.
- Mas: quando a invocação `| tail -n +2` processa um output que começa com uma linha de tracing WARN (não com o header CSV), a linha WARN fica no CSV como se fosse o "header que foi stripped". A 2a linha (o header real) passa pelo `tail -n +2` como uma linha de dados.
- Resultado: headers aparecem como linhas de dados no meio do CSV.
- **Fix:** Redirecionar stdout do binary para um arquivo temporário; extrair apenas linhas que são CSV válido (começam com benchmark name ou com "benchmark,"); concatenar no RAW_CSV. Alternativamente: usar `grep -v "^benchmark,"` na passagem para remover headers antes de concatenar, e manter o header separado.

**F2b — Linhas de tracing WARN no CSV (147):**
- `partition/helpers.rs`, L695-700: `tracing::warn!("build_subnet_sparse: to_dense allocation cap hit; returning empty Net")` — emitido quando `arena_len > 16777216` (16M × 1 byte = 16MiB threshold).
- `relativist-core/src/observability/tracing_init.rs`, L58-69: `tracing_subscriber::fmt::layer()` sem `.with_writer(std::io::stderr)` explícito — usa o default.
- No ambiente Windows + Git Bash, o tracing-subscriber escreve para stdout (fd 1) em vez de stderr (fd 2). Evidência: 214 arquivos `.stderr` no diretório, todos 0 bytes para reps que não são OOM; mas 147 linhas de tracing aparecem no CSV (capturado de stdout).
- **Fix:** Em `tracing_init.rs` L62-68, adicionar `.with_writer(std::io::stderr)` explicitamente ao `tracing_subscriber::fmt::layer()`. Isso garante que tracing sempre vá para fd 2, independente de plataforma. (source: `relativist-core/src/observability/tracing_init.rs`, L58-69)

---

### F3 — `vmrss_peak_mb = 0.000000` em todas as 960 linhas válidas

**Causa raiz:**
- `relativist-core/src/bench/suite.rs`, L1201-1204: `probe.peak_bytes().unwrap_or(0)` — silencia o erro ou o valor 0 retornado.
- `relativist-core/src/bench/memory_probe.rs`, L204-225: `read_windows_pmc()` chama `GetProcessMemoryInfo` com `GetCurrentProcess()`. O pseudo-handle é válido, mas `PeakWorkingSetSize` reporta 0 quando o processo é um filho novo que ainda não foi scheduled ou quando o Working Set Counter ainda não foi incrementado.

**Hipóteses para o 0:**
1. O binary roda tão rapidamente (sub-segundo para N pequeno) que o kernel ainda não registrou o peak.
2. Nos processos filho do Git Bash no Windows 11, `GetCurrentProcess()` pode retornar um handle que aponta para o processo errado em contexto de emulação POSIX.
3. A build `--release` com otimizações pode fazer o Working Set nunca chegar a ser contabilizado se a alocação é efêmera.

**Fix mínimo:** Em `run_one_sequence` (suite.rs L1203): logar um aviso quando `peak_bytes()` retorna Ok(0); adicionar um fallback que lê `peak_memory_during_construction` do `BenchmarkResult` (campo já populado, e que em Windows usa `sample_vmhwm` do legacy `bench/memory.rs`). Se esse fallback também for 0, emitir `tracing::warn!` para que o operador saiba que a métrica é inválida. **Não** usar `unwrap_or(0)` silencioso.

---

### F4 — `correct = true` silencioso para N > ~6M com workers > 1

**Causa raiz:**
- `relativist-core/src/partition/helpers.rs`, L687-701: quando `sparse.to_dense(Some(id_range))` falha com `DenseAllocationExceedsThreshold { arena_len, max=16777216, live_count }`, a função retorna `Net::new()` (subnet vazia) e emite `tracing::warn!`.
- O worker recebe uma subnet vazia; reduz ela (não há nada a reduzir); devolve subnet vazia.
- O correctness check em `bench/suite.rs` compara a subnet reduzida pelo worker com a subnet reduzida sequencialmente (benchmark).

**Por que `correct = true`:** O benchmark sequential (`ep_annihilation`) com N=10M produz a normal form correta. O distributed mode com workers > 1 produz subnet vazia per worker. O merge de subnets vazias resulta em... 0 interactions. Mas o `correct` check é `bench.verify(first_net, &reduced_net)` (L918-929 de suite.rs), comparando a **reduced net distribuída** com a **reduced net sequencial** (baseline). Se a subnet distribuída é vazia, o verify falha — mas as linhas CSV mostram `correct = true`, sugerindo que o verify passa mesmo assim.

**Provável mecanismo:** O verify de `ep_annihilation` verifica que os nós finais são todos ERA nodes (vacuous form). Se a merge produz 0 agents (porque todas as subnets são vazias), uma rede com 0 ERA nodes satisfaz "todos os nós são ERA" trivialmente (conjunto vazio). Isso é um false positive da verificação.

**Linha do WARN (evidência):**
```
2026-05-14T11:03:15.591053Z WARN ... partition::helpers: build_subnet_sparse: 
  to_dense allocation cap hit; returning empty Net arena_len=20000000 max=16777216 live_count=2500000
```
`arena_len=20000000` com `max=16777216` (SPEC-22 R22 threshold) para N=10M com W=2 workers (cada worker recebe ~5M agents, `arena_len = 2 × live_count`).

**Fix mínimo:**
1. Quando `to_dense` retorna `DenseAllocationExceedsThreshold`, em vez de retornar `Net::new()`, a função deveria retornar `Err` explícito que o `run_benchmark_suite` possa detectar e setar `result.correct = false`.
2. Alternativa mais simples: o dispatch de `--campaign stress-curve` deveria verificar se `agent_count_at_construction_complete > 0` nas rows. Se 0, o operador sabe que a run foi inválida.
3. Para o SPEC-22 R22 threshold (16MiB): considera aumentar `MAX_ARENA_FOR_DENSE_CONVERSION` ou garantir que `StressCurveDescriptor` force `representation = Dense` explicitamente para N > threshold.

(source: `relativist-core/src/partition/helpers.rs`, L680-708; `relativist-core/src/bench/suite.rs`, L918-929)

---

### F5 — StopRule semi-quebrada (dispara MemoryExceeded por razão errada, nunca dispara WallTimeExceeded)

**Causa raiz (F5a — MemoryExceeded falso positivo):**
- `relativist-core/src/commands.rs`, L377-390: o dispatch anota `stop_reason = "MemoryExceeded"` no último row do `last_attempted_n`. Mas esse `last_attempted_n` é determinado pelo `StopRule::run_sequence` que nunca disparou `MemoryExceeded` (pois vmrss = 0). Como o stop_reason veio de... 

Relendo o código: `outcome.stop_reason` é `None` quando nenhum stop rule disparou. O dispatch anota apenas quando `outcome.stop_reason` é `Some`. Então de onde vêm os 20 `MemoryExceeded`?

Olhando os dados: os 20 rows com `MemoryExceeded` são todos `ep_annihilation, 316227766, sequential, 0, {rep}`. O `run_one_sequence` interno chamou `run_benchmark_suite` para N=316227766, que por sua vez chama `MemoryProbe::peak_bytes()` — que retorna 0. Com `vmrss_peak_fraction = 0.0`, a `MemoryExceeded` nunca dispara.

**Reavaliação:** O N=316227766 para `ep_annihilation` com workers > 1 claramente usa memória (evidenciado pelos stderrs de `memory allocation of 268435456 bytes failed` para W=2, N=316M). Para esses casos, o processo provavelmente abortou com panic (não OOM-killer), retornando `ChildExit::NonZero { code: 1 }` — que o stop rule NÃO trata como OOM.

**Mas o script não usa `ChildExit` do StopRule!** O script captura o exit code (L283: `EC=$?`) e só loga um WARN. O `StopRule` no path do script nunca é consultado diretamente; o `StopRule` é consultado apenas dentro de `run_one_sequence` (Rust), que usa o exit code sintetizado internamente (L1202-1210 de suite.rs). Quando `run_benchmark_suite` retorna `Err`, o código sintetiza `ChildExit::NonZero { code: 1 }` — que NÃO é um dos `OOM_EXIT_CODES` ([137, -1073741801]).

**Como os 20 rows `MemoryExceeded` apareceram:** Provavelmente o `run_benchmark_suite` PASSOU para N=316M mas ficou lento (>5min), fazendo `stop_rule.check` disparar `WallTimeExceeded`... mas os rows dizem `MemoryExceeded`. Ou: o stop_reason foi anotado no row errado.

**Fix F5a:** Clarificar no dispatch que `stop_reason` em um row individual reflete o stop da sequência N, não o motivo da falha do rep individual. Adicionar log detalhado de qual razão disparou qual N.

**Causa raiz (F5b — `WallTimeExceeded` nunca aparece no CSV):**
O script não tem wall budget por rep: cada rep roda até completar (ou OOM). A `StopRule` wall budget é avaliada dentro do Rust path (`run_one_sequence`), que usa `start.elapsed()` para medir o wall do `run_benchmark_suite`. Para N=31622776 (31M), W=1 sequential, o wall foi 41-55s (< 300s budget). Para N=316227766 (316M), W=1, o wall foi 41-55s também — então o stop rule de wall NÃO dispara (5 min = 300s). A run foi deixada correr sem stop rule efetivo porque a métrica de memória estava zerada e o wall de reps individuais não ultrapassou 5 min.

(source: `relativist-core/src/bench/stop_rule.rs`, L120-143; `relativist-core/src/bench/suite.rs`, L1193-1221; `relativist-core/src/commands.rs`, L377-390)

---

## 5. Achados Surpresa

### S1 — `--reps "$REP"` é um bug de design não diagnosticado no QA/Review anterior

A TASK-0704 especifica "each child invokes `relativist-bench ... --reps 1 --n-target N`" mas o script implementou `--reps "$REP"`. Isso não foi capturado em TASK-0720, TASK-0722, nem nos integration tests. O test de smoke (`d014_stress_curve_smoke.rs`) verifica apenas que o CSV existe e tem `>= 2 rows` — não verifica que o número de rows é exatamente o esperado.

**Impacto:** A run de 2026-05-14 teve 960 rows de dados (ep_annihilation apenas), quando deveria ter `4 WKs × 10 N × 5 reps = 200 rows` para ep_annihilation. O excesso de rows (960/200 = 4.8×) reflete a combinação de `--reps "$REP"` e o fato de que cada binary call com `--reps k` produz `2k` rows para mode=Local (seq baseline + local) e `k` rows para mode=Sequential.

**Teste ausente:** Nenhum dos 6 integration tests de TASK-0707 verifica `SequenceOutcome.completed_reps.len() == expected_count` para um caso de produção (todos usam N pequenos e não verificam contagem total).

### S2 — `MemoryProbe` VmHWM limitation (Fix-B) mascara falhas silenciosas em produção

TASK-0720 BUG-003 adotou Fix-B: "emite `tracing::warn!` em vez de reset". O warning está corretamente implementado em `commands.rs` L326-334. Mas o warning vai para... stdout (pelo F2b). E mesmo que fosse para stderr, o operador não verifica stderrs de reps bem-sucedidas.

**Consequência:** A recomendação Fix-A (reset via `prctl(PR_SET_MM_HWM_RESET)` no Linux) seria necessária para uma leitura correta, mas está fora do escopo atual. No Windows não existe equivalente direto. O path correto para Windows seria usar `EmptyWorkingSet` antes de cada rep — não implementado.

### S3 — A run QUASE tinha dados utilizáveis para ep_annihilation W=1

Para ep_annihilation W=1 (mode=Sequential), as métricas principais (`wall_clock_secs`, `total_interactions`, `mips`) estão populadas e corretas. `vmrss_peak_mb = 0` mas as outras métricas são válidas. O N sweep completo (N=10k a N=316M) foi coberto para W=1 — 10 N-values × 5-outer-iterations × 1-rep-internal = 50 rows (mas com reps variadas: 1,2,3,4,5 internas = 15 rows com repetition indices 0..4).

Para uma análise parcial salvagável: filtrar `mode=sequential, workers=0, repetition=0` dá 10 rows (uma por N) com wall_clock_secs válido e total_interactions válido. Isso permite plotar a curva `wall_time(N)` para W=1 sequential ep_annihilation — com a caveat que vmrss = 0.

### S4 — `to_dense` threshold de 16MiB é um limite de design que afeta qualquer N > ~8M com W=2

O threshold `max = 16777216` em `partition/helpers.rs` vem de SPEC-22 R22 `MAX_ARENA_FOR_DENSE_CONVERSION`. Para `ep_annihilation N=10M` com `W=2`, cada partição tem ~5M agents, `arena_len = 2 × 5M = 10M > 16MiB`. Isso significa que a campanha com `workers > 1` **sistematicamente** retorna subnets vazias para N >= ~8M, e `correct = true` é um false positive sistemático nesse range.

Isso é um achado científico relevante para o TCC: o threshold de 16MiB para conversão dense limita o range útil da campanha distribuída. Documentar em `docs/benchmarks/limitations.md`.

### S5 — 214 stderrs × 0 bytes são dados de proveniência perdidos

O design doc §5 especifica que stderrs capturam logs forenses para diagnóstico noturno. Com stderrs todos 0 bytes (exceto OOM), a run não tem logs de diagnóstico para os 186 reps não-OOM. Isso é consequência direta do F2b (tracing vai para stdout). Qualquer run futura sem corrigir F2b perde toda a informação de tracing.

### S6 — O script em produção (sem --smoke) NÃO foi validado pelos integration tests

O test `d014_stress_curve_smoke.rs` usa `--smoke --no-docker`. A run real usa `--smoke 0` (full mode). Os bugs F2b (tracing no stdout) e F5 (--reps $REP) não são detectáveis em modo smoke porque: o smoke usa apenas N=[1000, 10000] que é muito menor que o threshold 16MiB para F4/F2b, e usa `--reps 1` implicitamente via smoke.

---

## Fontes Primárias

| # | Arquivo | Relevância |
|---|---|---|
| 1 | `docs/backlog/TASK-0700..0708.md` | Especificação de promessas D-014 |
| 2 | `docs/backlog/TASK-0720-d014-followup-bug-fixes.md` | Stage 6 bug fixes |
| 3 | `docs/backlog/TASK-0722-d014-followup-2-real-benchmark-data.md` | BUG-A + BUG-B corrigidos |
| 4 | `relativist-core/src/bench/memory_probe.rs` | Implementação MemoryProbe (L1-357) |
| 5 | `relativist-core/src/bench/stop_rule.rs` | StopRule + RepResult (L1-374) |
| 6 | `relativist-core/src/bench/suite.rs` | StressCurveDescriptor (L1035-1225) |
| 7 | `relativist-core/src/bench/csv.rs` | CSV writer schema (L84-143) |
| 8 | `relativist-core/src/commands.rs` | Dispatch stress-curve (L300-405) |
| 9 | `relativist-core/src/partition/helpers.rs` | to_dense cap WARN (L680-708) |
| 10 | `relativist-core/src/observability/tracing_init.rs` | Tracing init sem with_writer (L58-69) |
| 11 | `scripts/stress_curve.sh` | Orchestrator com --reps $REP bug (L240-280) |
| 12 | `results/locked/v2_stress_curve_2026-05-14/raw/in_process.csv` | CSV da run falha |
| 13 | `docs/superpowers/specs/2026-05-05-stress-test-large-nets-design.md` | Design doc original |

---

## Conexões Não-Óbvias

1. **F3 (vmrss=0) causa F5 (stop_rule ineficaz) causa F1 (run sem fim):** Os três não são bugs independentes em produção. A run sem fim (9h33min) é consequência direta de: vmrss=0 → stop_rule de memória nunca dispara → nenhum N é abortado precocemente → o script itera todos os N para todos os WKs de ep_annihilation.

2. **F2b (tracing no stdout) causa F2a (152 headers duplicados):** As linhas WARN que chegam ao stdout fazem o `tail -n +2` strip a linha errada. Se o tracing fosse para stderr, o `tail -n +2` removeria apenas o header real, e o CSV teria apenas 1 header.

3. **F4 (correctness silencioso) é uma manifestação de um bug de design de SPEC-22 R22 pré-existente:** O threshold de 16MiB para `to_dense` foi introduzido em D-009 como proteção contra DoS de alocação (QA-D009-005 fechado). Para a campanha stress-curve com N grandes, esse threshold torna-se um limite funcional que não foi previsto quando D-014 foi desenhado. O design doc §4.1 diz "geradores `ep_annihilation`, `dual_tree`, `condup_expansion` (versões streaming)" sem mencionar o threshold. A campanha assume que o distributed path funciona corretamente para N arbitrário — o que não é verdade.

4. **O --reps $REP bug (F1/F5) não foi detectado nos testes porque os tests usam n_seq_override com N pequenos:** Os 6 integration tests (TASK-0707) e o smoke test usam N=[1000, 10000] onde a run é sub-segundo. Com `--reps 5` (REP=5) e N=1000, o binary roda em <100ms — o smoke passa. O bug só se manifesta em produção com N grande onde cada rep leva minutos.

---

## Lacunas Identificadas

1. **Não existe teste que verifica a contagem exata de rows no CSV final** contra o número esperado para uma matrix `(workloads × WKs × N_values × 5 reps)`. A implementação atual de todos os integration tests verifica apenas "CSV exists" e "has >= 2 rows".

2. **O fix para F2b (tracing no stdout)** requer adicionar `.with_writer(std::io::stderr)` em `tracing_init.rs`. Não foi implementado em nenhum TASK até agora — é um novo bug.

3. **O fix para F3 (vmrss=0 no Windows)** não tem um TASK aberto. TASK-0722 fechou BUG-B mas não investigou por que `GetProcessMemoryInfo` retorna 0 em produção. Precisa de diagnóstico com `println!` temporário (violação do CLAUDE.md, mas necessário para debugging) ou de um teste específico que afirme `peak_bytes() > 0` após uma alocação de 100MiB no processo filho.

4. **O WARN `to_dense allocation cap hit` (F4) é um achado científico não documentado:** Para a campanha distribuída, N_max efetivo com W > 1 é ~8M (não 10⁹ como aspirado). Isso precisa ser documentado em `docs/benchmarks/limitations.md` e, potencialmente, em `docs/benchmarks/campaigns/stress-curve.md` como "Known Limitation L8".

5. **Não foi pesquisado** se existe um TASK aberto para aumentar o threshold `MAX_ARENA_FOR_DENSE_CONVERSION` ou torná-lo configurável via CLI. Se existir, pode ser pré-requisito para a campanha.

6. **Os runs anteriores (2026-05-06 e 2026-05-13)** em `results/locked/v2_stress_curve_2026-05-06/` e `results/locked/v2_stress_curve_2026-05-13/` não foram lidas neste briefing — podem conter dados parciais úteis para comparação ou para `--resume`.
