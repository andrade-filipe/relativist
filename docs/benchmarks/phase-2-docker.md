# Phase 2 — Docker (TcpLocalhost)

Phase 2 executa o protocolo BSP completo sobre TCP, mas com coordinator e workers em **containers na mesma maquina** (loopback). Isola o custo do algoritmo + protocolo TCP, sem interferencia de rede fisica.

## Pre-requisitos

- Docker Desktop em execucao (`docker info` deve funcionar).
- `docker-compose.yml` presente em `codigo/relativist/`.
- Binario `relativist` compilado em `target/release/`.
- Python 3 disponivel (`python3`) para o script de orquestracao.

## Execucao via script

```bash
cd codigo/relativist
bash reproduce_article/scripts/bench_docker.sh
```

O `bench_docker.sh`:

1. Constroi as imagens Docker (`relativist-worker` e `relativist-coordinator`).
2. Para cada benchmark, gera o input net uma vez.
3. Para cada tamanho, mede o baseline sequencial nativo (fora do Docker).
4. Para cada `(benchmark, tamanho, workers)`, executa `docker compose up` repetidamente com warmup + repeticoes.
5. Verifica G1 estrutural (`inspect seq` vs `inspect distributed`).
6. Agrega em `results/phase2_{detail,summary,rounds}.csv`.

## Flags do orquestrador

| Flag                 | Efeito                                                 |
|----------------------|--------------------------------------------------------|
| `--dry-run`          | Imprime o plano sem executar Docker                    |
| `--skip-build`       | Pula `docker compose build` (imagem ja pronta)         |
| `--skip-sequential`  | Pula os baselines nativos (reusa os anteriores)        |

Exemplo — usar imagem ja construida e baselines ja medidos:

```bash
bash reproduce_article/scripts/bench_docker.sh --skip-build --skip-sequential
```

## Executar manualmente um unico config (debug)

```bash
# 1. Gerar o input net
relativist generate ep-annihilation-con -n 500000 -o data/bench_ep_500000.bin

# 2. Copiar para o volume do docker-compose
cp data/bench_ep_500000.bin data/input.bin
rm -f data/output.bin data/metrics.json

# 3. Subir a stack com N workers
NUM_WORKERS=4 docker compose up \
  --abort-on-container-exit \
  --exit-code-from coordinator \
  --scale worker=4

# 4. Inspecionar o resultado
cat data/metrics.json
relativist inspect -i data/output.bin

# 5. Limpar
docker compose down --remove-orphans
```

> Em campanhas de stress ou com coordinator demorado para flushar metrics, use `docker compose up -d` + `docker wait` em vez de `--abort-on-container-exit`. Veja [campaigns/v1-stress.md](campaigns/v1-stress.md) e o item **L7** em [limitations.md](limitations.md).

## Retomar uma campanha parcial

Se `bench_docker.sh` for interrompido, os scripts `bench_docker_resume.sh` e `bench_docker_resume2.sh` em `scripts/` mostram o padrao para re-rodar apenas configs que falharam. Ajuste o array `CONFIGS` para listar apenas os itens pendentes.

## Validacao imediata

```bash
# Zero linhas com correct=false
awk -F, 'NR>1 && $6=="false"' results/phase2_detail.csv | wc -l   # DEVE ser 0

# Contagens
wc -l results/phase2_detail.csv
# Esperado: 329 (8 sequencial + 8*4*10 Docker + 1 header)

wc -l results/phase2_summary.csv
# Esperado: 41 (8 sequencial + 32 Docker + 1 header)

# Todas as 8 combinacoes bench*size
awk -F, 'NR>1 {print $1 "_" $2}' results/phase2_summary.csv | sort -u | wc -l   # Esperado: 8
```

## Proximo passo

- [Phase 3 — LAN](phase-3-lan.md) para rodar o mesmo protocolo em maquinas reais e extrair `t_network = t_lan - t_localhost`.
