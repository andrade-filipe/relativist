# Review: `build_subnet_sparse` empty-Net fallback (cap=2^24)

**Reviewer:** REVIEWER (code-quality + architecture)
**Data:** 2026-05-14
**Sob investigação:** `relativist-core/src/partition/helpers.rs:685-708`
**Triggered by:** ≥ 100 warns no run real `v2_stress_curve_2026-05-14` (N=10^9 com W=1,2,4,8).
**Veredito:** **(a) Correctness bug** — degradado por **(b) bug de propagação**. Veja §2.

---

## Sumário Executivo

O código sob revisão é o `Err` arm do `to_dense(...)` em `build_subnet_sparse`:

```rust
// helpers.rs:685-708
let mut result_net = match sparse.to_dense(Some(id_range)) {
    Ok(net) => net,
    Err(NetError::DenseAllocationExceedsThreshold { arena_len, max, live_count }) => {
        tracing::warn!(arena_len, max, live_count,
            "build_subnet_sparse: to_dense allocation cap hit; returning empty Net");
        crate::net::Net::new()                              // <-- (1) fallback silencioso
    }
    Err(e) => {
        tracing::warn!(error = %e,
            "build_subnet_sparse: to_dense error; returning empty Net");
        crate::net::Net::new()                              // <-- (2) idem para InvalidIdRange
    }
};
```

Quando o caller é o pipeline normal (`split_with_config` → `merge::run_grid` → `BenchmarkResult`), **nenhum consumer rio-abaixo distingue uma Net vazia de "fallback" de uma Net vazia legítima**. O `BenchmarkResult.correct` é setado por `nets_match_counts(seq_result, &result_net)` ou `verify(seq, &dist)` — ambas funções **não detectam fallback**; só comparam contagens/topologias finais.

Combinado com:
- comentários no próprio site (`helpers.rs:692-694`) afirmando que o fallback "should not be reached when the caller supplied a bounded range";
- a evidência empírica de **≥ 100 acertos** na run 2026-05-14 (i.e., **é** atingido na prática para inputs grandes);
- o fato de o `ContiguousIdStrategy` produzir `id_range` que pode crescer linearmente com `net.next_id` (ver §1.3);

…é seguro afirmar que o caminho atualmente em produção pode produzir CSV rows com `correct=true` quando uma ou mais partitions foram **silenciosamente substituídas por Net vazia**. Detalhes a seguir.

---

## 1. Rastreio dos call-sites

### 1.1 Call-sites diretos de `build_subnet_sparse`

Apenas **um** call-site não-teste:

| Local | Ramo |
|---|---|
| `relativist-core/src/partition/helpers.rs:515` | dentro de `build_subnet_with_config` quando `threshold_exceeded || force_sparse_for_empty` |

Não há outros consumers — `build_subnet_sparse` é `fn build_subnet_sparse(...)` privado de módulo (`helpers.rs:589`).

### 1.2 Call-sites de `build_subnet_with_config` (o wrapper)

Em código de produção (não-teste):

| Local | Erro tratado? | Como? |
|---|---|---|
| `relativist-core/src/partition/split.rs:77-92` (chamado dentro de `split_with_config`) | **Não** — `.unwrap_or_else(|e| panic!(...))` | Comentário em `split.rs:87-92` diz "DenseAllocationExceedsThreshold is unreachable when sparse_build: true"; mas isso só vale para o `Err` retornado pelo *wrapper* — não para o cap interno em `to_dense`, que **não** é propagado pelo wrapper (já é absorvido em `build_subnet_sparse`). |
| `relativist-core/src/partition/streaming.rs` (chamada em `streaming.rs:803-834`) | parcial — propaga `PartitionError::DenseAllocationExceedsThreshold` mas, novamente, o cap interno de `to_dense` que vira `Net::new()` **não chega lá** porque `build_subnet_sparse` engole o erro. |

Conclusão crítica: **nenhum caller checa `Net::is_empty()` ou `net.agents.is_empty()` após receber o subnet**. Verifiquei com grep abrangente em `relativist-core/src/` (ver §1.5).

### 1.3 Caminho de execução depois do fallback

A Net vazia é colocada em `Partition.subnet` (`split.rs:115-122`) e segue o pipeline normal:

1. `run_grid` → `reduce_all(&mut partition.subnet)` em `merge/grid.rs:112`.
   - `reduce_all` numa Net vazia retorna `ReductionStats { total_interactions: 0, ... }`. Não falha, não loga.
2. `rebuild_free_port_index(...)` em `merge/grid.rs:123` — vazio entra, vazio sai.
3. `compute_border_activity(...)` em `merge/grid.rs:140` — retorna `false` (sem agents principais → sem border activity).
4. `merge(plan)` em `merge/core.rs:33`:
   - `max_next_id` é `max(partitions.iter().map(|p| p.subnet.next_id))` (`core.rs:69-73`). A partition vazia tem `next_id == 0` (saída de `Net::new()`). As **outras partitions** ainda têm next_id correto, então `max_next_id` é o de uma partition viva. **Resultado:** o merge consolida apenas as partitions vivas. Os agents que estavam na partition perdida desaparecem.
   - Os bordes (`borders` HashMap) referenciam IDs que pertenciam à partition perdida. Em `core.rs:238-250` o merge tenta achar dois endpoints por border id; se uma das pontas estava na partition perdida, `current_a` ou `current_b` será `None` e o pareamento desse border é simplesmente **descartado** (fall-through silencioso — nenhum erro, nenhum warn).
5. `current_net = merged_net` na linha 302; iteração continua até `redex_queue` esvaziar ou `max_rounds` esgotar.

O resultado é uma `Net` final menor do que deveria, com agents da partition empty desaparecidos.

### 1.4 BenchmarkResult.correct depois do fallback

Em `bench/suite.rs:517-521`:
```rust
let correct = if params.skip_g1 {
    nets_match_counts(params.seq_result, &result_net)
} else {
    params.benchmark.verify(params.seq_result, &result_net)
};
```

`nets_match_counts` (`bench/isomorphism.rs:450-452`) compara `HashMap<Symbol, u32>` de contagens por símbolo. **Se a Net distribuída tiver menos agents que a sequencial, `correct=false` é detectado.** Bom.

`verify` em cada benchmark (e.g. `tree_sum.rs:31-33`) chama `nets_isomorphic`. Também detectaria.

**Mas há um caso silencioso:** se o `ep_annihilation` benchmark normaliza para uma Net que (a) também colapsa para o mesmo conjunto de Symbol counts, ou (b) chega a `Normal Form` apesar de algumas partitions evaporarem — o resultado *parece* válido. Vou olhar `ep_annihilation` rapidamente: a verificação chama `nets_match_counts(&reference, &input_net)` (`suite.rs:1980` / similar), o que só pegaria contagens finais. Se o input se anihila a uma Net de poucos agentes (era_era cascata), e por coincidência o resultado distribuído também colapsa para um conjunto similar, `correct=true` mesmo com agents perdidos no meio.

**Evidência:** o CSV `results/locked/v2_stress_curve_2026-05-14/raw/in_process.csv` mostra rows `ep_annihilation,100000000,local,W,...,correct=true,...,stop_reason=MemoryExceeded` *coincidindo* com 147 warns de `allocation cap hit` no mesmo arquivo. O `correct=true` é **suspeito** mas não falsificável só pelo CSV — precisaríamos do raw G1 isomorphism check em vez de `nets_match_counts` para garantir. **Para o objetivo do TCC isso é insuficiente: os números reportados podem estar contaminados.**

### 1.5 Verificação: nenhum caller checa Net::is_empty()

Grep em `relativist-core/src/` por `net.is_empty\(\)|\.agents\.is_empty|agents\.len\(\) == 0` — só matches em **testes/asserts**, nada em pipeline de produção do `partition`/`merge`/`bench`. Caller assume que tudo o que volta de `build_subnet_with_config` é semanticamente equivalente ao recorte do net original.

---

## 2. Veredito: (a) Correctness bug + (b) bug de propagação

Classificação: **mistura de (a) e (b)**.

- É **(a) correctness bug** porque o caller (`split_with_config`) não recebe sinalização alguma de que uma partition virou vazia. Evidências:
  - `split.rs:87-92` faz `.unwrap_or_else(|e| panic!(...))` mas o `Result` aqui é `Result<Net, PartitionError>` — o cap-hit nunca aparece como `Err`. O caller acredita que recebeu uma partition válida.
  - `merge::run_grid` (`merge/grid.rs:108-152`) opera silenciosamente sobre a Net vazia e a propaga para o merged.
  - `BenchmarkResult.correct` em `bench/suite.rs:594` é resolvido por `verify`/`nets_match_counts` que podem (e na prática parecem) reportar `true` mesmo após agents desaparecerem.

- É **(b) bug de propagação** porque o autor do site claramente tinha intenção `fail-soft`: o comentário em `helpers.rs:692-694` diz textualmente "*Returning Net::new() is safer than panicking; the caller will see an empty subnet and can handle it gracefully*." — mas **nenhum caller "handles it gracefully"**. A intenção de "throttle by-design" existe; a implementação é incompleta.

Em outras palavras: o `to_dense` foi feito fallible para satisfazer a guardrail QA-D009-005 contra DoS (`sparse.rs:344-350`), mas a sua reintegração no caminho do `build_subnet_sparse` desfaz a fallibility — converte `Err` em `Net::new()` antes de o erro alcançar o pipeline. Isso anula o trabalho do guard.

**Não é (c)** — o caller não checa nem reporta. Não fui injusto.

---

## 3. Mínimo necessário para consertar (sem código)

Há duas correções complementares; a primeira é a essencial.

### 3.1 Propagar o erro até `BenchmarkResult.stop_reason`

**Onde introduzir o erro:**

1. **`partition/helpers.rs:585-596` (`build_subnet_sparse` signature):** mudar de
   ```text
   fn build_subnet_sparse(...) -> Net
   ```
   para
   ```text
   fn build_subnet_sparse(...) -> Result<Net, PartitionError>
   ```
   Adicionar variante `PartitionError::SparseAllocationCapExceeded { arena_len, max, live_count, partition_index }` em `relativist-core/src/error.rs:53-65` (mesmo padrão de `DenseAllocationExceedsThreshold`, com `#[from] NetError`).

2. **`partition/helpers.rs:512-534` (`build_subnet_with_config`, sparse arm):** mudar a chamada para `let mut subnet = build_subnet_sparse(...)?;`. Já retorna `Result<Net, PartitionError>` — o `?` propaga.

3. **`partition/split.rs:77-92`:** o `.unwrap_or_else(|e| panic!(...))` atual é hostil; é o que precisa mudar. Há duas opções:
   - **3a (preferível):** mudar a assinatura de `split_with_config` para `pub fn split_with_config(...) -> Result<PartitionPlan, PartitionError>`. O pipeline `run_grid` então recebe `Result` e tem que decidir o que fazer.
   - **3b (paliativo):** manter a assinatura mas trocar o `panic!` por um caminho onde a `PartitionPlan` carrega uma flag `had_allocation_failure: bool` e o `run_grid` aborta o round com um valor sentinel. É mais feio mas evita refator amplo em testes.

4. **`merge/grid.rs:94-99` (chamada de `split_with_config`):** se for **3a**, propagar via `?`; precisa que `run_grid` retorne `Result`. Hoje retorna `(Net, GridMetrics)` — mudaria para `Result<(Net, GridMetrics), GridError>` com `GridError::AllocationCapExceeded(PartitionError)`. Isso é **caro**: `run_grid` é chamado em dezenas de testes (`grep "run_grid(" relativist-core/src/`).

5. **`bench/suite.rs:511 (`measure_grid`)`:** o `run_grid(...)` aqui é o ponto onde a Net distribuída é produzida. Quando ele retornar erro, popular o `BenchmarkResult`:
   - `correct = false` (a partition perdida é, por definição, uma falha de correção, não de capacidade).
   - **Alternativa mais limpa:** introduzir `StopReason::AllocationCapExceeded` em `bench/stop_rule.rs:22-26`. O CSV column `stop_reason` já existe (`bench/suite.rs:449,652`) — mudança trivial. Setar `stop_reason = Some(StopReason::AllocationCapExceeded)` e `correct = false`.

**Recomendação concreta sobre o BenchmarkResult:**
- **Usar `stop_reason=AllocationCapExceeded`** (StopReason já é o vehicle canônico). É consistente com `MemoryExceeded`/`Oom`. Não preciso de campo novo.
- **Setar `correct = false`** redundantemente. É o sinal que análises a jusante (validate.rs, CSVs publicados no artigo) leem direto.

### 3.2 Tamanho estimado da mudança

| Componente | LoC aproximado |
|---|---|
| `error.rs` — nova variante PartitionError | ~10 |
| `helpers.rs:589-714` — assinatura + `?` chain | ~20 (mais comentários) |
| `split.rs:77-92` — propagar Result | ~15 |
| `merge/grid.rs:94-99` + assinatura `run_grid` | ~30–50 (sinais de retorno + tests refactor) |
| `bench/suite.rs:511, 588-653` — popular stop_reason | ~15 |
| `bench/stop_rule.rs:22-26` — nova variante StopReason | ~5 |
| Testes (atualização de signatures, novos testes para o caminho) | ~80–120 |
| **Total** | **~175–235 LoC** |

Cabe em **uma task atômica** se considerar o limite de ~200 LoC do `task-splitter` — mas vai estourar facilmente se incluir refactor dos call-sites do `run_grid` nos testes. Recomenda-se splittar em **2 tasks**:
- Task A (~100 LoC): error variant + `build_subnet_sparse` fallibility + `split_with_config` propagation + dois testes happy/sad path.
- Task B (~100–135 LoC): `run_grid` retornando `Result` + `BenchmarkResult.stop_reason=AllocationCapExceeded` + refactor de testes.

### 3.3 Alternativa, se 3a/3b for caro demais

Mínimo absoluto: **manter assinaturas, mas dentro de `build_subnet_with_config` definir um flag em `PartitionConfig::error_on_cap: bool` (default `true` em runs reais; `false` para legacy/testes).** Quando `true`, o caminho cap-hit retorna `Err(PartitionError::SparseAllocationCapExceeded{..})`. Quando `false`, mantém o `Net::new()` legacy.

Combine com **escalar o `Err` no `split.rs`** por meio de uma variante `Partition::AllocationFailed(PartitionError)` em vez de `panic!`. O `run_grid` então observa isso e setta `metrics.converged = false` + um novo `metrics.partition_failure: Option<PartitionError>`. O bench layer lê e setta `stop_reason`.

Custo: **mais baixo (~80–120 LoC)** mas paga em coerência arquitetural — você está propagando um erro por meio de uma flag em `GridMetrics`, não por `Result`. Aceitável como hotfix; **não é o end-state**.

---

## 4. Impacto retroativo nos dados existentes

### 4.1 Run 2026-05-06 (`results/locked/v2_stress_curve_2026-05-06/`)

Verifiquei `raw/in_process.csv`:
- `grep -c "allocation cap hit"` → **0**.
- `awk` em `input_size` → max **10000**.

Com `live_count = O(N)` e N ≤ 10000, `arena_len ≤ ~30k`, **muitíssimo abaixo de 16.7M**. **Não atinge o cap.** Dados desta run estão limpos com respeito a este bug.

### 4.2 Baseline `results/locked/v2_post_d012_baseline_2026-05-05/`

- `grep -c "allocation cap hit"` em `detail.csv`/`rounds.csv`/`summary.csv` → **0**.
- Max `input_size` no CSV: **5.000.000**.

Para `ep_annihilation` com N=5M e w∈{2,4,8}, cada partition tem `live_count ≈ 1.25M–2.5M`. O `effective_arena_size = max_live_id + 1`. Sob `ContiguousIdStrategy`, `max_live_id` por partition é `≈ N/w`, então `arena_len ≈ N/w + O(1)` — bem abaixo do cap de 16.7M para N=5M.

**Conclusão:** baseline `v2_post_d012_baseline_2026-05-05` **não toca o cap**. Dados limpos.

### 4.3 Run 2026-05-14 (`results/locked/v2_stress_curve_2026-05-14/`)

- `grep -c "allocation cap hit"` em `raw/in_process.csv` → **147**.
- Combinações únicas observadas (arena_len, live_count): 17.5M/2.5M, 20M/2.5M, 39.5M/7.9M, 47.4M/7.9M, 55.3M/7.9M, 63.2M/7.9M, 125M/25M, 150M/25M, 175M/25M, 200M/25M.

**Dados contaminados:** todas as rows com `input_size ≥ 10^8` (especialmente w=2, w=4) onde o cap-hit foi acionado têm `correct=true` no CSV — **mas o subnet sumiu**. Sem corrida nova, não dá pra saber se o `nets_match_counts` happens to ter passado por coincidência (e.g., ep_annihilation cascata para Net trivial) ou se há contaminação real escondida.

**Recomendação:** **invalidar todas as rows do 2026-05-14 com `input_size ≥ 31.622.776` (o limite onde cap começa a disparar — arena_len 17.5M)** até que o pipeline seja corrigido e a run seja refeita. Marcar `results/locked/v2_stress_curve_2026-05-14/MANIFEST.md` com aviso de contaminação. A run 2026-05-13 pode ter o mesmo problema — fora do scope desta review, mas verificar antes de usar.

### 4.4 690 testes v1-feature-complete

`git show v1-feature-complete:src/partition/helpers.rs | grep "build_subnet_sparse"` → **0 matches**.

O sparse path **não existe** no v1. v1 usa apenas o dense path (`build_subnet`), que sob N=5000 (sizes de teste) nunca aproxima o cap. **Floor de 690 testes é imune** a este bug. Boa notícia para o sign-off do v1.

---

## 5. O cap de 2^24 faz sentido?

### 5.1 Origem

`relativist-core/src/net/sparse.rs:285`:
```rust
pub const MAX_DENSE_ARENA_SLOTS: usize = 1 << 24; // 16_777_216
```

Comentário (sparse.rs:277-284): hardcoded constant; introduzido em QA-D009-005 como guard contra DoS de `max_id` adversarial perto de `u32::MAX`. Justificativa registrada:

> 16 million slots × 3 ports × 8 bytes/slot ≈ 384 MiB — well within process limits for test and small-scale grid runs, and safely below the 256 MiB protocol frame cap.

A relação com o frame cap de 256 MiB é frouxa: o cap **não** é o frame de wire, é o tamanho do `Vec<Option<Agent>> + Vec<PortRef>` em RAM por partition. Essas duas grandezas não têm conexão obrigatória (o frame protocol é de wire; o arena é em-memória), só são citadas juntas como benchmark de plausibilidade.

### 5.2 É hardcoded ou config?

**Hardcoded.** `pub const`, sem variable de ambiente, sem campo em `PartitionConfig` ou `GridConfig`. Mudá-lo requer recompilar.

### 5.3 Pode subir para 2^26 ou 2^28?

- **2^26 = 67.108.864 slots** → ~1.5 GiB de arena (agents + ports). Em máquinas de bench típicas (16–64 GiB RAM), aceitável. Permitiria N até ~100M com w=2 sem cap-hit.
- **2^28 = 268.435.456 slots** → ~6 GiB de arena. Em máquinas de desktop padrão, **arriscado**; ok em servidores dedicados. Permitiria N até ~10^9 com w=2.

Restrição real: o cap protege contra `max_id = u32::MAX` adversarial. Como `max_id` ainda é `u32` em produção, o cap **só pode subir até 2^32-1 = 4.29 bilhões**. Não há outro hard wall.

**Workloads que ganhariam com 2^26:**
- Todo o stress-curve 2026-05-14 acima de N=10^7 com w=2.
- LAN benchmark Phase 3 com w pequeno (1–2).
- Qualquer recipe que produza nets com next_id alto mas live_count baixo (alocação fragmentada).

**Workloads que ganhariam com 2^28:**
- N=10^9 com w=2 (atualmente impossível sem cap-hit).
- Encoder paths densos (Horner CLI) onde `next_id` cresce rápido durante construção.

### 5.4 Recomendação sobre o cap

**Não simplesmente subir.** Duas mudanças complementares:

1. **Tornar `MAX_DENSE_ARENA_SLOTS` configurável** via:
   - `PartitionConfig.dense_alloc_max_slots: Option<usize>` (default = `1<<24`, override de bench/CLI).
   - Variável env `RELATIVIST_DENSE_ALLOC_MAX_SLOTS` (parseada pelo CLI no startup).
2. **Manter o default em 2^24 (defensivo) e elevar a 2^26 apenas para bench runs explícitos.** Risco de OOM em desktop com default alto não vale o ganho.
3. **Documentar a relação com `effective_arena_size = max_live_id + 1`** — operadores precisam saber que o cap é por-arena-slots, não por live count. Comentário atual em sparse.rs:277-284 é OK mas não cita o fenômeno do `max_live_id ≫ live_count` (M5 pathology) que **multiplica** o consumo.

**Tamanho estimado:** ~25 LoC + 2 testes. Trivial.

---

## 6. Sumário das ações recomendadas (priorizado)

| # | Ação | Severidade | LoC |
|---|---|---|---|
| 1 | Propagar `Err(PartitionError::SparseAllocationCapExceeded)` em vez de `Net::new()`. (§3.1 plano A) | **Must-fix** | ~100 |
| 2 | Adicionar `StopReason::AllocationCapExceeded`; popular no `measure_grid`. (§3.1 plano A) | **Must-fix** | ~30 |
| 3 | Refazer `run_grid` retornando `Result<(Net, GridMetrics), GridError>`. | **Should-fix** | ~50 |
| 4 | Anotar `results/locked/v2_stress_curve_2026-05-14/MANIFEST.md` com aviso de contaminação. (§4.3) | **Must-fix** | ~10 (doc) |
| 5 | Verificar `v2_stress_curve_2026-05-13` para mesma contaminação. | **Must-fix** | (auditoria) |
| 6 | Tornar `MAX_DENSE_ARENA_SLOTS` configurável (`PartitionConfig`). (§5.4) | **Should-fix** | ~25 |
| 7 | Comentário sparse.rs:277-284 destacando M5 / `max_live_id ≫ live_count`. (§5.4) | **NTH** | ~5 |

---

## Checks executados

- [x] Identifiquei o único call-site não-teste de `build_subnet_sparse` (helpers.rs:515).
- [x] Verifiquei que nenhum caller checa `Net::is_empty()`/`net.agents.is_empty()` na pipeline de produção.
- [x] Confirmei que `merge::core::merge` (core.rs:69-172) absorve silenciosamente partitions vazias.
- [x] Verifiquei que `BenchmarkResult.correct` é derivado por `verify`/`nets_match_counts` sem checagem de fallback.
- [x] Contagem de cap-hits no CSV 2026-05-14: 147.
- [x] Contagem de cap-hits em 2026-05-06: 0. Max input_size = 10.000. Limpo.
- [x] Contagem de cap-hits em post_d012_baseline_2026-05-05: 0. Max input_size = 5M. Limpo.
- [x] v1-feature-complete não contém `build_subnet_sparse` — floor 690 imune.
- [x] Verifiquei que o `panic!` em `split.rs:91` discorda do `tracing::warn!` em `helpers.rs:695` — duas estratégias de fail diferentes no mesmo caminho.
- [ ] **Não verifiquei**: `v2_stress_curve_2026-05-13` (recomendada auditoria como follow-up).
