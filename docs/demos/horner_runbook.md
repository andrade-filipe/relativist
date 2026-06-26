# Runbook — Comandos para Rodar Horner no Relativist

Referência operacional consolidada com **todos os comandos** para
exercitar o HornerCodec, do mais simples (single computation in-process)
ao mais complexo (demo distribuída multi-container com inspeção de logs
por worker).

Complementa:
- `docs/demos/horner-g1-demonstration.md` — narrativa acadêmica dos 7 demos
- `docs/demos/live_demo.md` — roteiro de apresentação interativo

---

## Cheat-sheet — escolha o cenário

| Cenário | Comando | Wall |
|---|---|---|
| Single computation rápida | `target/release/relativist.exe compute --codec horner --input '<JSON>'` | <1s |
| Single + workers internos | mesmo, com `--workers N` | <1s |
| Encode-only + decode separados | `compute --encode-only -o X.bin` + `decode --codec horner -i X.bin` | <1s |
| Batch validação 10 demos × 4W × 2 arms | `bash reproduce_article/scripts/horner_demo.sh` | ~12-30s |
| Apresentação Enter-paced single-container | `bash reproduce_article/scripts/horner_live_demo.sh` | interativo |
| **Multi-container distribuída (workers em containers separados)** | `bash reproduce_article/scripts/horner_distributed_demo.sh --workers N` | ~10-30s |
| Inspecionar logs pós-demo | `docker logs relativist-{coordinator,worker}-<N>` | instantâneo |

---

## 0. Pré-flight (uma vez por máquina)

```bash
cd codigo/relativist

# Compila binário release (~30s primeira vez, instantâneo depois)
cargo build --release --bin relativist

# Verifica Docker
docker ps                        # responde sem erro
docker compose version           # v2 disponível

# Builda imagem Docker (5-10min one-time)
docker compose --profile bench-tcp build
```

Confirma com:

```bash
target/release/relativist.exe --version
target/release/relativist.exe encoders list   # deve listar 'horner'
```

---

## 1. Single computation in-process

Caso mais simples — encoda → reduz → decoda em um único processo.

```bash
# Sequencial puro (sem BSP grid)
target/release/relativist.exe compute --codec horner \
  --input '{"coeffs":[10000,500,1],"x":100}'

# Com workers BSP no mesmo processo (paraleliza via threads)
target/release/relativist.exe compute --codec horner \
  --input '{"coeffs":[10000,500,1],"x":100}' --workers 4
```

Output esperado:
```
=== Relativist Compute (encoder: horner) ===
Encoding:    ~10k+ agents, 4-6 redexes
Reduction:   1220 interactions in 0.00s (... MIPS)
Result:      {"bit_length": 17, "value": "70000"}
```

---

## 2. Encode-only + decode separados (D-017)

Útil pra inspecionar a rede IC pré-redução, salvar pra processamento
em outro lugar, ou pipelinar com coordinator/worker.

```bash
# Encoda Horner sem reduzir (salva .bin com a rede crua)
target/release/relativist.exe compute --codec horner \
  --input '{"coeffs":[10000,500,1],"x":100}' \
  --encode-only --output /tmp/horner_net.bin

# Inspeciona a rede (opcional)
target/release/relativist.exe inspect --input /tmp/horner_net.bin

# Reduz a .bin (in-process)
target/release/relativist.exe reduce --input /tmp/horner_net.bin \
  --output /tmp/horner_reduced.bin

# Decoda a .bin reduzida
target/release/relativist.exe decode --codec horner \
  --input /tmp/horner_reduced.bin
```

Atalho equivalente (sem .bin intermediário):

```bash
target/release/relativist.exe compute --codec horner \
  --input '{"coeffs":[10000,500,1],"x":100}' \
  --output /tmp/horner_reduced.bin    # pós D-017 BUG-001 fix: agora salva
```

---

## 3. Batch — validação completa (D-016)

Roda 10 demos × W ∈ {1,2,4,8} × 2 arms (in-process + Docker
single-container) com G1 cross-check automático.

```bash
bash reproduce_article/scripts/horner_demo.sh --csv results/horner_demo_$(date -I).csv
```

Esperado: 80 rows no CSV, exit 0, `OK: 80/80 rows passed`. Tempo ~12-30s.

**Verificação:**
```bash
CSV=results/horner_demo_$(date -I).csv
wc -l $CSV                                          # 81 (header + 80)
awk -F, '$NF=="false"{print}' $CSV | wc -l          # 0 (nenhum g1 mismatch)
awk -F, '$(NF-1)=="false"{print}' $CSV | wc -l      # 0 (nenhum value mismatch)
```

---

## 4. Apresentação interativa single-container (D-016)

Script Enter-paced — pausa em cada passo para você narrar.

```bash
bash reproduce_article/scripts/horner_live_demo.sh                    # default [10000,500,1]@100
bash reproduce_article/scripts/horner_live_demo.sh --big              # [1,1025]@10000 — 2059 interactions
bash reproduce_article/scripts/horner_live_demo.sh --input '<JSON>'   # custom (dentro do envelope)
```

5 passos: encoders list → in-process → Docker W=1 → W=4 → W=8.
Tudo em **1 container Docker** (worker são threads internos).

Documentação completa: `docs/demos/live_demo.md`.

---

## 5. ⭐ Multi-container distribuída (D-017)

**A demo de verdade pra Grid Computing.** Coordinator em um container,
N workers em containers separados, cada um com seu log preservado.

### 5.1 Comando básico

```bash
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 4 \
  --input '{"coeffs":[10000,500,1],"x":100}'
```

### 5.2 O que acontece (passo-a-passo automatizado)

1. **Encode local:** `relativist compute --encode-only` salva
   `./data/horner_net_<timestamp>.bin`
2. **Coordinator up:** `docker compose up -d coordinator` (porta 9000)
3. **Workers up:** `docker compose up -d --scale worker=N worker` —
   **N containers separados**, cada um conectando ao coordinator via TCP
4. **Distribução:** coordinator particiona a rede, envia sub-redes pros workers
5. **Redução paralela:** cada worker reduz sua partição EM SEU CONTAINER
6. **Merge:** coordinator recebe resultados, mergeia, salva
   `./data/horner_reduced_<ts>.bin`
7. **Decode local:** `relativist decode` lê o .bin merged → JSON
8. **G1 cross-check:** compara com baseline in-process (mesmo input,
   mesmo encoder, sem distribuição) — devem casar
9. **Stop preservando:** `docker compose stop` (NÃO `down`) — containers
   ficam parados mas **presentes pra inspeção**

### 5.3 Variações

```bash
# Menos workers
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 2

# Mais workers
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 8

# Input mais pesado (2059 interactions)
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 4 \
  --input '{"coeffs":[1,1025],"x":10000}'

# Input didático (constante — 0 interactions, demo "anti-paralelismo")
bash reproduce_article/scripts/horner_distributed_demo.sh --workers 4 \
  --input '{"coeffs":[42],"x":99}'
```

---

## 6. Inspeção dos logs (depois da §5)

Os containers ficam preservados após `docker compose stop`. Inspecione
quanto tempo quiser.

```bash
# Lista containers da demo
docker ps -a --filter "name=relativist-"
```

Esperado:
```
relativist-coordinator-1   Exited
relativist-worker-1        Exited
relativist-worker-2        Exited
relativist-worker-3        Exited
relativist-worker-4        Exited
```

```bash
# Log do coordinator (mostra: load .bin, partition, distribute, merge, save)
docker logs relativist-coordinator-1

# Log de cada worker individual (mostra: recebeu partição X, reduziu Y interactions, devolveu)
docker logs relativist-worker-1
docker logs relativist-worker-2
docker logs relativist-worker-3
docker logs relativist-worker-4
```

### 6.1 Salvar todos os logs em arquivos (pra incluir no TCC ou revisar)

```bash
mkdir -p logs/horner_demo_$(date -I)
for c in $(docker ps -a --filter "name=relativist-" --format '{{.Names}}'); do
  docker logs "$c" > "logs/horner_demo_$(date -I)/${c}.log" 2>&1
done
ls -la logs/horner_demo_$(date -I)/
```

---

## 7. Limpeza

```bash
# Remove containers preservados (libera CPU/memória)
docker compose down

# Limpa .bin temporários
rm -f data/horner_net_*.bin data/horner_reduced_*.bin data/horner_metrics_*.json

# (Opcional) Apaga imagem Docker se quiser rebuild from scratch
docker rmi relativist:latest
```

---

## 8. Envelope de inputs aceitos

O decoder retorna `Err` (não `Ok` errado — vide D-016 BUG-001 fix) para
qualquer input fora desse envelope. Mantenha-se dentro pra ter demo
limpa:

| Tipo | Restrição | OK | Err |
|---|---|---|---|
| Constante | `coeffs.len() == 1`, valor ≤ 10000 | `[42]@99` | `[99999]@1` (cap) |
| Single-iter | `[c0, c1]` com c0 ∈ [0,10000], c1 ∈ [0,1025] | `[1,1025]@10000` | `[1,1026]@10` (envelope) |
| Degree-2 | `[c0, c1, 1]` (c2 deve ser **exatamente 1**) | `[10000,500,1]@100` | `[1,2,3]@2` (c2≠1) |

Polinômios fora — degree ≥ 3 OU degree-2 com c2 ≥ 2 — são SPEC-27 §5.1
Future Work (Mackie/Pinto shared-form readback).

---

## 9. Troubleshooting

| Sintoma | Causa provável | Fix |
|---|---|---|
| `cannot create C:/Program Files/Git/...` | MSYS path conv em invocação manual de docker | Use scripts (já têm `MSYS_NO_PATHCONV=1`) OU prefixe vc mesmo |
| `unrecognized net structure: non-CON in app chain` | Input fora do envelope §8 | Cole exemplo OK da tabela §8 |
| Primeira invocação Docker trava ~30s | Imagem não cacheada | Rode `docker compose --profile bench-tcp build` antes |
| `error: configuration error: cannot create /results/detail.csv` | Container não consegue escrever no volume | Confirme que `./data/` existe e tem permissão write |
| Containers órfãos após Ctrl+C | Script interrompido entre `compose up` e `compose stop` | `docker compose down` limpa |
| `coordinator timeout waiting for workers` | Workers demoraram > 30s pra conectar | Aumente `--initial-wait-timeout` no comando do coordinator OU verifique que workers subiram (`docker ps`) |
| `port 9000 already in use` | Outro coordinator antigo rodando | `docker compose down` ou `docker rm relativist-coordinator-1` |
| Multi-container demo retorna value diferente do in-process | Bug — F4 (empty-Net) OU regressão | Salve os logs, abra issue |

---

## 10. Cross-references

- **Scripts:** `reproduce_article/scripts/horner_demo.sh`, `reproduce_article/scripts/horner_live_demo.sh`, `reproduce_article/scripts/horner_distributed_demo.sh`
- **Datasets locked:** `results/horner_demo_2026-05-16.csv` (80 rows, 0 mismatches)
- **Spec G1:** `specs/SPEC-01-invariantes.md`
- **Spec encoder API:** `specs/SPEC-27-encoder-decoder-api.md` (v3)
- **Argumento P1-P6:** `discussoes/argumentos/ARG-001-confluencia-preserva-determinismo.md` (no repo TCC root)
- **Explainer matemático:** `docs/superpowers/specs/2026-05-06-horner-method-explainer.md`
- **Narrativa acadêmica:** `docs/demos/horner-g1-demonstration.md`
- **Roteiro apresentação:** `docs/demos/live_demo.md`
- **docker-compose:** `docker-compose.yml` (services `coordinator`, `worker`, `bench-tcp`)
- **Última tag:** `v0.21.0`; D-017 ainda local pendente push (futura `v0.22.0`)
