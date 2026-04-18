# Demonstracao: `church_sum_of_squares`

Unico benchmark do Relativist cujo proposito e explicitamente **demonstrativo**, nao comparativo: a grid resolve o problema classico de somar quadrados inteiros

```
sum_{i=1..N} i^2 = N * (N + 1) * (2N + 1) / 6
```

(formula de Arquimedes/Faulhaber) e o resultado decodificado e conferido contra a formula fechada. O objetivo e mostrar que a plataforma IC distribuida executa um calculo aritmetico reconhecivel **de ponta a ponta**, nao medir desempenho.

## Por que nao entra em `v1_local_baseline` nem em `v1_stress`

- **Proposito ilustrativo.** O problema e ilustrativo para o texto/defesa do TCC, nao dominado por uma regra de interacao especifica que as campanhas de performance isolam.
- **Wall-clock nao comparavel.** O numero de agentes finais cresce **cubicamente** em `N`, nao linearmente, porque o resultado e `Church(sum)` e `sum` cresce como `N^3`. Tempos dele nao sao comparaveis contra `ep_annihilation`, `dual_tree`, `cascade_cross`.
- **Quadratura pre-encodada.** Decode nao-canonico por reducao otima de `mul` composto com `add` produziria nets com multiplas fronteiras DUP aninhadas que `decode_shared_chain` nao sabe caminhar. Para manter tratavel, os quadrados `i^2` sao **pre-encodados** em Rust como numerais de Church antes de serem injetados no net. A grid ainda reduz a cadeia inteira de `add` (trabalho substancial, Profile B, expansao dominante), mas a fase de quadratura e local. Ver [L5 em limitations.md](../limitations.md#l5).

## Problema matematico

Valores esperados para as `default_sizes`:

| `N` | `sum i^2`  |
|-----|------------|
| 5   | 55         |
| 10  | 385        |
| 30  | 9.455      |
| 50  | 42.925     |
| 100 | 338.350    |

## Dimensionamento do net

| `N` | Agentes iniciais (~) | Agentes apos reducao (~) | Uso tipico                  |
|-----|----------------------|--------------------------|-----------------------------|
| 5   | ~130                 | ~111                     | Unit test de smoke          |
| 10  | ~800                 | ~771                     | Demo rapida sequencial      |
| 30  | ~20 000              | ~18 900                  | Demo grid local pequena     |
| 50  | ~88 000              | ~85 800                  | Demo grid local media       |
| 100 | ~690 000             | ~676 700                 | Demo "big number", grid 8 w |

A cadeia `Church(1) + Church(4) + Church(9) + ... + Church(N^2)` pre-encoda `N` numerais (totais iniciais agregam `sum_{i=1..N} (2*i^2 + 1)`) e aplica a reducao da cadeia direita de `add`. Resultado final: `Church(sum_{i=1..N} i^2)` com agent count `~2 * N*(N+1)*(2N+1)/6 + 1`.

## Smoke sequential

```bash
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode sequential \
  --sizes 10 \
  --repetitions 1 --warmup 0 --workers 1
```

Saida esperada (trecho):

```
=== Results ===
Benchmark                   Size Workers    Time(s)     MIPS  Speedup Efficiency
--------------------------------------------------------------------------------
church_sum_of_squares        10       0   0.00000X     X.X   1.0000     1.0000

Total datapoints: 1  |  All correct: true
```

`All correct: true` confirma `decode_nat_or_shared(net) == Some(385)` (385 = `1^2 + 2^2 + ... + 10^2`).

## Smoke grid local (in-process)

```bash
# N=30, 4 workers. Resultado esperado: sum = 9_455
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode local \
  --sizes 30 \
  --repetitions 1 --warmup 0 --workers 4

# N=50, 8 workers. Resultado esperado: sum = 42_925
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode local \
  --sizes 50 \
  --repetitions 1 --warmup 0 --workers 8

# N=100, 8 workers. Demo "big number". Resultado esperado: sum = 338_350
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode local \
  --sizes 100 \
  --repetitions 1 --warmup 0 --workers 8
```

Para cada tamanho, a verificacao roda `decode_nat_or_shared(seq) == decode_nat_or_shared(dist)` e cai para `nets_isomorphic(seq, dist)` se o decode falhar. A saida imprime `All correct: true` quando a grid produziu o numero esperado.

## Smoke grid Docker (`tcp_localhost`, opcional)

```bash
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode tcp_localhost \
  --sizes 30,50 \
  --repetitions 1 --warmup 0 --workers 4
```

`N=100` **nao e recomendado** no `tcp_localhost` no primeiro pass: o net final (`~680 k` agentes) aproxima o frame cap de 1 GiB sob `bincode` v1 + `CompactSubnet` (ver [ROADMAP 2.23](../../ROADMAP.md)). Se precisar, documente a falha como limitacao conhecida e rode em `local` (in-process).

## Formato de saida

O benchmark imprime uma linha descritiva:

```
Sum of squares 1..N^2 = <valor esperado>
```

Onde `<valor esperado> = N*(N+1)*(2N+1)/6`. Exemplos:

```
Sum of squares 1..10^2  = 385
Sum of squares 1..30^2  = 9455
Sum of squares 1..100^2 = 338350
```

## Honestidade academica

Este benchmark e o **unico** do suite com proposito explicitamente demonstrativo. Ele **nao** e incluido nas campanhas congeladas `v1_local_baseline` e `v1_stress`, e **nao** deve ser usado para comparacoes de desempenho contra benchmarks estruturais. Ele existe para:

1. Produzir uma figura/demo que o leitor do TCC reconheca como *"a grid distribuida computou um numero de verdade"*.
2. Validar end-to-end que a pilha de encoding aritmetico + grid + decode funciona sob os tres modos (sequential, local, tcp_*).
