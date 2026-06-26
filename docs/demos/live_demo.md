# Tutorial — Demonstração ao Vivo da Confluência (G1) com HornerCodec

Guia operacional para apresentar a redução distribuída de Interaction
Combinators **ao vivo** (defesa de TCC, seminário, demo de laboratório),
usando o algoritmo de Horner como cobaia e Docker para distribuição.

A tese narrada: **a mesma rede IC reduz pro mesmo valor numérico
independente da estratégia escolhida** — sequencial in-process, dentro
de container, ou particionada em N workers paralelos. Isso é a
**propriedade G1 (Lafont 1997)** que sustenta toda a hipótese do TCC.

---

## 0. Pré-requisitos

| Item | Como verificar |
|---|---|
| Docker Desktop rodando | `docker ps` responde sem erro |
| Binário release compilado | `ls target/release/relativist*` mostra `.exe` (Windows) ou sem extensão (Linux/Mac) |
| Repo na tag v0.21.0 ou superior | `git describe --tags` |
| Imagem `bench-tcp` já buildada | Primeira invocação `docker compose --profile bench-tcp run --rm bench-tcp ...` não pede build (passo 1 abaixo cuida) |

Se faltar qualquer um:

```bash
cd codigo/relativist
cargo build --release --bin relativist
docker compose --profile bench-tcp build      # buildar imagem (5-10min one-time)
```

---

## 1. Pré-aquecimento (faça ANTES da plateia entrar)

O primeiro `docker compose run` puxa a imagem para o cache local e leva
10-30 segundos extras. **Não faça isso no palco.** Roda uma vez antes:

```bash
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
  docker compose --profile bench-tcp run --rm bench-tcp \
  compute --codec horner --input '{"coeffs":[1],"x":1}' --workers 1
```

Saída esperada (~10-30s): "Result: { "value": "1" }". Depois disso, cada
invocação Docker leva 1-3 segundos.

**Nota Windows:** `MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*'` desliga a
conversão automática que o Git Bash faz em args com `/`, que mangleia o
JSON `--input` (lição aprendida em D-014 Phase 2).

---

## 2. Roteiro de apresentação (3-5 minutos)

Use o script interativo:

```bash
bash reproduce_article/scripts/horner_live_demo.sh
```

Ele pausa em cada passo aguardando Enter — você narra entre eles. 4
passos:

### Passo 1 — Encoders disponíveis (set the stage)

```bash
relativist encoders list
```

Mostra:
```
Available encoders:
  church_add             Church numeral addition (a + b)
  church_exp             Church numeral exponentiation (a ^ b)
  church_mul             Church numeral multiplication (a * b)
  church_sum_of_squares  Sum of squares (1^2 + 2^2 + ... + n^2)
  horner                 Polynomial evaluation via Horner's method
```

**Narrativa:** "O sistema tem um registry de encoders. Cada um traduz uma
operação aritmética em uma rede IC. Hoje uso Horner — algoritmo
canônico para avaliar polinômios."

### Passo 2 — Baseline in-process

```bash
relativist compute --codec horner --input '{"coeffs":[10000,500,1],"x":100}'
```

Polinômio: p(x) = 10000 + 500x + x² em x=100. Esperado:

```
=== Relativist Compute (encoder: horner) ===
Encoding:    <N> agents, <M> redexes
Reduction:   1220 interactions in 0.00s (... MIPS)
Result:      {
  "bit_length": 17,
  "value": "70000"
}
```

**Narrativa:** "Aqui o codec encoda o polinômio como rede IC, reduz
sequencialmente in-process, e decoda o resultado. 1220 interações,
value = 70000. Lembre desse número."

### Passo 3 — Mesma redução, dentro de Docker (W=1)

```bash
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
  docker compose --profile bench-tcp run --rm bench-tcp \
  compute --codec horner --input '{"coeffs":[10000,500,1],"x":100}' --workers 1
```

Esperado: `"value": "70000"` (mesmo).

**Narrativa:** "Agora isolei a redução em um container Docker. Single-
threaded ainda, mas com toda a stack de TCP do coordenador. Mesmo
value. Isso prova que o path do container funciona end-to-end."

### Passo 4 — Distribuído em W=4 workers

```bash
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
  docker compose --profile bench-tcp run --rm bench-tcp \
  compute --codec horner --input '{"coeffs":[10000,500,1],"x":100}' --workers 4
```

Esperado: `"value": "70000"` (mesmo).

**Narrativa:** "Agora particiono a rede em 4 sub-redes. Cada worker
reduz a sua, depois eu mergeio. Em IC puro, qualquer ordem de redução
chega no mesmo normal form — isso é **confluência forte**. Mesmo value:
70000."

### Passo 5 — Escalada final W=8

```bash
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
  docker compose --profile bench-tcp run --rm bench-tcp \
  compute --codec horner --input '{"coeffs":[10000,500,1],"x":100}' --workers 8
```

Esperado: `"value": "70000"`.

**Narrativa de fechamento:** "Quatro estratégias diferentes de redução
— in-process, W=1 container, W=4 paralelo, W=8 paralelo — produzem o
**mesmo** valor numérico, **70000**, para o **mesmo** polinômio. Isso é
G1, declarada em SPEC-01 e derivada da confluência forte demonstrada
por Lafont (1997). É o resultado que sustenta a viabilidade de
distribuir IC sobre Grid Computing sem comprometer correctness — a
tese central deste TCC."

---

## 3. Variações do script

| Comando | Polinômio | Interações | Value | Por quê usar |
|---|---|---|---|---|
| `bash reproduce_article/scripts/horner_live_demo.sh` | `[10000,500,1]@100` | 1220 | 70000 | **Default.** Degree-2 max-scale. Demo balanceada de trabalho real. |
| `bash reproduce_article/scripts/horner_live_demo.sh --big` | `[1,1025]@10000` | 2059 | 10250001 | **Mais dramática.** Single-iter c1 no limite do envelope. Quase o dobro de interações. |
| `bash reproduce_article/scripts/horner_live_demo.sh --input '{"coeffs":[42],"x":99}'` | constante 42 | 0 | 42 | Demo "anti-paralelismo" — nada pra reduzir. |
| `bash reproduce_article/scripts/horner_live_demo.sh --input '{"coeffs":[1,1,1],"x":2}'` | `x² + x + 1` em x=2 | 26 | 7 | Demo "didática" — pequena o suficiente pra explicar passo a passo. |

---

## 4. Envelope de inputs aceitos (NÃO estoure)

D-016 BUG-001 fix garante que o decoder retorna `Err` (não `Ok` errado)
fora do envelope, então tentar input inválido durante a apresentação
vai dar erro CLARO — não risco de mostrar resultado falso. Mas ainda
queremos demos que funcionem:

| Tipo | Restrição | Exemplo OK | Exemplo Err |
|---|---|---|---|
| Constante | `coeffs.len() == 1`, valor ≤ 10000 | `[42]@99` | `[99999]@1` (`exceeds cap`) |
| Single-iter | `[c0, c1]` com c0 ∈ [0,10000], c1 ∈ [0,1025] | `[1,1025]@10000` | `[1,1026]@10` (`Err out of envelope`) |
| Degree-2 | `[c0, c1, 1]` (c2 deve ser exatamente 1) | `[10000,500,1]@100` | `[1,2,3]@2` (c2≠1, `Err`) |

Tudo fora disso falha cedo com mensagem clara. Mantenha-se dentro pra
ter demo limpa.

---

## 5. Troubleshooting ao vivo

| Sintoma | Causa provável | Fix rápido |
|---|---|---|
| Docker invocação trava ~30s no primeiro run | Imagem não cacheada | Você esqueceu o pré-aquecimento §1 — improvise: "vou aproveitar pra falar de SPEC-21 enquanto o container sobe" |
| `error: unrecognized net structure: non-CON in app chain` | Input fora do envelope | Confira coluna 4 da tabela §4 — input inválido pra v1 codec |
| `cannot create C:/Program Files/Git/...` | MSYS path conv | Você esqueceu `MSYS_NO_PATHCONV=1` — só roda se tiver Git Bash no Windows |
| Docker compose `not found` | Docker Desktop fechou | Reabre Docker Desktop, espera ícone ficar verde |
| Values diferentes entre arms | NUNCA deveria acontecer no envelope | Se acontecer, é bug — para a demo, salva os outputs e abre issue depois |

---

## 6. Reprodutibilidade

Após a apresentação, qualquer ouvinte pode reproduzir:

```bash
git clone https://github.com/andrade-filipe/relativist.git
cd relativist
git checkout v0.21.0          # ou tag mais recente
cargo build --release --bin relativist
docker compose --profile bench-tcp build
bash reproduce_article/scripts/horner_live_demo.sh
```

Ou o batch completo (10 demos × 4 workers × 2 arms = 80 rows):

```bash
bash reproduce_article/scripts/horner_demo.sh --csv results/horner_demo_$(date -I).csv
```

Compara contra o dataset locked: `results/horner_demo_2026-05-16.csv`.

---

## 7. Variante multi-container (D-017)

A partir do bundle D-017, há **duas** formas de demonstrar a redução
distribuída ao vivo:

| Script                                | Containers | Distribuição     | Caso de uso             |
|---------------------------------------|------------|------------------|--------------------------|
| `horner_live_demo.sh` (default)       | 1          | Threads internas | Defesa concisa, 4 passos |
| `horner_distributed_demo.sh` (D-017)  | 1 + N      | TCP BSP real     | Demonstrar a tese        |

A versão default usa o profile `bench-tcp` do Compose — um único
container com `--workers N` interno (path in-process distribuído). A
variante D-017 sobe **N+1 containers separados** via o profile padrão:
um coordinator e N workers, cada um com log persistente. Use a variante
quando a banca pedir para ver "atrás das cortinas" — cada worker tem o
seu próprio container, e o operador pode chamar `docker logs
relativist-worker-K` mid-talk.

### Quando usar a variante multi-container

```bash
bash reproduce_article/scripts/horner_distributed_demo.sh                     # default: 4 workers
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 2
bash reproduce_article/scripts/horner_distributed_demo.sh --input '{"coeffs":[42],"x":7}'
bash reproduce_article/scripts/horner_distributed_demo.sh --keep-running      # NÃO derrubar containers
```

### Fluxo (6 estágios)

1. **Encode local** (host): JSON → `data/horner_input_<TS>.bin` via
   `relativist compute --codec horner --encode-only --output ...`.
2. **In-process reference** (host): mesmo input pelo path local, extrai
   o JSON do `Result:` para cross-check G1 no final.
3. **Coordinator container sobe** lendo o `.bin` via a env var
   `INPUT_PATH` (patch backwards-compat em `docker-compose.yml`).
4. **N worker containers** conectam via `coordinator:9000`. BSP loop
   até normal form; coordinator escreve `data/horner_output_<TS>.bin`.
5. **Decode local** (host): `relativist decode --codec horner --input
   ...` → JSON pretty-printed em stdout.
6. **G1 cross-check** (host): decoded vs in-process reference. String-
   equal ou exit 6.

O script chama `docker compose stop` no fim (NÃO `down` — preserva
containers para `docker logs`). Use `--keep-running` para deixar os
containers de pé durante a apresentação (`docker logs -f` mid-narração).

### Inspeção pós-execução

Ao final, o script imprime:

```text
Inspect logs:
  docker logs relativist-coordinator-1
  docker logs relativist-worker-1
  docker logs relativist-worker-2
  ...
  docker logs relativist-worker-N
```

Cada worker mostra o ciclo BSP individual (round IDs, redex counts,
partition stats). Use `docker logs -f relativist-worker-1` para
acompanhar em tempo real durante demos longas.

### Pré-flight (idêntico à seção §1)

Aqueça a imagem antes da plateia entrar — o primeiro `docker compose up`
demora 10–30s (puxa rede, monta volumes, inicia containers):

```bash
MSYS_NO_PATHCONV=1 MSYS2_ARG_CONV_EXCL='*' \
  docker compose --profile bench-tcp run --rm bench-tcp \
  compute --codec horner --input '{"coeffs":[1],"x":1}' --workers 1
```

(O profile `bench-tcp` usa a mesma imagem; basta um warm-up.)

### Cuidados

- O script termina com `docker compose stop` por default. Se você quiser
  inspecionar logs pós-talk sem precisar re-subir, use `--keep-running`.
- O cross-check G1 é string-equal sobre o JSON; mismatch é classificado
  exit 6 e dump de ambos os blocos JSON para stderr (deve ser bug).
- Concorrência: o script timestampa cada `.bin` (`horner_input_<TS>.bin`)
  para evitar colisão com execuções paralelas.

---

## 8. Cross-references

- **Script interativo (1 container, threads internas):** `reproduce_article/scripts/horner_live_demo.sh`
- **Script multi-container (D-017, N+1 containers separados):** `reproduce_article/scripts/horner_distributed_demo.sh`
- **Script batch (sem pausa):** `reproduce_article/scripts/horner_demo.sh`
- **Doc de demonstração base:** `docs/demos/horner-g1-demonstration.md`
- **Dataset locked:** `results/horner_demo_2026-05-16.csv`
- **Spec da invariante G1:** `specs/SPEC-01-invariantes.md`
- **Argumento P1-P6 derivando G1:** `discussoes/argumentos/ARG-001-confluencia-preserva-determinismo.md` (no repo TCC root)
- **Spec do encoder API:** `specs/SPEC-27-encoder-decoder-api.md` (v3)
- **Explainer matemático de Horner:** `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
- **Tag de release usada:** `v0.21.0` (commit `ebd0439` em main)
