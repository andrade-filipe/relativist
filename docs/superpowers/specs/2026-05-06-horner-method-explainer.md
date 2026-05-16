# Método de Horner — Explainer

**Para:** Topic 2 do brainstorm `feature/stress-and-encoder` (Encoder/Decoder API).
**Status:** Explainer/contexto. Não é design doc. O design doc do Topic 2 cita este arquivo.
**Audiência:** você (autor do TCC) + qualquer leitor do design doc que precise de fundamentação rápida.

---

## 1. O problema que Horner resolve: avaliação de polinômio

Dado um polinômio com coeficientes reais (ou inteiros) `a_0, a_1, ..., a_n` e um ponto `x`, queremos calcular o valor `p(x)`:

$$
p(x) = a_n \cdot x^n + a_{n-1} \cdot x^{n-1} + \cdots + a_2 \cdot x^2 + a_1 \cdot x + a_0
$$

**Exemplo concreto.** `p(x) = 3x³ + 2x² + 5x + 1`, em `x = 2`:

$$
p(2) = 3 \cdot 2^3 + 2 \cdot 2^2 + 5 \cdot 2 + 1 = 24 + 8 + 10 + 1 = 43
$$

Trivial. A pergunta interessante é: **como computar isso minimizando trabalho?**

## 2. Avaliação ingênua (e por que ela é ruim)

A receita "termo a termo" exige:

1. Calcular cada potência `x^k`: requer `k - 1` multiplicações por termo
2. Multiplicar pelo coeficiente: `1` multiplicação por termo
3. Somar tudo: `n` adições

Total para grau `n`:

| Operação | Custo |
|---|---|
| Computar potências `x², x³, ..., x^n` | `n - 1` mults |
| Multiplicar cada `a_k` pela potência | `n` mults |
| Somar todos os termos | `n` adds |
| **Total** | **`2n - 1` mults + `n` adds** |

Para `n = 100`, são **199 multiplicações + 100 adições**. Para `n = 10⁶`, **~2 milhões de multiplicações**. Caro.

## 3. O método de Horner

Horner observou que o polinômio pode ser **reescrito por aninhamento**:

$$
p(x) = a_0 + x \cdot \big( a_1 + x \cdot \big( a_2 + x \cdot \big( \cdots + x \cdot (a_{n-1} + x \cdot a_n) \cdots \big) \big) \big)
$$

Mesmo polinômio. Aritmética idêntica. Mas a forma de computar é diferente — você começa do **mais interno** e vai expandindo:

```
algorithm Horner(coeffs[a_0, a_1, ..., a_n], x):
    accumulator <- a_n
    for k from n-1 down to 0:
        accumulator <- accumulator * x + a_k
    return accumulator
```

**Exemplo prático**, mesmo polinômio `p(x) = 3x³ + 2x² + 5x + 1`, em `x = 2`:

| Passo | Acumulador | Conta |
|---|---|---|
| Inicial | `3` | (= a₃) |
| Iteração 1 | `3·2 + 2 = 8` | acc·x + a₂ |
| Iteração 2 | `8·2 + 5 = 21` | acc·x + a₁ |
| Iteração 3 | `21·2 + 1 = 43` | acc·x + a₀ |
| **Resultado** | **43** | ✓ |

Custo:

| Operação | Custo |
|---|---|
| Multiplicações | `n` |
| Adições | `n` |
| **Total** | **`n` mults + `n` adds** |

Para `n = 100`: **100 multiplicações + 100 adições** — quase metade do trabalho da ingênua. Para `n = 10⁶`, ~1 milhão de multiplicações em vez de 2 milhões. **Speedup ~2× pra qualquer grau.**

Adicionalmente, Horner é **numericamente mais estável** em ponto flutuante (menos operações = menos acúmulo de erro de arredondamento) e **não precisa de armazenamento extra** (uma única variável de acumulador, em vez de um array de potências).

## 4. Para que serve Horner no mundo real

Não é truque acadêmico — Horner está em todo lugar onde polinômios aparecem com grau não-trivial:

| Domínio | Onde aparece |
|---|---|
| **Processamento de sinais** | Avaliação de filtros FIR/IIR — coeficientes do filtro são um polinômio em z⁻¹; cada amostra processada é uma avaliação de polinômio |
| **Computação gráfica** | Curvas e superfícies de Bézier (B-splines) são polinômios; renderizar 1 ponto = 1 avaliação |
| **Métodos numéricos** | Newton-Raphson para encontrar raízes de polinômios usa Horner em cada iteração; integração numérica com regra de Simpson generalizada |
| **Sistemas algébricos computacionais** | Mathematica, Maple, SageMath — operações sobre polinômios usam Horner como primitiva |
| **Criptografia** | Polynomial commitment schemes (KZG, FRI) usados em provas de conhecimento zero (zk-SNARKs) avaliam polinômios de grau alto |
| **Compressão / códigos** | Reed-Solomon codes — codificação e decodificação envolvem avaliação de polinômios em pontos do corpo finito |
| **Aprendizado de máquina** | Splines polinomiais em regressão; expansões de Taylor truncadas em séries de potência |

Em qualquer biblioteca de matemática numérica (BLAS, GSL, NumPy), `polyval` é Horner. É o **default canônico** porque é mais rápido E mais preciso que a ingênua.

## 5. A propriedade crucial: Horner é fundamentalmente sequencial

Olhe o pseudocódigo de novo:

```
accumulator <- a_n
for k from n-1 down to 0:
    accumulator <- accumulator * x + a_k
```

Cada iteração precisa do **resultado da iteração anterior** (o `accumulator`). Não há como paralelizar essa estrutura — a iteração `k` precisa do `accumulator` da iteração `k+1`. Se você tentar quebrar o loop em pedaços e rodar em paralelo, **não dá**: o que você precisa pra começar a metade de cima depende da metade de baixo ter terminado.

Esta é a **definição clássica de algoritmo serial**. Em livros-texto de algoritmos paralelos, Horner é **o exemplo canônico** de "algoritmo intrinsecamente sequencial".

A maneira tradicional de paralelizar avaliação polinomial é abandonar Horner e voltar à fórmula expandida (calcular `x^k` em paralelo via *parallel prefix*, multiplicar `a_k * x^k` em paralelo, somar via *parallel reduce*). Isso recupera paralelismo, mas **paga o custo da ingênua** — `2n - 1` mults + `n` adds, perdendo o ganho original do Horner.

> **Tradeoff clássico:** ou você roda Horner serial e paga `n` mults + `n` adds; ou você paraleliza com a forma expandida e paga `2n - 1` mults + `n` adds. **Não há como ter os dois.**
>
> *(... pelo menos não com computação convencional.)*

## 6. Por que Horner é o problema perfeito pro TCC

A IC do Lafont muda essa conversa. **A confluência (Property P1, ARG-001) garante que qualquer ordem de redução produz o mesmo resultado.** Isso significa: se você encodar a expressão polinomial como uma rede IC, o motor de redução pode escolher a ordem que quiser — serial, paralela, distribuída entre N máquinas — e o resultado é igual.

A elegância da escolha do Horner pro TCC é que ele coloca essa propriedade **diretamente em conflito com a sabedoria convencional**:

| Aspecto | Mundo convencional | Mundo IC + Relativist |
|---|---|---|
| Ordem de avaliação | Imposta pelo algoritmo | Liberada pela confluência |
| Encoding | Algoritmo escolhe um custo (Horner serial OU expandida paralela) | Encoding é o mesmo (rede IC com `add`/`mul`); o motor escolhe ordem livremente |
| Custo de operações | Tradeoff `n` vs `2n-1` mults | `n` mults inerentes ao polinômio (igual a Horner) — mas executadas em ordem livre |
| Paralelização | Requer reescrita para forma expandida | Não requer reescrita — confluência cuida |

**A frase-chave da defesa do TCC:** _"O método de Horner é, na literatura, o exemplo canônico de algoritmo sequencial. Mostramos empiricamente que, ao encodá-lo como uma rede de Combinadores de Interação, a confluência (Lafont 1997) permite que o Relativist o avalie distribuidamente entre W workers e múltiplas máquinas, retornando o mesmo resultado byte-a-byte que a avaliação serial em C/Rust nativo. Isso ilustra concretamente o que ARG-001 P3 prova formalmente."_

## 7. Como Horner se encoda em redes IC (overview, não spec)

> O design doc do Topic 2 (`2026-05-06-horner-distributed-evaluation-design.md`, ainda a ser escrito) detalha a arquitetura. Aqui só o conceito.

A SPEC-14 do Relativist já entrega:

- `encode_nat(n) -> Net` — codifica um inteiro `n` como Church numeral (rede de CON/DUP/ERA)
- `build_add(net, a, b) -> AgentId` — compõe duas sub-redes Church para representar `a + b`; o redex resultante reduz para a soma
- `build_mul(net, a, b) -> AgentId` — idem, multiplicação
- (já tem também `build_exp`, mas não vamos usar — Horner não usa exp)

Para encodar `p(x) = ((a_n × x + a_{n-1}) × x + ... ) × x + a_0`, basta **construir a árvore de combinadores** que reflete o aninhamento de Horner:

```
Para coeffs = [a_0, a_1, ..., a_n]:
    acc <- encode_nat(a_n)
    para k de n-1 até 0:
        x_node <- encode_nat(x)
        prod <- build_mul(net, acc, x_node)
        coef_node <- encode_nat(a_k)
        acc <- build_add(net, prod, coef_node)
    return acc  // raiz da rede; reduzir para Normal Form devolve Church(p(x))
```

Isso constrói uma rede IC com `n` `mul` e `n` `add` "encadeados", **mas** — e aqui está o ponto — uma vez construída, essa rede **não tem ordem privilegiada de redução**. O motor pode reduzir o `mul` do topo antes do `add` do meio, ou vice-versa, ou em paralelo. A confluência garante que o resultado final (a Church numeral em Normal Form) é o mesmo.

O **decoder** é o `decode_nat(net) -> u64` (ou `BigUint` para resultados grandes) que já existe em SPEC-14. Lê a Church numeral resultante e devolve o valor numérico.

A **validação** é trivial: roda Horner serial em Rust (`fn horner_serial(coeffs: &[u64], x: u64) -> u64`), compara byte-a-byte com `decode_nat(reduce_distributed(encode_horner(coeffs, x)))`.

## 8. O que o Topic 2 vai entregar (preview)

O design doc do Topic 2 vai detalhar:

1. **Encoder** `HornerEncoder` que recebe `(coeffs: Vec<u64>, x: u64)` e produz a rede IC.
2. **Decoder** que devolve o resultado como número.
3. **Integração com o motor distribuído do Relativist** (nada novo no motor; usa o que já tem).
4. **Validação por oracle**: `horner_serial` em Rust nativo como ground truth.
5. **Demo end-to-end**: rodar a mesma `(coeffs, x)` distribuído entre `W ∈ {1, 2, 4, 8}` workers, reportar tempos e correctness.
6. **Decisão de escopo**: extender só SPEC-14 (mínimo) ou implementar o Encoder/Decoder API completo de SPEC-27 (com registro, traits, etc.) — esta é uma das próximas perguntas do brainstorm.

## 9. Limites práticos previsíveis

Honestamente, esperamos:

- **Resultados crescem rápido**. `p(x)` com `n=10` coeficientes pequenos e `x=10` já chega a `~10¹⁰`. Pra `n=20, x=10`, `~10²⁰`, próximo do limite de `u64`. Para escalar mais, vamos precisar de `BigUint`/`BigInt` no decoder. SPEC-14 atual limita Church a `n ≤ 10_000`, então temos teto.
- **Tempo de redução é dominado por aritmética em Church numeral**, que é "lenta" em comparação com aritmética nativa de CPU. Vamos rodar polinômios menores que faríamos com Horner nativo, mas o ponto não é perf — é correctness sob distribuição.
- **Não vamos vencer Horner nativo em tempo absoluto**. Isso seria absurdo (Church arithmetic é várias ordens de grandeza mais lento que aritmética de CPU). O ponto pro TCC é mostrar **viabilidade da abordagem** e **correctness sob distribuição arbitrária**.

A pergunta de pesquisa que Horner responde no TCC é **qualitativa**, não de performance: _"é possível avaliar um polinômio distribuídamente, retornando o resultado correto, usando IC + Relativist, sem reescrever o algoritmo para forma paralela?"_ — e a resposta empírica vai ser sim.

## 10. Referências

- **Horner, W. G.** (1819). "A new method of solving numerical equations of all orders, by continuous approximation." *Philosophical Transactions of the Royal Society of London*. (Origem do método; o nome Horner gruda apesar de o método já existir antes.)
- **Knuth, D. E.** *The Art of Computer Programming, Vol. 2: Seminumerical Algorithms*. Seção 4.6.4 sobre avaliação de polinômios — referência canônica.
- **Lafont, Y.** (1997). "Interaction Combinators." *Information and Computation*, 137(1):69-101. (REF-002 do TCC — fonte da confluência IC.)
- **ARG-001** do TCC: confluência preserva determinismo (P1-P6). Ponto teórico que a demo de Horner ilustra empiricamente.
- **SPEC-14** do Relativist: módulo `encoding/` com `encode_nat`, `build_add`, `build_mul`, `build_exp`, `decode_nat`. Já em produção.
