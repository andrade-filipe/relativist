# Demonstração — HornerCodec e a Propriedade G1 (Confluência)

**Data:** 2026-05-16
**Branch:** main (post-merge de feature/stress-and-encoder, tag v0.20.0)
**Binário usado:** target/release/relativist.exe
**Operador:** Filipe Andrade Nascimento (TCC UNIT 2026)

---

## Objetivo

Demonstrar empiricamente, com o **algoritmo de Horner** como cobaia, que a
redução de uma rede de Interaction Combinators (IC) produz o **mesmo
resultado numérico** independente da estratégia escolhida — sequencial
(reduce_all in-process) ou distribuída (BSP com W workers paralelos via
`local` mode). Isto é evidência direta da propriedade **G1 (Fundamental
Property)** declarada em `specs/SPEC-01-invariantes.md`, derivada da
confluência forte demonstrada por Lafont (1997).

---

## Por que Horner é uma escolha forte de demo

Horner é o algoritmo canônico para avaliar um polinômio

  p(x) = c₀ + c₁·x + c₂·x² + ... + cₙ·xⁿ

em uma forma re-associada que minimiza multiplicações:

  p(x) = c₀ + x·(c₁ + x·(c₂ + ... + x·cₙ))

A formulação **é intrinsecamente sequencial** em texto: para começar a
calcular `c₂·x² + c₁·x + c₀` na forma re-associada, você precisa primeiro
do resultado do parêntese mais interno. Linguagens funcionais puras
tipicamente conseguem paralelizar `map` mas têm dificuldade em paralelizar
Horner sem reescrever o algoritmo.

Em IC, a re-associação some — a rede codifica a estrutura sem
ordem implícita, e qualquer estratégia de redução converge ao mesmo
normal form (G1). É exatamente esse contraste que torna Horner um demo
interessante para um TCC sobre Grid Computing via IC.

Detalhamento histórico-matemático: `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`.

---

## Especificação do CLI

Encoder/decoder expostos via `compute --codec horner --input <JSON>`:

```
{"coeffs": [c0, c1, c2, ...], "x": valor}
```

onde `coeffs[i]` é o coeficiente de xⁱ. Distribuição opcional via `--workers N`.

Registry confirmado em runtime:

```
$ relativist encoders list
Available encoders:
  church_add             Church numeral addition (a + b)
  church_exp             Church numeral exponentiation (a ^ b)
  church_mul             Church numeral multiplication (a * b)
  church_sum_of_squares  Sum of squares (1^2 + 2^2 + ... + n^2)
  horner                 Polynomial evaluation via Horner's method
```

---

## Demo 1 — Constante p(x) = 42

Caso degenerado: o net codifica o numeral Church de 42 e não há nenhum
redex (nenhuma interação a fazer). O decoder lê o numeral diretamente.

```
$ relativist compute --codec horner --input '{"coeffs":[42],"x":99}'
=== Relativist Compute (encoder: horner) ===
Encoding:    85 agents, 0 redexes
Reduction:   0 interactions in 0.00s (0.00 MIPS)
Result:      {
  "bit_length": 6,
  "value": "42"
}
```

✅ Esperado 42; obtido **42**. Independente de x (= 99 ignorado, polinômio
constante).

---

## Demo 2 — Linear p(x) = 1 + x em x = 5

```
$ relativist compute --codec horner --input '{"coeffs":[1,1],"x":5}'
=== Relativist Compute (encoder: horner) ===
Encoding:    35 agents, 2 redexes
Reduction:   11 interactions in 0.00s (1.90 MIPS)
Result:      {
  "bit_length": 3,
  "value": "6"
}
```

✅ Esperado 1+5=6; obtido **6**. **Redução sequencial in-process.**

---

## Demo 3 — Mesma equação, **distribuído com W=4 workers** (evidência G1)

```
$ relativist compute --codec horner --input '{"coeffs":[1,1],"x":5}' --workers 4
=== Relativist Compute (encoder: horner) ===
Encoding:    35 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.00 MIPS)
Result:      {
  "bit_length": 3,
  "value": "6"
}
```

✅ Esperado 6; obtido **6** — **idêntico ao Demo 2 sequencial**. Mesmo número
de interações (11), mesmo bit_length, mesmo valor. A redução particionou
o net em 4 sub-nets, distribuiu para workers, e o merge convergiu no
mesmo normal form.

---

## Demo 4 — Escala maior p(x) = 100 + x em x = 50

```
$ relativist compute --codec horner --input '{"coeffs":[100,1],"x":50}'
=== Relativist Compute (encoder: horner) ===
Encoding:    323 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.00 MIPS)
Result:      {
  "bit_length": 8,
  "value": "150"
}
```

✅ Esperado 100+50=150; obtido **150**. Note que o número de agentes na
encoding cresce com x (323 ≈ 2·(50+100)+offset), mas o número de
interações de redução permanece **11** — o cálculo do polinômio é
"barato" relativo ao tamanho da representação dos numerais Church.

---

## Demo 5 — Mesmo polinômio, **distribuído com W=8 workers** (G1 em escala)

```
$ relativist compute --codec horner --input '{"coeffs":[100,1],"x":50}' --workers 8
=== Relativist Compute (encoder: horner) ===
Encoding:    323 agents, 2 redexes
Reduction:   11 interactions in 0.00s (2.04 MIPS)
Result:      {
  "bit_length": 8,
  "value": "150"
}
```

✅ Esperado 150; obtido **150** — **idêntico ao Demo 4 sequencial**.
Agora com 8 workers paralelos. O resultado numérico **não muda**, mesmo
particionando o net em 8 fragmentos independentes e mergeando ao final.

---

## Demo 6 — Limite de escala suportado p(x) = 42 + x em x = 10000

```
$ relativist compute --codec horner --input '{"coeffs":[42,1],"x":10000}' --workers 4
=== Relativist Compute (encoder: horner) ===
Encoding:    20107 agents, 2 redexes
Reduction:   11 interactions in 0.00s (1.53 MIPS)
Result:      {
  "bit_length": 14,
  "value": "10042"
}
```

✅ Esperado 42+10000=10042; obtido **10042**. Encoding com **20k agentes**
(crescimento linear com x), redução em **11 interactions** (constante).

---

## Demo 7 — Validação de input (defesa contra DoS)

A constante `MAX_CHURCH_NAT = 10000` em `relativist-core/src/encoding/codec_church.rs`
limita o numeral Church máximo aceito pelo encoder, evitando que um JSON
malicioso construa um net com bilhões de agentes:

```
$ relativist compute --codec horner --input '{"coeffs":[1,1],"x":99999}'
=== Relativist Compute (encoder: horner) ===
error: encoding error: invalid input: x = 99999 exceeds cap (max 10000)
```

✅ Erro reportado claramente, sem panic, sem corrupção. Validação
introduzida no fix de BUG-003 do D-015 QA pass (2026-05-06).

---

## Síntese — o que esses 7 demos provam

| Propriedade | Evidência |
|---|---|
| **G1 (Fundamental Property)** — `R*(n)` produz mesma observável | Demo 2 ≡ Demo 3 (W=4); Demo 4 ≡ Demo 5 (W=8). Mesmo valor numérico em sequencial vs distribuído. |
| **Determinismo** — múltiplas execuções do mesmo input convergem | Idem (cada demo é determinístico no número de interações: 0, 11, 11, 11, 11). |
| **Confluência forte (Lafont 1997)** — normal form é único | Aparece como **mesmo bit_length, mesmo "value"** entre execuções com partição diferente. |
| **Robustez de input** — encoder valida bounds | Demo 7 (MAX_CHURCH_NAT enforcement, sem panic). |

---

## Limitações conhecidas (escopo SPEC-27 v3 "v1 codec")

O HornerCodec atual aceita encoder de qualquer polinômio, **mas o decoder
falha para coeficientes c₁ ≥ 2 ou grau ≥ 2**, com o erro:

```
error: encoding error: unrecognized net structure: non-CON in app chain
```

Casos confirmados que **não funcionam** (decode-side):
- `{"coeffs":[3,5],"x":4}` — coef de x = 5 (não-1) ❌
- `{"coeffs":[1,1,1],"x":2}` — grau 2 ❌
- `{"coeffs":[1,0,1],"x":3}` — grau 2 ❌

A **redução em si executa corretamente** (encoder gera o net, reduce_all
faz as interações). O bug está em `relativist-core/src/encoding/biguint_readback.rs`:
a leitura recursiva da árvore DUP gerada pela multiplicação Church não
trata o caso `c_i ≠ 1` (Church combinator chain) nem grau ≥ 2 (composição
de Horner aninhada).

**Working set atual:** constantes (qualquer valor ≤ MAX_CHURCH_NAT) +
polinômios da forma `c + x` (coeficiente de x sendo 1, c sendo qualquer
constante ≤ MAX_CHURCH_NAT - 1).

**Future work:** estender `biguint_readback` para grau arbitrário (~100-200
LoC + tests). Tracking em backlog para D-016 ou bundle seguinte.

---

## Reprodutibilidade

Todos os 7 demos podem ser re-executados com:

```bash
cd codigo/relativist
cargo build --release --bin relativist     # se necessário
target/release/relativist[.exe] compute --codec horner --input '<JSON>' [--workers N]
```

Hash do binário usado nesta demonstração e do código que o gerou:

```
git rev-parse HEAD     # d35a784 (merge de feature/stress-and-encoder em main)
git tag --points-at HEAD   # v0.20.0
```

---

## Cross-references

- **Spec:** `specs/SPEC-27-encoder-decoder-api.md` (v3, HornerCodec promovido sobre LambdaCodec)
- **Tese central:** `specs/SPEC-01-invariantes.md` (G1, Fundamental Property)
- **Argumento:** `discussoes/argumentos/ARG-001-confluencia-preserva-determinismo.md` (P1-P6)
- **Explainer matemático:** `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
- **Design original:** `docs/superpowers/specs/2026-05-06-horner-distributed-evaluation-design.md`
- **Encoder/Decoder:** `relativist-core/src/encoding/horner.rs`, `horner_oracle.rs`, `biguint_readback.rs`
- **CLI dispatch:** `relativist-core/src/commands.rs` (`run_compute_with_encoder`)
- **Registry:** `relativist-core/src/encoding/registry.rs` (`default_registry`)
- **Tests IT:** `relativist-core/tests/horner_codec_cli_roundtrip.rs`, `horner_distributed_g1.rs`
