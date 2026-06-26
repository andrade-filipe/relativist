# 06 · Protocolo Delta (`--delta-mode`)

Guia para o **Protocolo Delta**, especificado em [SPEC-19](../../docs/specs/SPEC-19-delta-protocol.md). O modo delta e uma alternativa v2 ao ciclo BSP full-partition de v1 — em vez de reempacotar particoes inteiras a cada rodada, workers sao **stateful** e trocam apenas **deltas de borda** com o coordinator.

> **Status:** feature-gated, opt-in. Default continua no protocolo full-partition de v1 (zero regressao). Ative apenas se souber que o caso de uso se beneficia (borders pequenas em relacao ao tamanho de particao).

## 1. Quando usar

O modo delta reduz overhead de rede em workloads onde:

- As particoes sao **grandes** (MB-GB) mas a fronteira ativa entre elas e **pequena** (centenas de portas).
- O numero de rodadas BSP e alto (cascade_cross, dual_tree), de modo que o custo de re-enviar a particao inteira a cada rodada domina.
- A rede e o gargalo (Phase 3 LAN com `t_network > t_cpu`).

Se a particao inteira cabe em poucos KB e a rodada BSP e dominada por CPU, o ganho e marginal — prefira v1 padrao.

## 2. Ideia central

### v1 (padrao) — full-partition

A cada rodada BSP:

```
coord --[AssignPartition (N bytes)]--> worker
worker reduz localmente (reduce_all)
worker --[PartitionResult (N' bytes)]--> coord
coord merge() + split() + re-enviar particoes atualizadas
```

Custo por rodada: O(N) em rede, onde N e o tamanho da particao.

### v2 delta (opt-in)

Round 0 envia a particao inicial. Nas rodadas seguintes, **workers mantem a particao em memoria** e trocam apenas o que mudou nas bordas:

```
coord -> worker: "reduza ate estabilizar"
worker --[BorderDelta lista]--> coord    (apenas as portas de borda que mudaram)
coord aplica deltas em sua BorderGraph
coord detecta redexes de borda (side_a e side_b ambos principal) -> dispatch
```

Custo por rodada: O(|deltas|), tipicamente << O(N).

## 3. Componentes

### BorderGraph (lado coordinator)

Estrutura leve que rastreia conectividade inter-particao. Para cada `border_id`, armazena os dois endpoints atuais (`side_a`, `side_b`), os worker IDs donos de cada lado, e um flag `is_redex` (verdadeiro quando ambos os lados sao portas principais).

O BorderGraph **substitui a rede mergeada completa** em rodadas 1+. Apenas no final, quando o sistema converge para Global Normal Form, o coordinator reconstroi a particao e faz `merge()` uma unica vez.

### Stateful workers

Workers nao recebem uma particao nova a cada rodada. Eles:

1. Recebem a particao uma vez (round 0).
2. Em cada rodada subsequente, rodam `reduce_all` ate estabilidade interna.
3. Computam os deltas (mudancas nas portas de borda) e os enviam ao coordinator.
4. Aguardam proxima ordem (mais redexes de borda, ou convergencia global).

### Global Normal Form (DC-C5, tres conjuntos)

Convergencia ocorre quando **todos** abaixo sao verdadeiros na mesma rodada:

1. Todos os workers reportam `zero local redexes`.
2. O `BorderGraph` tem `zero active pairs`.
3. Nenhum delta novo foi reportado nesta rodada.

Os tres conjuntos garantem que nao ha redex pendente, local ou de borda, em canto nenhum do sistema. Equivalente ao v1 "merged net com fila de redexes vazia", porem sem precisar reconstruir o net.

## 4. Uso na CLI

### Modo `local` (simulacao in-process)

```bash
relativist local --workers 4 --delta-mode -i net.bin -o out.bin
```

### Modo `coordinator` (TCP real)

```bash
relativist coordinator --workers 4 --port 9000 --delta-mode -i net.bin -o out.bin
```

Workers nao precisam de flag especifica — eles detectam o modo do coordinator via handshake.

### Programaticamente (API)

```rust
use relativist::merge::GridConfig;

let config = GridConfig {
    num_workers: 4,
    delta_mode: true,                      // ativa SPEC-19
    ..GridConfig::default()
};

let result = run_grid_delta(net, &config)?;
```

Ver `src/merge/grid.rs` para o loop BSP delta (R41 estende `GridConfig`, R42 garante default false).

## 5. Invariantes preservados (amendments do bundle 2.26)

SPEC-19 re-formula G1/D3/D6 sem enfraquece-los:

- **G1 (Amendment).** O net final apos `run_grid_delta` e isomorfo ao `reduce_all(net)` sequencial. Strong confluence (T4) garante que a decomposicao em (BorderGraph, worker_partitions) e reconstruivel no final.
- **D3 (Amendment).** Deteccao de redex de borda e incremental via `BorderGraph.detect_border_redexes()`. D3d formaliza a equivalencia: para todo border redex detectavel por `merge() + findBorderRedexes()` completo, o BorderGraph detecta o mesmo redex na mesma rodada (given correct worker delta reporting).
- **D6 (Amendment).** Terminacao usa as **tres condicoes** acima. Sem reconstruir o net ate a convergencia.

Ver [docs/reference/invariants.md](../reference/invariants.md) para a forma completa.

## 6. Limitacoes conhecidas

- **Pending formal proof.** D3d e a alma da correctness do modo delta. Ha argumento informal + testes extensivos, mas a prova formal esta listada em [SPEC-19 §8](../../docs/specs/SPEC-19-delta-protocol.md).
- **Partition migration.** Se um worker morre, recuperar sua particao exige protocolo de resincronizacao (fora do SPEC-19; coberto em [SPEC-20 Elastic Grid](../../docs/specs/SPEC-20-elastic-grid.md)).
- **Compat.** Worker e coordinator **precisam** da mesma versao. Mismatch de versao rejeita o handshake.

## 7. Proximo passo

- [SPEC-19](../../docs/specs/SPEC-19-delta-protocol.md) — especificacao formal completa (R1-R42, diagramas de mensagem, prova de convergencia).
- [docs/reference/invariants.md](../reference/invariants.md) — G1/D3/D6 com amendments.
- [07-zero-copy.md](07-zero-copy.md) — SPEC-18 wire format v2 (complementar ao delta: reduz o custo de bytes efetivamente enviados).
