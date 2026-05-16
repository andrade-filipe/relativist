# Stress Test — Curva de Escalonamento do Relativist v2

**Data:** 2026-05-05
**Branch:** `feature/stress-and-encoder` (já criada localmente a partir de `v2-development @ 70edb5d`)
**Tópico:** 1 de 2 (este). Tópico 2 = Encoder/Decoder API (sub-projeto separado, design próprio após este fechar).
**Política de merge:** branch → `main` direto após aprovação explícita do usuário (desvia de "tudo via v2-development" do CLAUDE.md por decisão consciente do usuário).

---

## 1. Contexto

O Relativist v1 não conseguia rodar redes grandes — o coordenador materializava a rede inteira antes de particionar e estourava memória. O v2 (D-010 / SPEC-21) entregou streaming generation + streaming partitioning, o que destrava o coord. **Mas cada worker ainda materializa sua partição inteira em RAM antes de reduzir** — não há streaming reduction (decisão (α): não shipar; ROADMAP §2.16 documenta as 4 opções avaliadas com custo).

O TCC precisa de evidência empírica de até onde o Relativist v2 escala com a infra atual e onde, exatamente, a parede aparece. Este sub-projeto entrega essa evidência como **uma campanha de benchmark estruturada** que produz curvas log-log de tempo, throughput e memória sob 3 workloads complementares, em 2 ambientes, varrendo 4 contagens de worker. Os dados viram figuras na Seção 5 do `artigo/tcc_pt_br.tex`.

**Pergunta de pesquisa:** Como o Relativist v2 escala em `wall_time(N)`, `MIPS(N)` e `VmRSS_peak(N)` conforme `N` (tamanho da rede) e `W` (workers) variam ao longo de várias ordens de grandeza, sob workloads de natureza diferente (paralelo perfeito, profundidade serial, crescimento explosivo)?

## 2. Decisões fechadas no brainstorming

| # | Decisão | Valor |
|---|---|---|
| Goal | Tipo de stress test | (C) Curva de escalonamento — contém recorde + análise de gargalo como subprodutos |
| Env | Onde rodar | (D) Local in-process + Docker phase-2; Phase-3 LAN deferido |
| Streaming reduction | Shipa? | (α) Não. Caracterizar parede; documentar como future work em ROADMAP §2.16 |
| Workloads | Quais geradores | (C) Os 3: `ep_annihilation`, `dual_tree`, `condup_expansion` |
| W | Contagem de workers | `{1, 2, 4, 8}` — sweep completo em ambos os ambientes |
| N | Limite superior | Sem teto explícito; geometria ×√10; stop rule decide o teto real |
| Reps | Repetições por ponto | 5; CV ≤ 5% como flag (não como filtro) |
| Stop rule | Aborta sequência quando | wall > 5 min (in-process) ou 7m30s (Docker), OU RAM > 80%, OU OOM |
| Merge | Destino | `main` direto após aprovação |

## 3. Critérios de sucesso

1. Curvas log-log de `wall_time(N)`, `MIPS(N)` e `VmRSS_peak(N)` para cada `(workload, env, W)` — mínimo 4 ordens de grandeza em N.
2. Identificação precisa da parede: `N_max` por `(workload, env, W)` + `StopReason`.
3. Speedup absoluto reportável (W=2/4/8 vs W=1) por workload e por ambiente.
4. Dataset reproduzível, congelado em `results/locked/v2_stress_curve_<YYYY-MM-DD>/` com MANIFEST + SHA-256.
5. Zero regressão dos pisos: 1798 default / 1842 zero-copy / 1789 streaming-no-recycle / 1740 release / 690 v1 floor.

## 4. Arquitetura

Isto é **campanha de benchmark + metodologia**, não feature de sistema. Nenhuma mudança em `net/`, `reduction/`, `partition/`, `merge/` ou wire protocol. **Nenhum SPEC novo** — vive em `bench/`, `scripts/`, `docs/benchmarks/campaigns/`, e novos testes.

### 4.1 Reaproveitado (zero código novo)

| Componente | Onde mora |
|---|---|
| `bench/suite.rs` matriz + agregadores | `relativist-core/src/bench/suite.rs` |
| `--chunk-size`, `--recycle-policy`, `--streaming-strategy` | CLI já em produção |
| Geradores `ep_annihilation`, `dual_tree`, `condup_expansion` (versões streaming) | `relativist-core/src/io/generators.rs` |
| Lock-and-manifest pattern (D-012) | `results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md` como template |
| `scripts/bench_docker_v2.sh` como template de orchestration | `scripts/` |
| Métricas D-012 (network_time MAX, compute_time MAX, mips de per-rep total) | `relativist-core/src/protocol/coordinator.rs`, `relativist-core/src/merge/grid.rs`, `relativist-core/src/bench/suite.rs` |

### 4.2 Construído novo (~920 LoC distribuídos em 7 arquivos)

| Componente | Caminho | LoC | Papel |
|---|---|---|---|
| Campaign descriptor `stress-curve` | `relativist-core/src/bench/suite.rs` (extensão) | ~80 | Define matriz `workloads × envs × W × N_seq × reps` |
| `MemoryProbe` | `relativist-core/src/bench/memory_probe.rs` (novo) | ~120 | VmRSS atual + pico (Linux: `/proc/self/status`; Windows: `GetProcessMemoryInfo`); fração de RAM total |
| `StopRule` | `relativist-core/src/bench/stop_rule.rs` (novo) | ~90 | Aborta sequência N quando rep anterior bate uma das 3 condições; emite sentinel row |
| CSV schema estendido | `relativist-core/src/bench/csv_writer.rs` (extensão) | ~30 | Colunas novas: `vmrss_peak_mb`, `vmrss_current_end_mb`, `stop_reason`, `cv_above_gate` |
| `scripts/stress_curve.sh` | `scripts/` | ~150 | Orquestra Fase 1 (in-process) + Fase 2 (Docker via compose `bench-tcp` profile); pré-condições; `--resume` |
| `scripts/plot_stress_curve.py` | `scripts/` | ~200 | 9 PDFs (3 workloads × 3 métricas) IEEE-ready + `summary_walls.pdf` |
| `docs/benchmarks/campaigns/stress-curve.md` | `codigo/relativist/docs/benchmarks/campaigns/` | ~250 md | Metodologia + comandos para reproduzir |

### 4.3 Interfaces críticas

```rust
// relativist-core/src/bench/memory_probe.rs
pub struct MemoryProbe { /* opaque, platform-specific */ }
impl MemoryProbe {
    pub fn new() -> Result<Self, BenchError>;
    pub fn current_bytes(&self) -> Result<u64, BenchError>;
    pub fn peak_bytes(&self) -> Result<u64, BenchError>;     // VmHWM / PeakWorkingSetSize
    pub fn as_fraction_of_total(&self, bytes: u64) -> f64;
}

// relativist-core/src/bench/stop_rule.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopReason { WallTimeExceeded, MemoryExceeded, Oom }

pub struct StopRule { wall_budget: Duration, memory_fraction_max: f64 }
impl StopRule {
    pub fn check(&self, last_rep: &RepResult) -> Option<StopReason>;
    pub fn run_sequence<F>(&self, n_seq: &[usize], runner: F) -> SequenceOutcome
        where F: FnMut(usize) -> RepResult;
}
```

Justificativa para `peak_bytes` usar `VmHWM` / `PeakWorkingSetSize`: a métrica é process-wide e monotônica não-decrescente. Para esta campanha (1 processo `bench` por rep, reset entre reps via processo filho), ela mede o que queremos: pico do rep. O failure mode documentado em D-011/D-012 (sparse-vs-dense indistinguíveis no mesmo processo) **não se aplica aqui** porque comparamos pico entre `(workload, W, N)` distintos, não entre representações no mesmo processo.

### 4.4 Campaign descriptor

```text
campaign: stress-curve
matrix:
  workload: [ep_annihilation, dual_tree, condup_expansion]
  env:      [in_process, docker_tcp]
  workers:  [1, 2, 4, 8]
  n_seq:    [10_000, 31_623, 100_000, 316_228, 1_000_000,
             3_162_278, 10_000_000, 31_622_776, 100_000_000,
             316_227_766, 1_000_000_000]   # ×√10; teto aspiracional —
             # stop rule fecha a sequência muito antes de alcançar 10⁹ em qualquer
             # hardware razoável; valores acima existem só para que a parede não seja
             # artificialmente o último item do array.
  reps:     5
  cv_gate:  0.05  (flag, não filtro)
features: [--release, --recycle-policy disable-under-delta,
           --streaming-strategy round-robin, --chunk-size 1000]
metrics:  [wall_time_ns, mips, vmrss_peak_mb, vmrss_current_end_mb,
           network_time_ns, compute_time_ns, bytes_per_round_avg,
           all_correct, stop_reason, cv_above_gate]
```

## 5. Procedimento experimental

`scripts/stress_curve.sh` orquestra:

**Pré-condições (script aborta se falhar):**
- Branch limpa; testes verdes em todos os 5 perfis (default/zero-copy/streaming-no-recycle/release/v1-floor); clippy limpo; Docker disponível (se braço Docker ativado); RAM ≥ 8 GiB (warning < 16); diretório destino ainda não existe; CPU governor não está em low-power (Linux) ou plano de energia não é Power saver (Windows); `df --output=avail` ≥ 10 GiB.

**Fase 1 — In-process** (12 sequências = 3 workloads × 4 W):
- Cada rep em **processo filho separado** (`Command::spawn`) para garantir VmHWM zerado.
- StopRule decide quando interromper a sequência N daquele `(workload, W)`.

**Fase 2 — Docker** (mesma matriz, só roda se Fase 1 ok):
- Pre-step: `docker compose run --rm gen` produz `data/input_<workload>_<N>.bin`.
- Profile `bench-tcp` reaproveitado (já em produção).
- Stop rule **wall-budget × 1.5 (= 7m30s)**; threshold de RAM (80% do `MemTotal` físico, **excluindo swap**) e detecção de OOM permanecem **idênticos** ao in-process. RAM medida como soma dos RSS de todos os containers do compose.

**Fase 3 — Agregação:**
- Concat CSVs → `aggregated.csv`.
- `plot_stress_curve.py` emite 9 PDFs + `summary_walls.pdf`.
- SHA-256 de tudo + `MANIFEST.md` (D-012 pattern).
- **NÃO commita; espera revisão do usuário.**

**Tempo estimado:** ~3h in-process + ~4.5h Docker = ~7-8h overnight.

## 6. Outputs e artefatos

> **Convenção de nomenclatura:** `<YYYY-MM-DD>` em todos os caminhos abaixo é resolvido em runtime pelo `scripts/stress_curve.sh` para a data ISO da execução. Múltiplas execuções produzem múltiplos diretórios (não há sobrescrita).

```
results/locked/v2_stress_curve_<YYYY-MM-DD>/
├── MANIFEST.md
├── README.md
├── raw/
│   ├── in_process.csv
│   ├── docker_tcp.csv
│   └── env.txt          # uname/cargo/rustc/meminfo/cpuinfo
├── aggregated.csv
├── figures/
│   ├── ep_annihilation_walltime.pdf
│   ├── ep_annihilation_mips.pdf
│   ├── ep_annihilation_vmrss.pdf
│   ├── dual_tree_walltime.pdf
│   ├── dual_tree_mips.pdf
│   ├── dual_tree_vmrss.pdf
│   ├── condup_expansion_walltime.pdf
│   ├── condup_expansion_mips.pdf
│   ├── condup_expansion_vmrss.pdf
│   └── summary_walls.pdf
└── checksums.sha256
```

**Atualizações de docs do Relativist após campanha:**
- `docs/INDEX.md` — entrada nova "Benchmark Results > Stress Curve (v2)"
- `docs/benchmarks/campaigns/stress-curve.md` — metodologia (escrita antes, validada pela campanha)
- `docs/ROADMAP.md` §2.16 — linha de status referenciando este design e a parede caracterizada
- `docs/next-steps.md` — bundle "D-014 Stress Curve Campaign"; move pra `progress.md` ao fechar
- `CHANGELOG.md` — entrada em `[Unreleased]`

**Handoffs para TCC root** (pós-campanha, sessão separada):
- REDATOR: incorpora figuras + texto de análise no `artigo/tcc_pt_br.tex` Seção 5
- DEBATEDOR: atualiza `discussoes/argumentos/ARG-004-viabilidade-limites-praticos.md` com números empíricos da parede

## 7. Riscos & limitações

**Riscos de execução (mitigados):**
- Battery saver / thermal throttle → script verifica governor + loga
- VmHWM não reseta → cada rep em processo filho
- Docker host noise em W=8 → asterisco na legenda das figuras
- OOM killer antes do probe → sentinel row com `stop_reason=Oom` em `Command::wait()` SIGKILL
- `condup_expansion` explosivo → esperado e desejado, vira dado
- CV > 5% em N pequeno → flag não filtro; plot pinta diferente
- Variância entre runs → governor + processos paralelos documentados no MANIFEST
- Geradores estourando disco → guard adicional `df --output=avail`, aborta < 10 GiB livre
- Falha overnight no meio → `--resume` continua de onde parou (padrão `bench_docker_v2.sh`)
- Smoke run obrigatório antes do full (1 rep, N até 100k, ~15min) para validar pipeline

**Limitações estruturais (a campanha NÃO resolve, e o TCC declara):**
1. Teto N nunca passa do worker memory cap = `(N/W) × sizeof_agent`. Caracteriza, não remove.
2. Não fala sobre WAN.
3. Não fala sobre falhas de worker.
4. Speedup absoluto W=1→W=8 mistura overhead de setup com paralelismo verdadeiro.
5. Métricas de pico de memória são monotônicas (não trajetória).
6. Docker pode estourar antes do in-process (overhead de container come RAM).
7. 3 workloads não esgotam o universo.

## 8. Estratégia de testes

**Pirâmide:**
- ~12 unit tests em `relativist-core/src/bench/...`
- ~6 integration tests em `relativist-core/tests/stress_curve_*.rs`:
  (a) memory probe vs oracle 100 MiB; (b) stop rule wall; (c) stop rule ram; (d) stop rule oom (SIGKILL); (e) `--resume`; (f) end-to-end smoke (1 workload, W=2, N=[1k,10k], 1 rep)
- ~3 properties (proptest opcional): stop monotônico, CV não-negativo, agregador determinístico

**Não-regressão (pré-condição do script):**
```bash
cargo test --release             # ≥1740
cargo test                       # ≥1798 default
cargo test --features zero-copy  # ≥1842
cargo test --features streaming-no-recycle  # ≥1789
cargo clippy --all-features -- -D warnings  # 0
cargo fmt --check                            # 0 diff
```

**Sanity checks pós-agregação (manual):**
- `mips` ep_ann in-process W=1 N grande → plateau próximo do baseline v1 (~10-30 MIPS)
- `wall_time` ep_ann slope log-log ≈ 1
- `vmrss_peak` dual_tree W fixo slope ≈ 1; N fixo slope ≈ -1
- Speedup `W=4/W=1` ep_ann ≈ 2.5-3.5×
- `network_time / wall_time` Docker ≈ 30-60% (consistente com baseline post-D-012)
- `all_correct = true` em 100% dos reps (qualquer false → para tudo, vira QA)

## 9. Pipeline SDD aplicado

| Stage | Agente | Output |
|---|---|---|
| 1. SPLITTING | `task-splitter` | TASKs em `docs/backlog/`: NNN1 memory probe, NNN2 stop rule, NNN3 campaign descriptor, NNN4 CSV schema, NNN5 script + plot, NNN6 docs page, NNN7 campaign run + lock |
| 2. TESTS | `test-generator` | TEST-SPECs em `docs/tests/` por TASK |
| 3. DEV | `developer` | Implementação TDD; único agente que escreve código |
| 4. REVIEW | `reviewer` | Review unificado (quality + arquitetura) |
| 5. QA | `qa` | Adversarial em StopRule + MemoryProbe |
| 6. REFACTOR | `developer` | Aplica fixes do QA |

Após 6 stages verdes + campanha rodada + aprovação do usuário → merge da `feature/stress-and-encoder` direto pra `main`.

## 10. Política de commits

1. Branch: `feature/stress-and-encoder` (já criada)
2. Commits por etapa (não monolítico):
   (a) memory probe + tests
   (b) stop rule + tests
   (c) campaign descriptor + integration test
   (d) CSV schema extension
   (e) script + plot generator
   (f) docs/benchmarks/campaigns/stress-curve.md
   (g) campanha rodada — diretório locked + atualizações em INDEX/ROADMAP/next-steps/CHANGELOG
3. Push + merge → `main` apenas com aprovação explícita do usuário

## 11. Fora de escopo

- Streaming reduction (qualquer opção A/B/C/D) — ROADMAP §2.16 future work
- Phase-3 LAN bench
- Encoder/Decoder API (Topic 2 — design próprio)
- FENNEL streaming strategy axis
- `--use-zero-copy` axis
- Profiling de chamadas (perf/flamegraph) — manual fora do harness
- Comparação com HVM2/Bend/Haskell prototype
- Power profiling
- Editar `OBJETIVO_TCC.md`, `artigo/tcc_pt_br.tex`, `discussoes/...` (delegado para REDATOR/DEBATEDOR via TCC root)
- SPEC novo no `relativist/specs/`
- Tag de release durante esta entrega

## 12. Verificação end-to-end

Após implementação e antes da campanha real:

```bash
cd codigo/relativist

# 1. Pisos de teste verdes
cargo test --release
cargo test
cargo test --features zero-copy
cargo test --features streaming-no-recycle
cargo clippy --all-features -- -D warnings
cargo fmt --check

# 2. Smoke run da campanha (~15 min)
scripts/stress_curve.sh --smoke

# 3. Inspeção do output do smoke
ls results/locked/v2_stress_curve_smoke_*/
cat results/locked/v2_stress_curve_smoke_*/MANIFEST.md
# Confere: 1 workload × 1 W × 2 N × 1 rep = 2 linhas no CSV; 1 figura PDF

# 4. Campanha completa (overnight)
scripts/stress_curve.sh   # ~7-8h

# 5. Inspeção da campanha completa
ls results/locked/v2_stress_curve_<YYYY-MM-DD>/figures/
# Espera: 9 PDFs + summary_walls.pdf
cat results/locked/v2_stress_curve_<YYYY-MM-DD>/MANIFEST.md
# Espera: provenance completa (git SHA, cargo/rustc, /proc/meminfo, /proc/cpuinfo,
#         comando exato, total reps, total wall time, %CV mediano, contagem de stop_reason)

# 6. Sanity checks manuais (conforme tabela na seção 8)
# Aprovação do usuário → merge para main
```

## 13. Próximos passos

1. Você revisa este doc e aprova ou pede revisões
2. Brainstorm Topic 2 (Encoder/Decoder) — sub-projeto separado, design próprio
3. Após Topic 2 também aprovado → invocar `superpowers:writing-plans` ou disparar `task-splitter` do Relativist (decisão do usuário) para gerar TASKs SDD por sub-projeto
