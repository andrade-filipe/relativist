# 4 — Aritmetica Church

O subcomando `compute` codifica numeros naturais como **Church numerals** em IC, reduz a expressao e decodifica o resultado de volta para inteiro. E o jeito mais pedagogico de ver uma reducao IC produzindo "um numero de verdade".

```bash
relativist compute <OPERACAO> <A> <B> [--workers N]
```

> Pre-requisito: voce sabe o que e `relativist local` ([03 — Grid Local](03-local-grid.md)), porque `compute` aceita `--workers` e roda o grid internamente.

## 4.1 Operacoes disponiveis

| Operacao | Formula   | Exemplo                     | Status    |
|----------|-----------|-----------------------------|-----------|
| `add`    | `a + b`   | `compute add 3 5` → 8       | estavel   |
| `mul`    | `a * b`   | `compute mul 3 4` → 12      | estavel   |
| `exp`    | `a ^ b`   | `compute exp 2 3` → (sem decode) | limitado |

### Nota sobre `exp`

A reducao termina corretamente — a rede alcanca a forma normal. Mas o resultado usa uma **forma compartilhada ciclica** (DUP sharing) que o decoder atual (`decode_shared_chain`) nao sabe caminhar. Limitacao conhecida de readback em reducao otima. Detalhes em [benchmarks/limitations.md](../benchmarks/limitations.md) (item **L5**).

## 4.2 Exemplos

### Adicao sequencial

```bash
relativist compute add 3 5
```

```
=== Relativist Compute ===
Expression:  add(3, 5)
Encoding:    29 agents, 1 redexes
Reduction:   6 interactions in 0.00s (0.88 MIPS)
Result:      8
```

### Multiplicacao sequencial

```bash
relativist compute mul 3 4
```

```
=== Relativist Compute ===
Expression:  mul(3, 4)
Encoding:    23 agents, 1 redexes
Reduction:   9 interactions in 0.00s
Result:      12
```

### Adicao distribuida (in-process, 2 workers)

```bash
relativist compute add 10 20 --workers 2
```

```
=== Relativist Compute ===
Expression:  add(10, 20)
Encoding:    73 agents, 1 redexes
Reduction:   6 interactions in 0.00s
Workers:     2
Rounds:      1
Result:      30
```

### Exponenciacao (limitacao)

```bash
relativist compute exp 2 3
```

```
=== Relativist Compute ===
Expression:  exp(2, 3)
Encoding:    17 agents, 1 redexes
Reduction:   15 interactions in 0.00s
Result:      (non-decodable normal form)
  Final agents: 7
```

A forma normal esta correta, mas o decoder nao consegue extrair o inteiro `8` por conta do DUP sharing.

### Multiplicacao com saida e metricas

```bash
relativist compute mul 5 6 --workers 4 -o result.bin -m metrics.json
```

`result.bin` e a rede reduzida; `metrics.json` e identico ao do `local` ([03 — Grid Local](03-local-grid.md#33-salvar-resultado-e-metricas)).

## 4.3 Como funciona por dentro

`compute add a b` internamente:

1. Constroi `encode_nat(a)` e `encode_nat(b)` como Church numerals em IC.
2. Conecta os dois numerais pelo operador `add` (tambem codificado como rede IC).
3. Chama `reduce_all` (se `--workers` nao foi passado) ou `run_grid` (se foi).
4. Aplica `decode_nat_or_shared` sobre a rede reduzida.

A especificacao formal da codificacao esta em `docs/specs/SPEC-14-encoding.md`.

## 4.4 Por que demonstrar com Church

Church numerals sao o teste mais rigoroso da pilha de reducao IC:

- **Usam as 6 regras.** `mul` combina γ-γ + γ-δ + δ-δ; `add` usa quase tudo.
- **Exigem sharing (`δ`).** Sem DUP funcional nao ha como reduzir `mul` em IC.
- **Validam o decoder.** Se o resultado decodificado bate com a formula fechada, a pilha inteira (encode → reduce → decode) esta correta.

O benchmark `church_sum_of_squares` (ver [campanha](../benchmarks/campaigns/church-sum-of-squares.md)) estende a ideia para somar `Σi²`.

---

**Proximo guia →** [05 — Modo Distribuido TCP](05-distributed-tcp.md): subir coordinator e workers em processos/maquinas separados.
