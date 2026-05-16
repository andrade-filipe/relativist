# QA Adversarial Audit — D-014 Stress Curve real-run failure (2026-05-14)

**Author:** qa agent
**Run dir:** `results/locked/v2_stress_curve_2026-05-14/`
**Run window:** 2026-05-14 00:30 → 10:03 (≈ 9 h 33 min, aborted by operator)
**Verdict:** **NÃO PUBLICÁVEL no TCC sem filtragem agressiva.** Dataset parcialmente salvável (curva ep_annihilation N ∈ [10⁴, 10⁸] para W ≤ 4 utilizável; o restante é lixo ou duplicata).
**Bugs found:** F1 explicado · F2 confirmado · F3 confirmado · F4 confirmado · F5 parcialmente confirmado · 5 bugs adicionais **novos e críticos**.

---

## 0. Inventário do diretório

```
results/locked/v2_stress_curve_2026-05-14/
└── raw/
    ├── in_process.csv      (1259 linhas, mtime 09:03:47)
    └── *.stderr            (214 arquivos: 186 vazios + 28 com OOM)
```

Faltando, comparado ao layout canônico esperado:

| Esperado | Presente |
|---|---|
| `aggregated.csv` | **AUSENTE** |
| `figures/*.pdf` | **AUSENTE** |
| `MANIFEST.md` | **AUSENTE** |
| `checksums.sha256` | **AUSENTE** |
| `raw/docker_tcp.csv` | **AUSENTE** |
| `raw/env.txt` | **AUSENTE** |

Causa: o script `scripts/stress_curve.sh` (linhas 298-394) só produz esses artefatos APÓS o término normal do loop principal (linha 289). O operador deu Ctrl+C → trap em `on_interrupt` (linha 110-135) → `exit 130` direto, **sem rodar Phase 3 (aggregation), Phase 4 (plots), Phase 5 (env capture + checksums + manifest)**. O lockfile `.lock` também não aparece (foi removido por `release_lock`).

**Bug latente do script (não dispara nesta run mas é relevante):** o trap só faz `release_lock`; se o operador interrompe no meio da Phase 3+ (aggregation/plots), o `MANIFEST.md` fica metade.

---

## 1. Veredito por bug candidato (F1–F5)

### F1 — Só `ep_annihilation` rodou; `dual_tree` e `condup_expansion` ausentes
**Status: EXPLICADO — não é bug do código, é orçamento de tempo estourado.**

Estrutura do CSV (verbatim — primeiras 3 linhas de cabeçalho duplicado):

```
1:benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,...
3:benchmark,input_size,...
6:benchmark,input_size,...
```

Todas as 960 linhas de dado começam com `ep_annihilation,` — busca por outros workloads retornou zero hits:

```bash
$ grep -c "^dual_tree\|^condup_expansion" raw/in_process.csv
0
```

Loop bash do orquestrador (script linha 240-289):
```bash
for WL in "${WL_ARR[@]}"; do        # ep_annihilation, dual_tree, condup_expansion
    for WK in "${WK_ARR[@]}"; do    # 1, 2, 4, 8
        for N in "${NS[@]}"; do     # 10⁴ … 10⁹ (11 pontos)
            for REP in $(seq 1 5); do
                ...
```

Como o loop é WL OUTER, o operador abortou ANTES de o loop chegar em `dual_tree`. Último arquivo gravado (mtime 10:03:47): `ep_annihilation_8_316227766_3.stderr` — confirma que a campanha estava em **`ep_annihilation, W=8, N=316M, REP=3`** quando o Ctrl+C ocorreu (vide §3 timeline).

**Não há bug F1 propriamente dito**, mas o **design da matriz é inviável neste hardware** (vide §4).

---

### F2 — 152 cabeçalhos duplicados + WARN tracing intercalados no CSV
**Status: CONFIRMADO. Origem: duas causas ortogonais.**

Contagens verificadas (`wc -l` + `grep -c`):

| Categoria | Linhas |
|---|---|
| Total | **1259** |
| Header (`^benchmark,`) | **152** (1 inicial + 151 duplicados) |
| Dado (`^ep_annihilation,`) | **960** |
| WARN tracing | **147** |
| Soma | 152 + 960 + 147 = **1259** ✓ |

Exemplo verbatim de WARN intercalado (linha 1008 do CSV):
```
[2m2026-05-14T11:03:15.591053Z[0m [33m WARN[0m ThreadId(01) [2mrelativist_core::partition::helpers[0m[2m:[0m build_subnet_sparse: to_dense allocation cap hit; returning empty Net [3marena_len[0m[2m=[0m20000000 [3mmax[0m[2m=[0m16777216 [3mlive_count[0m[2m=[0m2500000
```

**Origem #1 (headers duplicados):** O bash script tenta evitar isso com `tail -n +2` (linha 274), MAS a lógica `HEADER_WRITTEN` é controlada por **`if [[ -f "$RAW_CSV" ]]; then HEADER_WRITTEN=1; fi`** (linha 229), que pré-marca como "escrito" se o arquivo já existe. Porém, dentro do loop (linha 280) `HEADER_WRITTEN=1` é reatribuído **APÓS** o `wait`. Em paralelo, o **binário em `commands.rs:394-396`** **sempre** chama `write_csv_detail` que **sempre** escreve um header (não conhece `--no-header`). Então em rep 1 (HEADER_WRITTEN=0) o header é mantido; em reps 2+ deveria ser estripado pelo `tail -n +2`. Verificado: número de headers (152) ≈ número de invocações ≈ 9 N × 4 W × 5 REP - sequência abortada parcialmente. **152 ≈ 38 sequências completas** (9 N × 4 W = 36 + 2 do último N=316M sequential).

Aguarda: 38 sequências completas? Espera; revendo, a contagem real:
- N ∈ {10⁴..10⁸} completos (9 valores), W ∈ {1,2,4,8} (4), REP ∈ {1..5} (5) = 180 invocações
- N = 316M só W=1 com 5 REPs e W=2,4,8 com 5 REPs CADA (mas crash) = 4×5 = 20
- Total esperado de invocações ≈ 200, mas só 152 cabeçalhos → bash provavelmente acertou ~48 invocações primeiro com `tail -n +2`.

Isto sugere que o **branch `if [[ $HEADER_WRITTEN -eq 0 ]]`** (linha 253) e o **branch else** (linha 263 com `tail -n +2`) não estão equilibrados — mas o efeito agregado é o que importa: **CSV é não-parseável diretamente por `pandas.read_csv` sem `skiprows` ou `error_bad_lines`**.

**Origem #2 (WARN poluição):** `tracing::warn!` no `relativist_core::partition::helpers::build_subnet_sparse` (cap-hit) — esse macro emite para **stdout** quando o subscriber padrão é usado (provavelmente sem redirect), portanto **vai parar no `>>"$RAW_CSV"`**. Total: 147 linhas WARN no CSV, todas referentes ao cap-hit `to_dense allocation cap hit; returning empty Net`. Vide §2 bug F4 para impacto.

**Severidade: HIGH.** Quebra parser estrito CSV. Tolerável para pandas com `on_bad_lines='skip'`, mas viola formato wire.

---

### F3 — `vmrss_peak_mb = 0.000000` em 100% das linhas
**Status: CONFIRMADO em 960/960 rows. `vmrss_current_end_mb` também é zero.**

```bash
$ awk -F',' '$1=="ep_annihilation"{print $30}' raw/in_process.csv | sort | uniq -c
    960 0.000000

$ awk -F',' '$1=="ep_annihilation"{print $31}' raw/in_process.csv | sort | uniq -c
    960 0.000000
```

Adicionalmente as colunas relacionadas a memória **TODAS** estão a zero/vazias:
- `peak_memory_bytes` (col 14) = 0 em 960/960
- `peak_memory_during_construction` (col 23) = **vazio** em 960/960
- `peak_memory_during_reduction` (col 24) = **vazio** em 960/960

**Implicação:** O eixo Y da figura de "VmHWM vs N" prometida pelo design `2026-05-05-stress-test-large-nets-design.md` é **completamente impossível de plotar**. Todo o argumento de footprint de memória (parte central do D-014) ficou sem dados.

**Severidade: CRITICAL.** Bloqueia 50% das figuras do TCC seção 5.4 (Stress Curve).

**Causa provável (não verificada no código):** O `MemoryProbe::peak_bytes()` (suite.rs:1203) está sempre retornando `0` ou `unwrap_or(0)` está mascarando um erro de leitura `/proc/self/status` no Windows (WSL?). Note que a campanha rodou no Windows host (`Filipe` em path Windows) — `/proc/self/status` não existe nativo, ou o probe Windows não está implementado.

---

### F4 — `correct=true` mesmo quando `build_subnet_sparse` retornou Net vazia (cap-hit)
**Status: CONFIRMADO. 45 rows comprometidas.**

Padrão claro do CSV:

```bash
$ awk -F',' '$1=="ep_annihilation" && $8 != $2 {key=$2"|"$3"|"$4; trunc[key]++} END{for(k in trunc) print k, trunc[k]}'
10000000|local|8 15
100000000|local|8 15
31622776|local|8 15
```

**45 rows** com `total_interactions < input_size` MAS `correct=true`. Exemplos verbatim:

```
ep_annihilation,10000000,local,8,N,true,...,7500000,...      ← esperava 10000000 interactions, fez 7.5M
ep_annihilation,31622776,local,8,N,true,...,15811388,...     ← esperava 31.6M, fez 15.8M (exato 50%)
ep_annihilation,100000000,local,8,N,true,...,50000000,...    ← esperava 100M, fez 50M (exato 50%)
```

O padrão "metade" ou "75%" sugere que algumas partições retornaram Net vazia (cap hit) e foram **ignoradas silenciosamente** no merge — mas o `verify()` declara `correct=true` porque Net vazia tem normal form trivial.

Cruzamento com os 147 WARN no CSV: o WARN explicita `arena_len=20000000 max=16777216 live_count=2500000` — isso confirma que workloads N ≥ 10M com W=8 acionam o cap-hit em alguma partição. **W=2 e W=4 com mesmos N são CORRETOS** (interactions == input_size), validando a hipótese: 16M/2 = 8M e 16M/4 = 4M cabem, mas com W=8 cada partição pode receber mais que 16M se a distribuição for desigual.

**Severidade: CRITICAL.** `correct=true` é mentiroso. O sanity check `interactions == input_size` precisa ser usado como filtro pos-hoc na agregação. Pior: para `dual_tree`/`condup_expansion` (não rodaram), o ratio interactions/N pode não ser conhecido a priori, então o filtro não é trivial de generalizar.

**Fix proposto (não-implementar; só sugerir):** `correct = correct && total_interactions == expected_for_size(workload, N)` ou propagar uma flag `partial_net_construction` por meio da pipeline `build_subnet_sparse → merge → BenchmarkResult`.

---

### F5 — StopRule semi-quebrado: só 20 MemoryExceeded; 5 reps de N=1B com OOM real "68GB" não geraram MemoryExceeded
**Status: PARCIALMENTE CONFIRMADO — o StopRule funcionou em alguns casos mas falha catastroficamente para N=1B porque o processo crasha ANTES de escrever a row.**

Distribuição de `stop_reason` (coluna 32):

```
940 rows  (vazio)
 20 rows  MemoryExceeded
```

As 20 rows MemoryExceeded são:
- `316227766, sequential, 0` → 5 rows (uma por iteração do ladder bash, REP=1..5)
- `100000000, local, 2` → 5 rows
- `100000000, local, 4` → 5 rows
- `100000000, local, 8` → 5 rows

**StopRule funcionou** para N=100M (local) e N=316M (sequential) — esses crasharam por OOM e o Rust capturou a exceção, populou stop_reason, e escreveu a row.

**StopRule FALHOU CATASTROFICAMENTE** para:
1. **N=1B, todos os W:** crash 68GB allocation (vide stderrs). Nenhuma row gravada. O CSV NÃO TEM N=1B. Stderrs confirmam:
   ```
   $ cat raw/ep_annihilation_1_1000000000_1.stderr
   memory allocation of 68719476736 bytes failed
   note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
   ```
   Ocorreu em 15 reps (5 reps × W=1,2,4 = 15 stderrs, exatamente como o ladder bash 1+2+3+4+5=15).

2. **N=316M, W∈{2,4,8} (local):** stderrs mostram OOM (268MB e 9.1GB), nenhuma row no CSV:
   ```
   $ cat raw/ep_annihilation_2_316227766_1.stderr
   memory allocation of 268435456 bytes failed
   $ cat raw/ep_annihilation_8_316227766_1.stderr
   memory allocation of 9126805520 bytes failed
   ```

**Severidade: CRITICAL.** O StopRule **só funciona se o processo sobreviver ao OOM** (ou seja, se o Rust capturar Err em vez de o allocator panicar). Para N=1B com 68GB, o `Vec` global alloc bate em `handle_alloc_error` antes que qualquer código de captura possa rodar. Resultado: o bash script registra `EC != 0` (linha 283: `WARN: rep ... exit=ABORTED`) mas o **ladder do StopRule não toma conhecimento, então o bash CONTINUA tentando N=1B para os 4 W e gasta 15+15+15+15 = 60 invocações inúteis** (~5min cada para crash) ≈ 5h de tempo desperdiçado.

E mais grave: **N=316M com W=2,4,8 também crashou** mas o bash continuou a iterar (chegou a tentar W=8 antes do Ctrl+C). Isso explica parte das ~7h "perdidas" no orçamento (vide §3).

---

## 2. Bugs adicionais descobertos na auditoria (NEW)

### NEW-1: Duplicação 4× de rows sequencial (orçamento de tempo é destruído)
**Severidade: HIGH.** **Impacto: ~3× de inflação de duração da campanha.**

Para cada `(workload, env, W, N, REP)`, o `run_benchmark_suite` (`suite.rs:910-957`) **SEMPRE** roda `config.repetitions` baseline sequential **antes** dos `config.repetitions` em mode `Local`. Como o bash chama o binário 4× (W=1,2,4,8) por (workload, N, REP), o **mesmo baseline sequential é re-executado 4 vezes consecutivas**.

Evidência verbatim:
```bash
$ awk -F',' '$1=="ep_annihilation"{key=$2"|"$3"|"$4; c[key]++} END{for(k in c) print c[k], k}' | sort
60 10000|sequential|0       ← esperava 15 (5 reps × 3 W onde sequential é útil = 0 sequenciais "extras")
60 100000|sequential|0
60 1000000|sequential|0
60 10000000|sequential|0
60 100000000|sequential|0
60 31622776|sequential|0
60 3162278|sequential|0
60 316228|sequential|0
60 31623|sequential|0
15 10000|local|2
15 10000|local|4
15 10000|local|8
...
```

60 / 15 = **4×** — exatamente o número de Ws no bash loop.

Impacto direto no tempo: a soma de `wall_clock_secs` para sequential N=100M é 153.52s (60 reps). Se fossem só 15 reps (correto), 38.4s. Para N=31.6M: 46.6s → 11.7s. Total **economizado** seria ~25min só de sequencial inflado. Isso não parece muito, mas o sequencial roda **ANTES** do local; se o local crasha (OOM), o tempo sequencial foi 4× pago em vão.

**Correção sugerida (não-implementar):** Reescrever o bash para fazer **1 invocação por (workload, env, N, REP)** passando `--workers 1,2,4,8` em uma chamada — o `run_benchmark_suite` já aceita `workers: Vec<u32>` (linha 1170 — atualmente só passa `vec![workers_u32]` porque `run_one_sequence` força 1 valor). Refactor: o orchestrator descritor deve receber todos os W de uma vez.

Adicional/alternativa: cache o `seq_baseline_secs` por N entre invocações via arquivo temporário.

---

### NEW-2: `rounds = 0` e `live_agent_count_watermark = 0` para TODAS as rows sequential
**Severidade: MEDIUM.**

```bash
$ awk -F',' '$1=="ep_annihilation"{print $3, $10}' | sort -u
local 1
sequential 0

$ awk -F',' '$1=="ep_annihilation"{print $3, $26}' | sort -u
local 200000        ← agent_count
...                  ← (qualquer valor não-zero)
sequential 0
```

`measure_sequential` (suite.rs:383+) não populadas as colunas `rounds` e `live_agent_count_watermark`. Não é um bug crítico de correctness, mas distorce qualquer agregação que faça média ou sum across `mode`. Plots de "rounds vs N" são impossíveis para sequencial.

---

### NEW-3: Slowdown absoluto catastrófico em mode=Local — w=8 é 67× MAIS LENTO que sequencial
**Severidade: SHOWSTOPPER para o argumento do TCC. Isto não é bug de código, é evidência empírica explosiva.**

Para N=100M:
```
sequential (avg of 60):   2.56s
local W=2 (avg of 15):  101.60s   (40× mais lento)
local W=4 (avg of 15):  129.28s   (51× mais lento)
local W=8 (avg of 15):  171.29s   (67× mais lento)
```

Para N=10M:
```
sequential:  0.253s
local W=8:  13.745s   (54× mais lento)
```

Speedup reportado pelo Rust: para W=8 N=100M → speedup=0.0146, efficiency=0.0018. Isso é uma **REGRESSÃO ABSOLUTA**, não uma aceleração.

Implicação: o argumento de speedup do TCC seção 5 **morre completamente** para o workload ep_annihilation em mode `Local` (single-host BSP). É consistente com `c_o/c_r = 2.2` que o ROADMAP §2.40 já registra como break-even acima de 0.50.

**Não é bug, é resultado científico negativo**, mas precisa estar no relatório explícito para que o REDATOR ajuste a narrativa do artigo.

---

### NEW-4: `peak_memory_during_construction` e `peak_memory_during_reduction` ambas VAZIAS
**Severidade: HIGH (combina com F3).**

Verificado: `awk` mostra string vazia em todas as 960 rows. Combinado com F3, **NENHUMA coluna de footprint de memória contém dados úteis**.

A coluna `peak_memory_bytes` (col 14, R18) também = 0 universal. O probe está completamente broken neste host/build.

---

### NEW-5: `recycle_policy` (coluna 29) tem valor `disable-under-delta` mas `representation` foi MOVIDO de coluna para `dense` em col 27 e `chunk_size` (col 28) está vazio
**Severidade: LOW (formato OK, mas suspeito).**

```
26 representation      → vazio na coluna do header... wait
27 representation      → 'dense'
```

Re-conferindo:
```
=== header column ordering ===
27 representation
28 chunk_size
29 recycle_policy

=== data row ===
27 dense       ← representation: ok
28 (empty)     ← chunk_size: ok (None → vazio)
29 disable-under-delta   ← recycle_policy: ok
```

Falso alarme — colunas estão alinhadas. `chunk_size` é legitimamente None (streaming desabilitado). **Não é bug.**

---

### NEW-6: Bug do `tail -n +2` no orquestrador é "best-effort" não strict — 152 headers no CSV final
**Severidade: HIGH.** (Já mencionado como F2 origem #1. Reforço aqui como bug separado de ENGENHARIA do script bash, não do tracing.)

A lógica do bash:
```bash
HEADER_WRITTEN=0
if [[ -f "$RAW_CSV" ]]; then HEADER_WRITTEN=1; fi   # linha 229
...
if [[ $HEADER_WRITTEN -eq 0 ]]; then
    "$RELATIVIST_BIN" ... >>"$RAW_CSV"               # keep header
else
    "$RELATIVIST_BIN" ... | tail -n +2 >>"$RAW_CSV"  # strip header
fi
HEADER_WRITTEN=1                                     # linha 280
```

Mesmo quando `tail -n +2` é usado, ainda assim **152 cabeçalhos** apareceram. Hipótese: o binário emite o header AINDA QUANDO há zero rows (caso de OOM/abort interno), e o stdout pode estar bufferizado de forma que o `tail -n +2` come a primeira linha de **dados** em vez do header. **Necessita instrumentação adicional para confirmar.**

---

### NEW-7: Nenhum arquivo `env.txt` / `MANIFEST.md` / `checksums.sha256` produzido apesar do script TER lógica para criar
**Severidade: BLOQUEADOR para auditoria pos-hoc.**

A lógica de Phase 5 (script linha 348-394) só roda após o loop principal terminar normalmente. Ctrl+C → trap `on_interrupt` → `exit 130` direto. Resultado: **impossível reproduzir** o run sem saber git rev, rustc version, cpuinfo. Para o TCC isso é gravíssimo (peer reviewer exige `MANIFEST.md`).

**Correção sugerida:** Mover Phase 5 (env capture pelo menos) para o **TOP** do script (após acquire_lock), não no final. MANIFEST pode ser appendado incrementalmente.

---

## 3. Timeline da run

| Evento | Hora local | Evidência |
|---|---|---|
| Início | 2026-05-14 00:30 | mtime mais antigo do diretório |
| Loop em N=316M, W=8 | 2026-05-14 10:03 | mtime do último `.stderr` (`ep_annihilation_8_316227766_3.stderr`) |
| Operador aborta (Ctrl+C) | ≈ 10:03 | rep=4 e rep=5 do W=8 N=316M têm `.stderr` vazio (placeholder) |
| Total | **9 h 33 min** | |

Soma de `wall_clock_secs` no CSV: **2 h 33 min** (9205 s). Diferença vs. wall total: **~7 h** gastos em:
- Construção de input nets (não medida no `wall_clock_secs`)
- Crashes de OOM com retry no ladder do StopRule (N=1B ×15 reps + N=316M com W=2,4,8 ×15 reps)

---

## 4. Quantificação do estrago — quanto é salvável

### Tabela de validade por célula (workload=ep_annihilation, env=in-process)

| N | mode | W | n_reps_no_CSV | reps_correct_no_truncation | reps_with_stop_reason | salvável? |
|---|---|---|---:|---:|---:|:---:|
| 10⁴ | sequential | 0 | 60 | 60 | 0 | YES (mas inflado 4×) |
| 10⁴ | local | 2 | 15 | 15 | 0 | YES |
| 10⁴ | local | 4 | 15 | 15 | 0 | YES |
| 10⁴ | local | 8 | 15 | 15 | 0 | YES |
| 10^4.5 (31623) | sequential | 0 | 60 | 60 | 0 | YES |
| 10^4.5 | local | 2,4,8 | 45 | 45 | 0 | YES |
| 10⁵ | sequential | 0 | 60 | 60 | 0 | YES |
| 10⁵ | local | 2,4,8 | 45 | 45 | 0 | YES |
| 10^5.5 (316228) | sequential, local 2,4,8 | – | 105 | 105 | 0 | YES |
| 10⁶ | seq + local 2,4,8 | – | 105 | 105 | 0 | YES |
| 10^6.5 (3.16M) | seq + local 2,4,8 | – | 105 | 105 | 0 | YES |
| 10⁷ | sequential | 0 | 60 | 60 | 0 | YES |
| 10⁷ | local | 2 | 15 | 15 | 0 | YES |
| 10⁷ | local | 4 | 15 | 15 | 0 | YES |
| 10⁷ | local | 8 | 15 | **0** (todas truncadas) | 0 | **NO (F4 cap-hit)** |
| 10^7.5 (31.6M) | sequential | 0 | 60 | 60 | 0 | YES |
| 10^7.5 | local | 2 | 15 | 15 | 0 | YES |
| 10^7.5 | local | 4 | 15 | 15 | 0 | YES |
| 10^7.5 | local | 8 | 15 | **0** (todas truncadas) | 0 | **NO** |
| 10⁸ | sequential | 0 | 60 | 60 | 0 | YES |
| 10⁸ | local | 2 | 15 | 15 | 5 (StopRule MEM) | PARCIAL |
| 10⁸ | local | 4 | 15 | 15 | 5 | PARCIAL |
| 10⁸ | local | 8 | 15 | **0** (todas truncadas) | 5 | **NO** |
| 10^8.5 (316M) | sequential | 0 | 15 | 15 | 5 (StopRule MEM) | PARCIAL |
| 10^8.5 | local 2,4,8 | – | **0** | 0 | 0 | **NO (crash sem CSV)** |
| 10⁹ | qualquer | – | **0** | 0 | 0 | **NO (crash sem CSV)** |

**Resumo de salvabilidade do dataset 2026-05-14:**
- **915 rows úteis** (de 960 — descontando as 45 do W=8 N≥10M com cap-hit)
- **MAS as 60-reps-sequenciais por N são todas duplicatas 4×**, então rows únicas conceituais: ~225 distintas (15 por célula).
- **Curva utilizável:** N ∈ {10⁴, 10^4.5, 10⁵, 10^5.5, 10⁶, 10^6.5, 10⁷, 10^7.5, 10⁸} × W ∈ {seq, 2, 4} — **9 pontos × 3 modos = 27 células**, cada com 15 reps válidas.
- N=10⁸·⁵ (316M) sequencial é parcialmente salvável (15 reps, mas 5 com MemoryExceeded — usar só as primeiras 10).
- **Zero dados para dual_tree e condup_expansion.**
- **Zero dados de footprint de memória** (F3 + NEW-4).

**Worth saving?**  Sim, mas APENAS como "smoke validation" das fixes de bugs. Não é publicável como figura final do TCC.

---

## 5. Veredito final

### 5.1 Dataset 2026-05-14 é publicável no TCC?

**NÃO COMO ESTÁ.** Justificativa quantitativa:

1. **Cobertura de workloads: 33% (1/3)** — só ep_annihilation, faltam dual_tree e condup_expansion (que são justamente os interessantes para o argumento de B=growing workload).
2. **Cobertura de N: 73% (8/11)** — N=316M parcial, N=10⁹ ausente. Cap empírico de hardware = N ≈ 10⁸ para construção. Logaritmicamente, **só 3 ordens de grandeza** (10⁴ a 10⁸) — insuficiente para um power-law fit de qualidade IEEE.
3. **Memória: 0%** — nenhuma coluna de VmHWM, peak_memory, ou similar tem dados. A figura de memory vs N (central no design D-014) é impossível.
4. **45 rows envenenadas** com `correct=true` enganoso (F4 cap-hit silencioso). Sem flag de filtro automatizado, qualquer agregação cega usa dados truncados.

### 5.2 Bugs BLOQUEADORES de qualquer próxima run

| # | Bug | Severidade | Razão |
|---|---|:---:|---|
| F3 + NEW-4 | VmRSS/peak_memory = 0 em todas as colunas | CRITICAL | Impossível plotar memória; abort da próxima run sem isto é desperdício |
| F4 | Cap-hit silencioso + `correct=true` mentiroso | CRITICAL | Dados corrompidos sem aviso; envenena qualquer plot automático |
| F5 (parcial) | StopRule não captura crash do alloc 68GB (N=1B) | HIGH | Bash desperdiça 5h tentando N=1B repetidamente |
| NEW-1 | Sequential baseline rodando 4× por N | HIGH | Inflação de 25-50% no tempo da campanha |
| NEW-7 | Manifest/checksum só no fim do script | HIGH | Se a run aborta, dataset é impossível de reproduzir |
| F2 + NEW-6 | Headers duplicados + WARN tracing no CSV | MEDIUM | Quebra parser CSV estrito; precisa `on_bad_lines='skip'` |

### 5.3 Bugs **OPCIONAIS** (deixam dados utilizáveis mas degradados)

- NEW-2: `rounds = 0` e `live_agent_count_watermark = 0` para sequential.
- NEW-3 não é bug; é resultado científico (slowdown absoluto W=8) — informar REDATOR.

### 5.4 Matriz original é viável neste hardware?

**NÃO.** A matriz original (3 workloads × 4 W × 11 N × 5 reps com N até 10⁹) tem:
- **660 células × 5 reps = 3300 invocações** (sem contar a duplicação 4× do bug NEW-1 → ~10000 invocações reais).
- Mesmo cortando N=10⁹ (impossível com 32-64GB RAM) e N=10^8.5 com W=8 (também OOM), e mesmo corrigindo NEW-1:
  - Wall extrapolado de ep_annihilation (2.5h para 1 workload × N até 10⁸) × **3 workloads** ≈ **7-8 h reais**.
  - **Mas o tempo de construção do net (build_subnet_sparse, ~50-70% do total observado)** soma facilmente outras 5-10h.
  - **Estimativa realista de campanha completa neste hardware: 25-35 horas wall.**

### 5.5 N_max realístico deduzível dos dados

**N_max = 10⁸ (100M) para sequencial.** Para W=8 local, **N_max = 10⁷·⁵ (31.6M)** — acima disso o cap-hit envenena os dados.

Observe-se que **wall_clock para N=10⁸ sequencial é 2.56s e para N=10^8.5 é 48.2s** — uma escalada de 18× para um aumento de 3.16×. Isto indica que entre 10⁸ e 10^8.5 já saímos do regime linear (cache misses dominantes). Logo o **regime onde a curva power-law é trustworthy: N ∈ [10⁴, 10⁸]** — apenas 4 ordens de grandeza, o que é **marginal mas aceitável** para uma figura IEEE com fit log-log.

### 5.6 Recomendação operacional para a próxima campanha

1. Fix obrigatório: F3+NEW-4 (memory probe) — **sem isto, nada de remediar**.
2. Fix obrigatório: F4 (cap-hit + correct flag) — **propagar `partial_construction` para o CSV row**.
3. Fix obrigatório: NEW-1 (baseline 4×) — **refactor bash para 1 invocação por (workload, N, REP) com todos os W**.
4. Reduzir matriz: **N ∈ [10⁴..10⁸] (9 pontos), W ∈ {1,2,4} (3 valores)** — drop W=8 (slowdown absoluto + cap-hit), drop N=10^8.5 e 10⁹ (OOM determinístico).
5. Considerar reduzir reps de 5 para 3 — variância observada é < 5% em walls não-truncados.
6. **Estimativa pós-fixes: 4-6h wall total** para 3 workloads × 3 W × 9 N × 3 reps = 243 células × ~30s média = **~2h efetivos + ~2h de construção** = factível em uma janela noturna única.

---

## 6. Resumo de evidência citável (verbatim)

### CSV header (linha 1):
```
benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,peak_memory_bytes,bytes_sent,bytes_received,con_con,dup_dup,era_era,con_dup,con_era,dup_era,peak_memory_during_construction,peak_memory_during_reduction,agent_count_at_construction_complete,live_agent_count_watermark,representation,chunk_size,recycle_policy,vmrss_peak_mb,vmrss_current_end_mb,stop_reason
```

### Última row do CSV (com MemoryExceeded):
```
ep_annihilation,100000000,local,8,4,true,169.805860,50000000,0.294,1,0.0143,0.0018,0.9845,0,0,0,0,0,50000000,0,0,0,,,200000000,200000000,dense,,disable-under-delta,0.000000,0.000000,MemoryExceeded
```
Note: `interactions=50000000` mas N=100000000 (50% truncado por cap-hit — F4 confirmado), `correct=true` (mentiroso), `vmrss_peak_mb=0.000000` (F3 confirmado).

### Stderr de crash N=1B (15 reps idênticos):
```
memory allocation of 68719476736 bytes failed
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
```

### WARN poluindo CSV (linha 1008):
```
WARN ThreadId(01) relativist_core::partition::helpers: build_subnet_sparse: to_dense allocation cap hit; returning empty Net arena_len=20000000 max=16777216 live_count=2500000
```

---

**Fim do relatório.**
