# 5 — Modo Distribuido (TCP + Docker)

Neste guia voce sobe o ciclo BSP sobre **TCP real**, com coordinator e workers em processos separados. Funciona em tres cenarios: loopback (127.0.0.1), containers Docker na mesma maquina, ou maquinas diferentes numa LAN.

Pre-requisito: voce entendeu o ciclo BSP com `local` ([03 — Grid Local](03-local-grid.md)). O mecanismo e o mesmo; so muda como os processos sao lancados.

## 5.1 Quando usar cada modo

| Modo                  | Comando                        | Cenario                                         |
|-----------------------|--------------------------------|-------------------------------------------------|
| Sequencial            | `reduce`                       | Baseline, rede pequena                          |
| Local (in-process)    | `local -w N`                   | Simular grid numa unica maquina, sem TCP        |
| TCP loopback          | `coordinator` + `worker`       | Exercitar protocolo TCP local, sem Docker       |
| TCP Docker            | `docker compose up`            | Isolar processos em containers (Secao 5.4)      |
| TCP LAN               | `coordinator` + `worker` em maquinas diferentes | Grid distribuido real (ver [benchmarks/phase-3-lan.md](../benchmarks/phase-3-lan.md)) |

## 5.2 `coordinator` — No mestre

```bash
relativist coordinator --workers N --bind HOST:PORT \
  -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>] [opcoes]
```

| Flag                  | Descricao                                              |
|-----------------------|--------------------------------------------------------|
| `--workers N`         | Quantos workers conectarao (obrigatorio)               |
| `--bind HOST:PORT`    | Endereco de bind (ex.: `0.0.0.0:9000`)                 |
| `-i, --input`         | Arquivo da rede de entrada                             |
| `-o, --output`        | Salvar rede reduzida final                             |
| `-m, --metrics`       | Salvar metricas em `.json` ou `.csv`                   |
| `--max-rounds N`      | Limitar rodadas BSP                                    |
| `--strategy`          | Estrategia de particao (`round-robin`, default)        |
| `--token auto|<b64>`  | Autenticacao (Secao 5.5)                               |
| `--delta-mode`        | Ativa protocolo delta v2 — ver [guia 06](06-delta-protocol.md) |
| `--log-format`        | `text` ou `json`                                       |

O coordinator **bloqueia** esperando os N workers conectarem antes de iniciar a primeira rodada BSP.

## 5.3 `worker` — No de calculo

```bash
relativist worker --coordinator HOST:PORT [--token <b64>]
```

| Flag                       | Descricao                                          |
|----------------------------|----------------------------------------------------|
| `--coordinator HOST:PORT`  | Endereco do coordinator (obrigatorio)              |
| `--token <b64>`            | Token de autenticacao (se coordinator exigir)      |
| `--log-format`             | `text` ou `json`                                   |

O worker conecta com **retry exponencial** (ate ~30s). Recebe particoes, reduz, devolve resultado.

## 5.4 Exemplo — 1 coordinator + 2 workers no loopback

Abra **3 terminais** (ou use `&` para background).

### Terminal 1 (coordinator)

```bash
# Gerar rede de teste
relativist generate ep-annihilation-con -n 10000 -o ep10k.bin

# Iniciar coordinator com 2 workers
relativist coordinator \
  --workers 2 \
  --bind 127.0.0.1:9000 \
  -i ep10k.bin \
  -o ep10k_grid.bin \
  -m ep10k_metrics.json
```

### Terminal 2 (worker 1)

```bash
relativist worker --coordinator 127.0.0.1:9000
```

### Terminal 3 (worker 2)

```bash
relativist worker --coordinator 127.0.0.1:9000
```

Apos os 2 workers conectarem, o coordinator executa o grid, imprime o resumo e todos os processos terminam. `ep10k_metrics.json` contem metricas detalhadas por rodada (bytes enviados/recebidos, compute, merge, network).

## 5.5 Seguranca — autenticacao por token

Bind em `0.0.0.0` **sem** token emite warning. Para habilitar autenticacao (SPEC-10):

### Coordinator gera token automaticamente

```bash
relativist coordinator --workers 2 --bind 0.0.0.0:9000 \
  --token auto --token-file /tmp/rel-token \
  -i input.bin
```

O token base64 e impresso no stdout e salvo em `/tmp/rel-token`.

### Workers usam o mesmo token

```bash
TOKEN=$(cat /tmp/rel-token)
relativist worker --coordinator coord-host:9000 --token "$TOKEN"
```

Detalhes do modelo de 3 niveis em `docs/specs/SPEC-10-security.md`.

## 5.6 Docker

### Construir a imagem

```bash
cd codigo/relativist
docker build -t relativist .
```

### Verificar

```bash
docker run --rm relativist --version
```

### Uso com volume montado

```bash
mkdir -p /tmp/relativist-data

# Gerar uma rede
docker run --rm -v /tmp/relativist-data:/data \
  relativist generate ep-annihilation -n 100 -o /data/ep100.bin

# Reduzir sequencialmente
docker run --rm -v /tmp/relativist-data:/data \
  relativist reduce -i /data/ep100.bin -o /data/ep100_reduced.bin

# Simular grid com 4 workers
docker run --rm -v /tmp/relativist-data:/data \
  relativist local -w 4 -i /data/ep100.bin \
  -o /data/ep100_grid.bin -m /data/metrics.json

# Aritmetica Church (sem volume)
docker run --rm relativist compute mul 5 6 --workers 4

# Benchmark suite com CSV
docker run --rm -v /tmp/relativist-data:/data \
  relativist bench --benchmark ep_annihilation \
    --sizes 100,500 --workers 2,4 \
    --warmup 1 --repetitions 3 \
    --csv-detail /data/detail.csv \
    --csv-summary /data/summary.csv
```

### `docker-compose` — coordinator + N workers

A raiz do repo inclui `docker-compose.yml`. Para subir com 4 workers:

```bash
docker compose up --scale worker=4
```

Este e o mecanismo usado pela [campanha Phase 2](../benchmarks/phase-2-docker.md).

### Notas de ambiente

- **Git Bash (Windows).** Prefixe comandos com `MSYS_NO_PATHCONV=1` para evitar conversao automatica de caminhos:
  ```bash
  MSYS_NO_PATHCONV=1 docker run --rm -v "C:/Users/Filipe/data:/data" \
    relativist generate ep-annihilation -n 100 -o /data/ep100.bin
  ```
- **`peak_memory_bytes` so funciona em Linux.** Le `/proc/self/status` (`VmHWM`). No Docker container (Linux), funciona. No Windows/Mac nativo, retorna 0.

## 5.7 Proximos passos

- **Rede real (LAN).** Veja [benchmarks/phase-3-lan.md](../benchmarks/phase-3-lan.md) para hardware, topologia, setup de NTP e token.
- **Reduzir trafego.** Veja [guia 06 — Protocolo Delta](06-delta-protocol.md) se voce esta pensando em ativar `--delta-mode`.
- **Reduzir alocacoes.** Veja [guia 07 — Zero-Copy](07-zero-copy.md) se voce vai compilar com `--features zero-copy`.

---

**Proximo guia →** [06 — Protocolo Delta (v2)](06-delta-protocol.md).
