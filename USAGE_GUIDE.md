# Relativist — Guia Completo de Uso

Motor de reducao distribuida de Interaction Combinators para Grid Computing.

---

## Indice

1. [Instalacao](#1-instalacao)
2. [Conceitos Basicos](#2-conceitos-basicos)
3. [generate — Gerar Redes](#3-generate--gerar-redes)
4. [inspect — Inspecionar Redes](#4-inspect--inspecionar-redes)
5. [reduce — Reducao Sequencial](#5-reduce--reducao-sequencial)
6. [local — Simulacao de Grid](#6-local--simulacao-de-grid)
7. [compute — Aritmetica Church](#7-compute--aritmetica-church)
8. [bench — Suite de Benchmarks](#8-bench--suite-de-benchmarks)
9. [coordinator e worker — Modo Distribuido (TCP)](#9-coordinator-e-worker--modo-distribuido-tcp)
10. [Docker](#10-docker)
11. [Campanhas de Benchmark (Phase 1/2/3)](#11-campanhas-de-benchmark-phase-123)
12. [Pipeline Completa: Gerar, Inspecionar, Reduzir, Comparar](#12-pipeline-completa)
13. [Formatos de Arquivo](#13-formatos-de-arquivo)
14. [update — Atualizar Relativist](#14-update--atualizar-relativist)
15. [completions — Autocompletar no Shell](#15-completions--autocompletar-no-shell)
16. [Desenvolvimento: Verificacoes Pre-Push](#16-desenvolvimento-verificacoes-pre-push)
17. [Referencia Rapida](#17-referencia-rapida)

---

## 1. Instalacao

### Opcao 1: Install script (Linux/macOS) — Recomendada

```bash
curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

O script detecta seu OS/arquitetura, baixa o binario pre-compilado do GitHub Releases,
verifica o checksum SHA256, e instala em `/usr/local/bin` (ou `~/.local/bin`).

Para instalar uma versao especifica:

```bash
VERSION=0.9.0 curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

### Opcao 2: Docker

```bash
docker pull ghcr.io/andrade-filipe/relativist
docker run --rm ghcr.io/andrade-filipe/relativist --version
```

### Opcao 3: Download manual

Baixe o binario para seu sistema em:
https://github.com/andrade-filipe/relativist/releases

- **Linux (recomendado para Debian/Ubuntu):** `relativist-vX.Y.Z-x86_64.deb`
  ```bash
  sudo dpkg -i relativist-vX.Y.Z-x86_64.deb
  ```
- **Linux (alternativa):** `relativist-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` (extrair e colocar no PATH)
- **Windows (recomendado):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.exe` (download direto, sem extrair)
- **Windows (alternativa):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.zip` (extrair o .exe)

Para Linux .tar.gz e Windows .exe/.zip, coloque o binario numa pasta do seu PATH.

**Nota Windows (SmartScreen):** Como o executavel ainda nao possui assinatura digital,
o Windows pode exibir um aviso "O Windows protegeu seu PC". Para executar:

1. Clique com botao direito no `.exe` → Propriedades → marque "Desbloquear" → OK
2. Ou: no dialogo SmartScreen, clique "Mais informacoes" → "Executar assim mesmo"

Isso e normal para executaveis sem certificado de code signing e nao indica risco.
O checksum SHA256 do arquivo pode ser verificado no `SHA256SUMS` do release.

### Opcao 4: Compilar do codigo fonte

Requer Rust 1.75+ (toolchain stable):

```bash
cargo install --git https://github.com/andrade-filipe/relativist
```

Ou para desenvolvimento local:

```bash
cd codigo/relativist
cargo build --release
# Binario em target/release/relativist (Linux/Mac)
# Binario em target\release\relativist.exe (Windows)
```

### Verificar instalacao

```bash
relativist --version
# relativist 0.9.0

relativist --help
```

### Rodar os testes (desenvolvimento)

```bash
cargo test
```

---

## 2. Conceitos Basicos

### Interaction Combinators (IC)

O Relativist trabalha com redes de **Interaction Combinators** (Lafont, 1997). Uma rede IC e composta por:

- **Agentes**: nos com 3 portas (principal + 2 auxiliares). Tres tipos:
  - **CON** (Constructor) — constroi estruturas
  - **DUP** (Duplicator) — duplica estruturas
  - **ERA** (Eraser) — apaga estruturas
- **Wires**: conexoes entre portas
- **Redexes**: pares de agentes conectados por portas principais (candidatos a interacao)
- **Normal Form**: rede sem redexes (resultado final)

### As 6 Regras de Interacao

| Regra      | Par       | Efeito                      |
|------------|-----------|-----------------------------|
| CON-CON    | mesmo     | Aniquilacao (cross-connect) |
| DUP-DUP    | mesmo     | Aniquilacao (parallel)      |
| ERA-ERA    | mesmo     | Void (ambos removidos)      |
| CON-DUP    | diferente | Comutacao (+2 agentes)      |
| CON-ERA    | diferente | Erasure (2 ERAs criados)    |
| DUP-ERA    | diferente | Erasure (2 ERAs criados)    |

### Perfis de Carga de Trabalho

- **Profile A** (Embarrassingly Parallel): todos os redexes sao independentes. 1 rodada no grid.
- **Profile B** (Expansion + Collapse): CON-DUP cria novos agentes antes de aniquilar. Multiplas rodadas.
- **Profile C** (Sequential Dependency): cascata nivel-a-nivel. Muitas rodadas, alto overhead de borda.

---

## 3. generate — Gerar Redes

Gera redes de exemplo parametricas e salva em arquivo.

```bash
relativist generate <TIPO> -n <TAMANHO> -o <ARQUIVO>
```

### Tipos disponiveis

| Tipo                  | Perfil | Descricao                              |
|-----------------------|--------|----------------------------------------|
| `ep-annihilation`     | A      | N pares ERA-ERA (aniquilacao trivial)  |
| `ep-annihilation-con` | A      | N pares CON-CON (aniquilacao cross)    |
| `ep-annihilation-dup` | A      | N pares DUP-DUP (aniquilacao parallel) |
| `con-dup-expansion`   | B      | N pares CON-DUP (expansao+colapso)     |
| `dual-tree`           | C      | Duas arvores de profundidade N         |
| `mixed-rules`         | B      | N pares de cada regra (6 tipos)        |
| `erasure-propagation` | C      | Cadeia de N CON com ERA na ponta       |
| `tree-sum`            | A/B    | Soma de N uns via Church add           |

### Exemplos

```bash
# Gerar 100 pares ERA-ERA em formato binario
relativist generate ep-annihilation -n 100 -o ep100.bin

# Gerar dual-tree de profundidade 6 em formato texto
relativist generate dual-tree -n 6 -o dual6.ic

# Gerar mixed-rules com 10 pares de cada tipo
relativist generate mixed-rules -n 10 -o mixed10.bin

# Gerar cadeia de erasure propagation com 50 CON
relativist generate erasure-propagation -n 50 -o erasure50.bin

# Gerar rede CON-DUP expansion (Profile B)
relativist generate con-dup-expansion -n 100 -o condup100.bin
```

### Formato texto (.ic)

O formato `.ic` e legivel por humanos. Exemplo com 3 pares ERA-ERA:

```bash
relativist generate ep-annihilation -n 3 -o ep3.ic
cat ep3.ic
```

Saida:
```
agent a0 ERA
agent a1 ERA
agent a2 ERA
agent a3 ERA
agent a4 ERA
agent a5 ERA
wire a0.principal a1.principal
wire a2.principal a3.principal
wire a4.principal a5.principal
```

---

## 4. inspect — Inspecionar Redes

Mostra estatisticas de uma rede sem modifica-la.

```bash
relativist inspect -i <ARQUIVO>
```

### Exemplos

```bash
# Inspecionar uma rede nao-reduzida
relativist generate ep-annihilation -n 100 -o ep100.bin
relativist inspect -i ep100.bin
```

Saida:
```
=== Relativist Inspect ===
Agents:  200
  CON: 0
  DUP: 0
  ERA: 200
Redexes: 100
Normal Form: no
```

```bash
# Inspecionar apos reducao
relativist reduce -i ep100.bin -o ep100_reduced.bin
relativist inspect -i ep100_reduced.bin
```

Saida:
```
=== Relativist Inspect ===
Agents:  0
  CON: 0
  DUP: 0
  ERA: 0
Redexes: 0
Normal Form: yes
```

```bash
# Inspecionar uma rede mixed-rules
relativist generate mixed-rules -n 5 -o mixed5.bin
relativist inspect -i mixed5.bin
```

Saida:
```
=== Relativist Inspect ===
Agents:  60
  CON: 20
  DUP: 20
  ERA: 20
Redexes: 30
Normal Form: no
```

---

## 5. reduce — Reducao Sequencial

Reduz uma rede ate a Normal Form usando `reduce_all` (sem distribuicao).

```bash
relativist reduce -i <ENTRADA> [-o <SAIDA>]
```

### Exemplos

```bash
# Reducao basica
relativist generate ep-annihilation -n 1000 -o ep1000.bin
relativist reduce -i ep1000.bin
```

Saida:
```
=== Relativist Reduce Summary ===
Interactions: 1000
Final agents: 0
```

```bash
# Reducao com saida para arquivo
relativist generate dual-tree -n 8 -o dual8.bin
relativist reduce -i dual8.bin -o dual8_reduced.bin
```

```bash
# Reducao de erasure-propagation (cascata de ERAs)
relativist generate erasure-propagation -n 20 -o erasure20.bin
relativist reduce -i erasure20.bin -o erasure20_reduced.bin
relativist inspect -i erasure20_reduced.bin
```

---

## 6. local — Simulacao de Grid

Executa o ciclo completo BSP (Bulk Synchronous Parallel) em-processo:
particionar -> reduzir localmente -> merge -> resolver borda -> repetir.

```bash
relativist local -w <WORKERS> -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>]
```

### Opcoes

| Flag               | Descricao                                    |
|--------------------|----------------------------------------------|
| `-w, --workers`    | Numero de workers simulados (>= 1)          |
| `-i, --input`      | Arquivo da rede de entrada (.bin ou .ic)     |
| `-o, --output`     | Salvar rede reduzida                         |
| `-m, --metrics`    | Salvar metricas em JSON                      |
| `--max-rounds`     | Limite de rodadas (sem limite por padrao)    |
| `--strategy`       | Estrategia de particionamento (round-robin)  |
| `--log-format`     | Formato de log: text ou json                 |

### Exemplos

```bash
# Simular grid com 4 workers
relativist generate ep-annihilation -n 500 -o ep500.bin
relativist local -w 4 -i ep500.bin
```

Saida:
```
=== Relativist Execution Summary ===
Converged:          yes
Rounds:             1
Total interactions: 500
Total time:         0.000s
Final agents:       0
Avg round time:     0.000s
Local interactions: 500
Border interactions:0
```

```bash
# Grid com 2 workers e saida + metricas
relativist generate mixed-rules -n 5 -o mixed5.bin
relativist local -w 2 -i mixed5.bin -o mixed5_grid.bin -m metrics.json
```

```bash
# Grid com limite de rodadas
relativist generate dual-tree -n 10 -o dual10.bin
relativist local -w 4 -i dual10.bin --max-rounds 5
```

```bash
# Log em formato JSON (util para pipelines de dados)
relativist generate con-dup-expansion -n 50 -o condup50.bin
relativist local -w 2 -i condup50.bin --log-format json
```

---

## 7. compute — Aritmetica Church

Codifica numeros naturais como Church numerals em IC, reduz, e decodifica o resultado.

```bash
relativist compute <OPERACAO> <A> <B> [--workers N]
```

### Operacoes

| Operacao | Formula | Exemplo               |
|----------|---------|------------------------|
| `add`    | a + b   | `compute add 3 5` = 8  |
| `mul`    | a * b   | `compute mul 3 4` = 12 |
| `exp`    | a ^ b   | `compute exp 2 3` = 8* |

\* Exponenciacao: a reducao termina corretamente, mas o resultado usa uma forma
compartilhada ciclica (DUP sharing) que nao e decodificavel atualmente.
Esta e uma limitacao conhecida da readback de optimal reduction.

### Exemplos

```bash
# Adicao
relativist compute add 3 5
```

Saida:
```
=== Relativist Compute ===
Expression:  add(3, 5)
Encoding:    29 agents, 1 redexes
Reduction:   6 interactions in 0.00s (0.88 MIPS)
Result:      8
```

```bash
# Multiplicacao
relativist compute mul 3 4
```

Saida:
```
=== Relativist Compute ===
Expression:  mul(3, 4)
Encoding:    23 agents, 1 redexes
Reduction:   9 interactions in 0.00s
Result:      12
```

```bash
# Adicao distribuida (2 workers)
relativist compute add 10 20 --workers 2
```

Saida:
```
=== Relativist Compute ===
Expression:  add(10, 20)
Encoding:    73 agents, 1 redexes
Reduction:   6 interactions in 0.00s
Workers:     2
Rounds:      1
Result:      30
```

```bash
# Exponenciacao (limitacao conhecida)
relativist compute exp 2 3
```

Saida:
```
=== Relativist Compute ===
Expression:  exp(2, 3)
Encoding:    17 agents, 1 redexes
Reduction:   15 interactions in 0.00s
Result:      (non-decodable normal form)
  Final agents: 7
```

```bash
# Multiplicacao distribuida com 4 workers e saida
relativist compute mul 5 6 --workers 4 -o result.bin -m metrics.json
```

---

## 8. bench — Suite de Benchmarks

Executa a suite completa de benchmarks com baseline sequencial, warmup,
repeticoes, verificacao de corretude, e saida CSV.

```bash
relativist bench [OPCOES]
```

### Opcoes

| Flag              | Padrao      | Descricao                              |
|-------------------|-------------|----------------------------------------|
| `--benchmark`     | todos       | Benchmarks a executar (separados por `,`) |
| `--sizes`         | por bench   | Tamanhos de problema                   |
| `--workers`       | `1,2,4,8`  | Contagens de workers                   |
| `--mode`          | `local`     | Modo: `sequential`, `local`            |
| `--warmup`        | `2`         | Rodadas de warmup (descartadas)        |
| `--repetitions`   | `5`         | Repeticoes cronometradas               |
| `--csv-detail`    | —           | CSV detalhado (1 linha por repeticao)  |
| `--csv-rounds`    | —           | CSV por rodada (overhead por fase)     |
| `--csv-summary`   | —           | CSV resumo (estatisticas agregadas)    |
| `--max-rounds`    | —           | Limite de rodadas do grid              |

### Benchmarks disponiveis

| Nome                   | Perfil | Descricao                                |
|------------------------|--------|------------------------------------------|
| `ep_annihilation`      | A      | ERA-ERA (aniquilacao void)               |
| `ep_annihilation_con`  | A      | CON-CON (aniquilacao cross)              |
| `ep_annihilation_dup`  | A      | DUP-DUP (aniquilacao parallel)           |
| `condup_expansion`     | B      | CON-DUP (expansao + colapso)             |
| `dual_tree`            | C      | Arvores duais (cascata nivel-a-nivel)    |
| `tree_sum`             | A/B    | Soma via Church add                      |
| `tree_sum_balanced`    | A/B    | Soma balanceada                          |
| `mixed_net`            | B      | Todas as 6 regras                        |
| `erasure_propagation`  | C      | Propagacao de erasure em cadeia          |
| `church_add`           | B      | Adicao Church numeral                    |
| `church_mul`           | B      | Multiplicacao Church numeral             |
| `cascade_cross`        | C      | Cascata de CON-DUP com cross-partition   |
| `church_sum_of_squares`| B      | Demo aritmetica: `sum i^2` (ver 11.8)    |

### Exemplos

```bash
# Executar um unico benchmark com tamanhos e workers especificos
relativist bench \
  --benchmark ep_annihilation \
  --sizes 100,500,1000 \
  --workers 1,2,4 \
  --warmup 1 \
  --repetitions 3
```

```bash
# Executar todos os benchmarks com configuracao padrao
relativist bench --warmup 2 --repetitions 5
```

```bash
# Apenas modo sequencial (sem grid)
relativist bench \
  --benchmark ep_annihilation \
  --sizes 100,1000 \
  --mode sequential \
  --repetitions 5
```

```bash
# Exportar CSVs para analise
relativist bench \
  --benchmark ep_annihilation,condup_expansion,dual_tree \
  --sizes 100,500 \
  --workers 2,4 \
  --warmup 1 \
  --repetitions 5 \
  --csv-detail results/detail.csv \
  --csv-rounds results/rounds.csv \
  --csv-summary results/summary.csv
```

```bash
# Benchmark rapido para verificar corretude
relativist bench \
  --benchmark ep_annihilation,mixed_net,erasure_propagation,church_add \
  --sizes 10 \
  --workers 2 \
  --warmup 0 \
  --repetitions 1
```

### Saida da suite

A suite mostra uma tabela formatada com resultados e avisos:

```
=== Relativist Benchmark Suite ===
Benchmarks:  ep_annihilation
Mode:        local
Workers:     [1, 2, 4]
Warmup:      1
Repetitions: 3

=== Results ===
Benchmark                   Size Workers    Time(s)     MIPS  Speedup Efficiency
--------------------------------------------------------------------------------
ep_annihilation    100       0   0.000001     83.3   1.0000     1.0000
ep_annihilation    100       1   0.000003     34.1   0.4087     0.4087
ep_annihilation    100       2   0.000022      4.3   0.0520     0.0260
  WARNING: high variance (CV=13.72%) ...

Total datapoints: 18  |  All correct: true
```

### Metricas

- **Time(s)**: Mediana do wall-clock time
- **MIPS**: Millions of Interactions Per Second
- **Speedup**: tempo_sequencial / tempo_grid (> 1.0 = distribuicao compensa)
- **Efficiency**: speedup / workers (1.0 = escala perfeitamente)
- **CV**: Coeficiente de variacao (aviso se > 10%)

### Formato dos CSVs

**detail.csv** — Uma linha por execucao:
```
benchmark,input_size,mode,workers,repetition,correct,wall_clock_secs,
total_interactions,mips,rounds,speedup,efficiency,overhead_ratio,
peak_memory_bytes,bytes_sent,bytes_received,
con_con,dup_dup,era_era,con_dup,con_era,dup_era
```

**rounds.csv** — Uma linha por rodada (apenas grid):
```
benchmark,input_size,workers,mode,repetition,round,
partition_time_secs,compute_time_secs,merge_time_secs,network_time_secs,
border_redexes,border_ratio,agents_at_start,bytes_sent,bytes_received
```

**summary.csv** — Uma linha por configuracao (estatisticas agregadas):
```
benchmark,input_size,mode,workers,repetitions,all_correct,
wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,
mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv
```

---

## 9. coordinator e worker — Modo Distribuido (TCP)

Executa o ciclo BSP sobre TCP com processos separados para o coordinator
(1 no mestre) e workers (N nos de calculo). Este e o mesmo mecanismo usado
no Docker (secao 10) e em testes em rede real (secao 11.3), a diferenca e
apenas como os processos sao lancados e orquestrados.

### 9.1 Quando usar cada modo

| Modo               | Comando             | Cenario                                             |
|--------------------|---------------------|-----------------------------------------------------|
| Sequencial         | `reduce`            | Baseline, rede pequena, sem paralelismo             |
| Local (in-process) | `local -w N`        | Simular grid numa unica maquina sem TCP             |
| TCP (loopback)     | `coordinator`+`worker` | Exercitar protocolo TCP localmente sem Docker    |
| TCP (Docker)       | `docker compose up` | Isolar processos em containers (secao 10)           |
| TCP (rede real)    | `coordinator`+`worker` em maquinas diferentes | Grid distribuido real (secao 11.3) |

### 9.2 coordinator — Nó mestre

```bash
relativist coordinator --workers N --bind HOST:PORT \
  -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>] [opcoes]
```

| Flag               | Descricao                                           |
|--------------------|-----------------------------------------------------|
| `--workers N`      | Quantos workers conectarao (obrigatorio)            |
| `--bind HOST:PORT` | Endereco de bind (ex: `0.0.0.0:9000`)               |
| `-i, --input`      | Arquivo da rede de entrada                          |
| `-o, --output`     | Salvar rede reduzida final                          |
| `-m, --metrics`    | Salvar metricas detalhadas em .json ou .csv         |
| `--max-rounds N`   | Limitar rodadas BSP                                 |
| `--strategy`       | Estrategia de particao (`round-robin`, default)     |
| `--token auto\|<base64>` | Autenticacao (secao 9.5)                      |
| `--log-format`     | `text` ou `json`                                    |

O coordinator bloqueia esperando os N workers conectarem antes de iniciar
o primeiro round BSP.

### 9.3 worker — Nó de calculo

```bash
relativist worker --coordinator HOST:PORT [--token <base64>]
```

| Flag                      | Descricao                                    |
|---------------------------|----------------------------------------------|
| `--coordinator HOST:PORT` | Endereco do coordinator (obrigatorio)        |
| `--token <base64>`        | Token de autenticacao (se coordinator exigir)|
| `--log-format`            | `text` ou `json`                             |

O worker conecta ao coordinator com retry automatico (exponencial, ate
30s total). Ele recebe particoes, reduz localmente, e devolve o resultado.

### 9.4 Exemplo: 1 coordinator + 2 workers numa maquina (loopback)

Use 3 terminais (ou `&` em background):

**Terminal 1 (coordinator):**
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

**Terminal 2 (worker 1):**
```bash
relativist worker --coordinator 127.0.0.1:9000
```

**Terminal 3 (worker 2):**
```bash
relativist worker --coordinator 127.0.0.1:9000
```

Apos os 2 workers conectarem, o coordinator executa o grid, imprime o
resumo, e todos os processos terminam. O arquivo `ep10k_metrics.json`
contem metricas detalhadas por rodada (bytes enviados/recebidos, tempo
de compute, merge, network, etc.).

### 9.5 Seguranca: autenticacao por token

Por padrao o bind em `0.0.0.0` sem token emite um aviso. Para habilitar
autenticacao (SPEC-10):

**Coordinator gera token automatico:**
```bash
relativist coordinator --workers 2 --bind 0.0.0.0:9000 \
  --token auto --token-file /tmp/rel-token \
  -i input.bin
```

O token base64 e impresso e salvo em `/tmp/rel-token`. Os workers devem
usar esse mesmo token:

```bash
TOKEN=$(cat /tmp/rel-token)
relativist worker --coordinator coord-host:9000 --token "$TOKEN"
```

---

## 10. Docker

### Construir a imagem

```bash
cd codigo/relativist
docker build -t relativist .
```

### Verificar

```bash
docker run --rm relativist --version
# relativist 0.9.0
```

### Usar com volume montado

Para ler/escrever arquivos, monte um volume com `-v`:

```bash
# Criar diretorio de trabalho
mkdir -p /tmp/relativist-data

# Gerar uma rede
docker run --rm -v /tmp/relativist-data:/data \
  relativist generate ep-annihilation -n 100 -o /data/ep100.bin

# Inspecionar
docker run --rm -v /tmp/relativist-data:/data \
  relativist inspect -i /data/ep100.bin

# Reduzir sequencialmente
docker run --rm -v /tmp/relativist-data:/data \
  relativist reduce -i /data/ep100.bin -o /data/ep100_reduced.bin

# Simular grid com 4 workers
docker run --rm -v /tmp/relativist-data:/data \
  relativist local -w 4 -i /data/ep100.bin \
  -o /data/ep100_grid.bin -m /data/metrics.json

# Aritmetica Church
docker run --rm relativist compute add 10 20

# Aritmetica Church distribuida
docker run --rm relativist compute mul 5 6 --workers 4

# Benchmark suite com CSV
docker run --rm -v /tmp/relativist-data:/data \
  relativist bench \
  --benchmark ep_annihilation \
  --sizes 100,500 \
  --workers 2,4 \
  --warmup 1 \
  --repetitions 3 \
  --csv-detail /data/detail.csv \
  --csv-summary /data/summary.csv
```

### Nota para Windows (Git Bash)

No Git Bash do Windows, prefixe o comando com `MSYS_NO_PATHCONV=1` para
evitar conversao automatica de caminhos Unix:

```bash
MSYS_NO_PATHCONV=1 docker run --rm -v "C:/Users/Filipe/data:/data" \
  relativist generate ep-annihilation -n 100 -o /data/ep100.bin
```

### Nota sobre memoria (peak_memory_bytes)

A metrica `peak_memory_bytes` le `/proc/self/status` (VmHWM), disponivel
apenas no Linux. No Docker (Linux container), funciona perfeitamente.
No Windows/Mac nativo, retorna 0.

---

## 11. Campanhas de Benchmark (Phase 1/2/3)

Esta secao documenta os **comandos exatos** usados para reproduzir a
campanha de benchmarks do TCC, dividida em tres fases progressivas:

| Fase    | Modo                    | Maquinas             | Dataset alvo |
|---------|-------------------------|----------------------|--------------|
| Phase 1 | Sequential + Local (in-process) | 1 maquina     | 2 260 datapoints |
| Phase 2 | TcpLocalhost via Docker Compose | 1 maquina     | 400 datapoints |
| Phase 3 | TcpNetwork em maquinas reais    | 2+ maquinas   | ate definir |

Todas as fases gravam os resultados em `results/phase{N}_detail.csv`,
`results/phase{N}_summary.csv` e `results/phase{N}_rounds.csv` com o
mesmo schema, de forma que a analise posterior pode cruzar as fases
diretamente.

### 11.1 Phase 1 — Sequential + Local (in-process)

Executa em um unico processo, sem TCP. Usa o comando `relativist bench`
nativo. E o baseline de referencia: se Phase 1 nao mostra speedup, nao
ha como Phase 2/3 ser melhor, ja que elas somam overhead de rede.

**Pre-requisitos:**
- `relativist` instalado (`cargo build --release`)
- `results/` criado na raiz do repositorio

**Comando completo (todos os profiles + encoding + data-bound):**

```bash
cd codigo/relativist
mkdir -p results

# Profile A (embarrassingly parallel) — EP annihilation
relativist bench \
  --benchmark ep_annihilation_con,ep_annihilation_dup,ep_annihilation \
  --sizes 100,500,1000,5000,10000,50000,100000,500000,1000000,5000000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_a_detail.csv \
  --csv-rounds results/phase1_profile_a_rounds.csv \
  --csv-summary results/phase1_profile_a_summary.csv

# Profile B (expansion + collapse) — CON-DUP expansion
relativist bench \
  --benchmark condup_expansion,mixed_net \
  --sizes 10,50,100,500,1000,5000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_b_detail.csv \
  --csv-rounds results/phase1_profile_b_rounds.csv \
  --csv-summary results/phase1_profile_b_summary.csv

# Profile C (sequential dependency) — DualTree, Erasure
relativist bench \
  --benchmark dual_tree,erasure_propagation \
  --sizes 4,6,8,10,12,14,16,18,20,22 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_profile_c_detail.csv \
  --csv-rounds results/phase1_profile_c_rounds.csv \
  --csv-summary results/phase1_profile_c_summary.csv

# Encoding + data-bound — Church arithmetic, TreeSum
relativist bench \
  --benchmark church_add,church_mul,tree_sum \
  --sizes 10,50,100,500,1000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/phase1_encoding_detail.csv \
  --csv-rounds results/phase1_encoding_rounds.csv \
  --csv-summary results/phase1_encoding_summary.csv
```

**Dicas:**

- `workers 0` e adicionado automaticamente pelo comando `bench` para
  gerar a linha de baseline `sequential`.
- O flag `--warmup 2` executa duas rodadas descartadas para estabilizar
  cache/JIT antes da medicao.
- Para rodadas mais rapidas durante desenvolvimento: `--repetitions 3
  --warmup 1`.

### 11.2 Phase 2 — Docker (TcpLocalhost)

Executa o protocolo BSP completo sobre TCP, porem com coordinator e
workers em containers na **mesma maquina** (loopback). Isola o custo
algoritmico + protocolo TCP, sem interferencia de rede fisica.

**Pre-requisitos:**
- Docker Desktop em execucao (`docker info` deve funcionar)
- `docker-compose.yml` presente em `codigo/relativist/`
- Binario `relativist` compilado em `target/release/`
- Python 3 disponivel (`python3`) para o script de orquestracao

**Execucao via script de orquestracao:**

```bash
cd codigo/relativist
bash scripts/bench_docker.sh
```

O script `bench_docker.sh`:

1. Constroi a imagem Docker (`relativist-worker` e `relativist-coordinator`)
2. Para cada benchmark, gera o input net uma vez
3. Para cada tamanho, mede o baseline sequencial nativo (fora do Docker)
4. Para cada (benchmark, tamanho, workers), executa `docker compose up`
   repetidamente com warmup + repeticoes
5. Verifica G1 estrutural (inspect seq vs inspect distributed)
6. Agrega em `results/phase2_{detail,summary,rounds}.csv`

**Flags do orquestrador:**

| Flag               | Efeito                                              |
|--------------------|-----------------------------------------------------|
| `--dry-run`        | Imprime o plano sem executar Docker                 |
| `--skip-build`     | Pula `docker compose build` (imagem ja pronta)      |
| `--skip-sequential`| Pula os baselines nativos (reusa os anteriores)     |

**Executar apenas uma parte (sem recomeco do zero):**

```bash
# Usar imagem ja construida e baselines ja medidos
bash scripts/bench_docker.sh --skip-build --skip-sequential
```

**Executar manualmente um unico config (debug):**

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

**Retomar uma campanha parcial:**

Se o `bench_docker.sh` for interrompido, os scripts `bench_docker_resume.sh`
e `bench_docker_resume2.sh` (presentes em `scripts/`) mostram o padrao
para re-rodar apenas os configs que falharam sem reprocessar os demais.
Ajuste o array `CONFIGS` no script para listar apenas os itens que
precisam ser executados.

### 11.3 Phase 3 — TcpNetwork (maquinas reais)

Executa o mesmo protocolo BSP em **maquinas fisicas diferentes**
conectadas por rede Ethernet real. Expoe o custo que nenhum outro
modo consegue reproduzir: latencia de RTT, throughput limitado,
jitter, contencao de NIC. E a campanha que transforma Relativist de
"implementacao correta de ICs distribuidos" em "evidencia empirica
sobre o custo real de distribuir reducao de ICs num grid".

> **Pre-requisito forte:** a Phase 3 so faz sentido depois que
> `v1_local_baseline` (tag `v0.10.0-bench`) esta congelada em
> `results/locked/v1_local_baseline/`. A medida principal e a
> subtracao `t_network = t_lan - t_localhost`, e essa subtracao exige
> que Phase 2 Docker ja tenha produzido `phase2_summary.csv` no mesmo
> binario. Nao comece Phase 3 sem ter a Phase 2 travada.

#### 11.3.0 O que Phase 3 mede (e o que nao mede)

**Medida primaria — overhead de LAN por subtracao.** Para cada triple
`(bench, size, workers)`:

```
t_network(bench, size, W) = t_lan(bench, size, W)  -  t_localhost(bench, size, W)
                            ^^^^^^^^^^^^^^^^^^^^^    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
                            produzido aqui           v1_local_baseline/phase2_summary.csv
```

`t_localhost` ja esta congelado em `v1_local_baseline`. Phase 3
produz `t_lan`. A diferenca e a fracao de wall clock atribuida a
latencia de fio + banda + jitter — tudo que Docker em loopback nao
consegue reproduzir. E o numero que o artigo do TCC reporta como
"custo de rede do grid".

Para a subtracao ser valida, *tudo* tem de ser identico menos a rede:
mesmo binario (tag `v0.10.0-bench`), mesmos bytes de input, mesma
estrategia de particao, mesmas 10 repeticoes, mesmo modo BSP
(lenient por padrao). Qualquer drift invalida o resultado.

**Medida secundaria — RTT por rodada sob strict BSP.** O modo
`--strict-bsp` (SPEC-05 R30a) ja foi validado em Phase 1: empiricamente
`cascade_cross(N) = N` rodadas e `dual_tree(d) = d` rodadas com
`workers >= 2`. Em Phase 3, as *mesmas* topologias produzem as
*mesmas* contagens de rodada — mas cada rodada agora inclui um RTT
real. Decompoe-se:

```
t_round_lan  ~=  t_round_localhost  +  RTT_round
              ~=  t_round_localhost  +  2 * RTT_wire * (split_msg + merge_msg)
```

Essa e a medida que permite ao TCC afirmar que caracterizou o
protocolo de grid sob rede realista.

**O que Phase 3 *nao* e:**
- **Nao e** um benchmark de throughput absoluto do BSP. Phase 1 ja
  mostrou que a cycle BSP tem overhead algoritmico alto (L1);
  Phase 3 nao tenta esconder isso.
- **Nao e** um teste sob rede adversa. Sem injecao de packet loss,
  sem WAN, sem contencao proposital de NIC. LAN = melhor caso
  distribuido.
- **Nao e** um teste do fix L2. L2 e validado por Phase 1 strict
  (empirico = teorico em `phase1_strict_rounds.csv`). Phase 3 apenas
  *usa* strict mode para extrair o sinal de RTT limpo.

#### 11.3.1 Hardware e rede necessarios

**Maquinas.** Para cumprir SPEC-09 R27 (MUST: "pelo menos 4 e 8
workers") voce precisa de **9 maquinas no total**: 1 coordinator + 8
workers. Alternativas:

| Opcao | Maquinas | Configs LAN | Cumpre R27? |
|---|---|---|---|
| A | 5 (1 coord + 4 workers) | W ∈ {1, 2, 4} | Parcial (4 sim, 8 nao — documentar exclusao) |
| B | 9 (1 coord + 8 workers) | W ∈ {1, 2, 4, 8} | Sim, integralmente |
| C | 3 (1 coord + 2 workers) | W ∈ {1, 2} | Nao cumpre. Apenas validacao de protocolo, nao conta como Phase 3 oficial |

O alvo para o TCC e a Opcao B. Opcao A e aceitavel com ressalva
documentada no manifest. Opcao C e so para provar que o protocolo
funciona sobre TCP real antes de agendar as maquinas de verdade.

**Requisitos por maquina** (iguais para coordinator e workers, o
ponto central e *homogeneidade*):

| Componente | Minimo | Motivo |
|---|---|---|
| CPU | 4 cores fisicos | Worker roda `reduce_all` single-threaded por particao |
| RAM | 4 GiB livres | Pior caso `ep_annihilation_con=5M` em W=1: ~1.5 GiB residente |
| Disco | 2 GiB livres | `output.bin` + `metrics.json` por run; coordinator acumula tudo |
| OS | Linux (Debian/Ubuntu/Fedora/Arch) | Tooling de campanha assume POSIX (ssh/scp/rsync) |
| NIC | 1 Gbps ethernet, mesmo switch, mesma VLAN | Wi-Fi introduz jitter 5-10x e contamina `t_network` |

**Homogeneidade.** A subtracao `t_network = t_lan - t_localhost`
assume que a *unica* diferenca entre Phase 2 e Phase 3 e a presenca
do fio. Se as maquinas de Phase 3 tiverem CPU diferente, RAM
diferente ou binario diferente, voce esta medindo uma mistura de
delta-hardware + delta-rede, e a isolacao cai por terra. O caminho
limpo:

1. Escolha um modelo de CPU para todas as maquinas (lab da
   universidade com desktops identicos e ideal).
2. Compile o binario **uma unica vez** na build host a partir da tag
   `v0.10.0-bench` e copie para todas as maquinas via `scp`. **Nao
   recompile em cada maquina** — variacoes de toolchain geram codigo
   diferente.
3. Antes de comecar, confirme que todas as maquinas reportam o mesmo
   `relativist --version` e o mesmo sha256 do binario.

**Topologia de referencia:**

```
      +---------------+
      | Coordinator   |  coord  (IP: 10.0.0.10)
      | (Maquina A)   |
      +-------+-------+
              |
              |  1 Gbps Ethernet, mesmo switch, mesma VLAN
              |
    +---------+-----------+------------+-----------+
    |         |           |            |           |
  +-+--+   +--+-+      +--+-+      +---+-+     +---+-+
  | W1 |   | W2 |  ..  | W4 |      | W6  |     | W8  |
  | .11|   | .12|      | .14|      | .16 |     | .18 |
  +----+   +----+      +----+      +-----+     +-----+
```

Regras:
- **Um switch, nao roteador.** Roteador adiciona hop IP e
  potencialmente firewall no caminho, com jitter extra.
- **Mesma VLAN / mesmo broadcast L2.** Workers e coordinator se
  alcancam via 1 ARP + 1 connect. Sem NAT entre eles.
- **Cabo, nao Wi-Fi.** Wi-Fi serve para sanity check; a campanha
  congelada tem de ser cabeada. Documentar o modelo da NIC no
  manifest.
- **Sem outro trafego pesado.** Videoconferencia no mesmo switch
  contamina a medida. Avise quem divide o switch sobre a janela da
  campanha.
- **IP estatico ou DHCP com lease reservado** para que os URIs
  `tcp://10.0.0.11:9001` nos scripts permanecam estaveis atraves de
  reboots.

**Medir o RTT baseline antes de qualquer coisa.** Isto e o piso —
nenhum `t_network` pode ser menor que `RTT * num_round_trips`. Se
for, ou ha bug de medicao ou o protocolo fez short-circuit em algum
lugar.

```bash
# Do coordinator, para cada worker
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    echo "=== $W ==="
    ping -c 20 -i 0.2 -q "$W" | tail -2
done
```

Anote o bloco `rtt min/avg/max/mdev` para cada worker num arquivo de
rascunho. Tipico LAN cabeado: `avg 0.1-0.5 ms`, `max 0.5-2 ms`,
`mdev < 0.2 ms`. Se `avg > 2 ms` ou `mdev > 1 ms`, investigue antes
de rodar a campanha — alguma coisa no switch esta saturada.

Meca tambem a banda efetiva com iperf3:

```bash
# Em cada worker (um de cada vez)
iperf3 -s &

# Do coordinator
iperf3 -c 10.0.0.11 -t 10
```

Registre `sender -> receiver` Gbps. Espere `>= 900 Mbps` num link
nominal de 1 Gbps. Abaixo disso, cheque cabos e negociacao da porta
(`ethtool eth0`).

#### 11.3.2 Setup de software

**1. Na build host (uma vez):**

```bash
# Clone e check out da tag travada
git clone git@github.com:andrade-filipe/relativist.git
cd relativist
git checkout v0.10.0-bench

# Use o mesmo toolchain que gerou v1_local_baseline
# (manifest.md registra: rustc 1.94.1 / cargo 1.94.1)
rustup override set 1.94.1 || rustup install 1.94.1
cargo build --release

# Verifique a versao e calcule o sha256 do binario
./target/release/relativist --version
sha256sum target/release/relativist
```

Guarde esse sha256 — ele entra no manifest da Phase 3 como
evidencia de que todas as maquinas rodaram o mesmo binario.

**2. Distribuir o binario para todas as maquinas** (sem recompilar):

```bash
# Da build host, para cada no do cluster
for HOST in 10.0.0.10 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    scp target/release/relativist "$USER@$HOST:/tmp/relativist"
    ssh "$USER@$HOST" "sudo install -m 755 /tmp/relativist /usr/local/bin/relativist && relativist --version"
done
```

Verifique que o sha256 de `/usr/local/bin/relativist` bate em todas
as maquinas:

```bash
for HOST in 10.0.0.10 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    echo -n "$HOST: "
    ssh "$USER@$HOST" "sha256sum /usr/local/bin/relativist" | awk '{print $1}'
done
```

Todos os hashes tem de ser identicos.

**3. Abrir porta 9000/tcp em cada maquina:**

```bash
# Ubuntu / Debian / Fedora com ufw
sudo ufw allow from 10.0.0.0/24 to any port 9000 proto tcp
sudo ufw reload

# Arch ou qualquer setup sem ufw
sudo iptables -A INPUT -p tcp --dport 9000 -s 10.0.0.0/24 -j ACCEPT
```

Nao exponha `9000/tcp` para a internet. A campanha usa token auth
(passo 5), mas bind em `0.0.0.0` num IP publico ainda e superficie
TCP visivel.

**4. Sincronizar relogio via NTP:**

```bash
# Em cada no
sudo apt install -y chrony   # ou dnf install -y chrony
sudo systemctl enable --now chronyd
chronyc tracking
```

Procure `System time : <valor menor que 0.100000 s>`. Wall clock e
medido localmente no coordinator via `std::time::Instant`, entao
drift entre relogios nao afeta os numeros diretamente — mas
correlacionar logs de coordinator + workers fica impossivel se o
drift for de segundos.

**5. Gerar e distribuir o token de auth:**

```bash
# No coordinator
relativist coordinator --workers 1 --bind 127.0.0.1:9999 \
    --token auto --token-file /tmp/rel-token \
    --dry-run 2>/dev/null || true   # gera o token e sai
chmod 600 /tmp/rel-token

# Copiar para cada worker sobre ssh
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    scp /tmp/rel-token "$USER@$W:/tmp/rel-token"
    ssh "$USER@$W" "chmod 600 /tmp/rel-token"
done
```

Alternativa: deixe o primeiro coordinator real (passo seguinte)
emitir o token com `--token auto --token-file /tmp/rel-token` e
copie o arquivo depois. **Nao reutilize token entre campanhas** —
gere um novo no inicio de cada Phase 3.

**6. Pre-estagiar inputs em todas as maquinas** (para eliminar a
variavel "transferencia de input" do caminho critico):

```bash
# No coordinator
mkdir -p ~/phase3/data
cd ~/phase3/data

# Matriz da campanha (mesma que Phase 2)
relativist generate ep-annihilation-con -n 500000  -o ep_con_500k.bin
relativist generate ep-annihilation-con -n 1000000 -o ep_con_1M.bin
relativist generate ep-annihilation-con -n 5000000 -o ep_con_5M.bin
relativist generate dual-tree -d 18 -o dual_tree_18.bin
relativist generate dual-tree -d 20 -o dual_tree_20.bin
relativist generate dual-tree -d 22 -o dual_tree_22.bin
relativist generate con-dup-expansion -n 1000 -o condup_1k.bin
relativist generate con-dup-expansion -n 5000 -o condup_5k.bin

# Subconjunto strict-BSP (opcional mas recomendado para RTT por rodada)
relativist generate cascade-cross -n 10   -o cc_10.bin
relativist generate cascade-cross -n 50   -o cc_50.bin
relativist generate cascade-cross -n 100  -o cc_100.bin
relativist generate cascade-cross -n 500  -o cc_500.bin
relativist generate cascade-cross -n 1000 -o cc_1k.bin
relativist generate dual-tree -d 6  -o dual_tree_6.bin
relativist generate dual-tree -d 10 -o dual_tree_10.bin
relativist generate dual-tree -d 14 -o dual_tree_14.bin
```

Rsync o diretorio `data/` para cada worker:

```bash
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    rsync -av --progress ~/phase3/data/ "$USER@$W:~/phase3/data/"
done
```

**Confira sha256 byte-identico em todas as maquinas:**

```bash
# Em cada no
cd ~/phase3/data && sha256sum *.bin > /tmp/input_sha256.txt

# Comparar todos contra o do coordinator
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    diff <(ssh "$USER@$W" "cat /tmp/input_sha256.txt") /tmp/input_sha256.txt \
        && echo "$W OK" || echo "$W DRIFT"
done
```

Qualquer drift aqui e **a causa #1 de bugs silenciosos em Phase 3**:
worker roda input diferente, G1 acusa resultado divergente, e voce
perde horas procurando no lugar errado. Pegue na verificacao
pre-flight, nao em tempo de execucao.

#### 11.3.3 Dry run — sanity checks antes da campanha

Antes de lancar a campanha de varias horas, rode **tres sanity
checks**. Se qualquer um falhar, corrija antes de continuar.

**Check 1 — TCP loopback com binario real.** Numa maquina, abra
coordinator + worker em dois terminais usando localhost e o token
real. Isola o binario antes de envolver a rede.

```bash
# Terminal 1 (coordinator loopback)
relativist coordinator \
    --workers 1 \
    --bind 127.0.0.1:9000 \
    --token "$(cat /tmp/rel-token)" \
    -i ~/phase3/data/ep_con_500k.bin \
    -o /tmp/out_loop.bin \
    -m /tmp/metrics_loop.json \
    --log-format json

# Terminal 2 (worker loopback)
relativist worker \
    --coordinator 127.0.0.1:9000 \
    --token "$(cat /tmp/rel-token)" \
    --log-format json
```

Esperado: coordinator emite `{"event":"grid_complete", ...}`, sai
com codigo 0, `/tmp/out_loop.bin` existe, `/tmp/metrics_loop.json`
tem um array `rounds` de comprimento 1 (modo lenient).

**Check 2 — Smoke test cross-machine.** Do coordinator real:

```bash
# No coordinator (10.0.0.10)
relativist coordinator \
    --workers 2 \
    --bind 0.0.0.0:9000 \
    --token "$(cat /tmp/rel-token)" \
    -i ~/phase3/data/ep_con_500k.bin \
    -o /tmp/out_smoke.bin \
    -m /tmp/metrics_smoke.json \
    --log-format json
```

Coordinator loga `waiting for 2 workers`. Em paralelo (ssh nos
workers):

```bash
# Em 10.0.0.11 e 10.0.0.12
TOKEN=$(cat /tmp/rel-token)
relativist worker --coordinator 10.0.0.10:9000 --token "$TOKEN"
```

Esperado: coordinator sai com 0, os dois workers saem com 0,
`/tmp/out_smoke.bin` + `/tmp/metrics_smoke.json` no coordinator.

Se em vez disso:

| Sintoma | Diagnostico |
|---|---|
| Worker trava em `connecting to 10.0.0.10:9000` | Firewall ou rota. Teste `telnet 10.0.0.10 9000` do worker. |
| Worker conecta mas falha no register | Token mismatch. Regenere e redistribua. |
| Coordinator trava em `waiting for N workers` | Register recebido mas incompleto. Cheque stderr do coordinator. |
| Coordinator sai com protocol error | Binario heterogeneo. Reverifique sha256. |

**Check 3 — G1 equivalencia contra referencia sequencial.**

```bash
# No coordinator
relativist reduce -i ~/phase3/data/ep_con_500k.bin -o /tmp/out_seq.bin

# Comparar (estrutural rapido)
relativist inspect -i /tmp/out_smoke.bin > /tmp/dist.txt
relativist inspect -i /tmp/out_seq.bin   > /tmp/seq.txt
diff /tmp/dist.txt /tmp/seq.txt && echo "G1 OK" || echo "G1 FAIL"
```

Se `G1 FAIL`, **pare**. Ou o binario esta quebrado, ou os bytes do
input divergiram entre as maquinas, ou o protocolo tem corrupcao
silenciosa. Reconfira rodando `relativist bench --mode local -w 2`
no coordinator e comparando. Nao rode a campanha com G1 quebrado.

#### 11.3.4 Orquestracao da campanha

Phase 3 precisa de ~400 runs: 8 bench x size × 4 worker configs × 10
reps + 8 baselines sequenciais × 10 reps, mesma matriz que Phase 2.
Disparar manualmente e impossivel. Duas opcoes:

**Opcao A — script SSH (recomendado para <= 9 nos).** O padrao
espelha `scripts/bench_docker_resume2.sh`, trocando
`docker compose up` por `ssh worker_N relativist worker ...`. O
driver precisa, para cada triple `(bench, size, workers)`:

1. Limpar artefatos da rep anterior no coordinator e nos workers.
2. Em cada worker: lancar `relativist worker --coordinator ...` em
   background via `ssh`, redirecionando stdout/stderr para
   `/tmp/worker_N.log`.
3. No coordinator: rodar `relativist coordinator --workers N ...`
   (chamada bloqueante).
4. Quando o coordinator sai, coletar `metrics.json` local e
   `/tmp/worker_N.log` de cada worker via `scp`.
5. Rodar `relativist inspect` e comparar com a referencia sequencial
   (produzida uma vez por input no inicio da campanha).
6. Anexar uma linha ao CSV em progresso.
7. Repetir para a proxima repeticao.

Esqueleto minimo (a ser refinado num driver
`scripts/bench_phase3_locked.sh` proprio):

```bash
#!/usr/bin/env bash
set -euo pipefail

COORD_IP=10.0.0.10
COORD_BIND=0.0.0.0:9000
TOKEN=$(cat /tmp/rel-token)
WORKERS_POOL=(10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14 \
              10.0.0.15 10.0.0.16 10.0.0.17 10.0.0.18)

DATA_DIR=~/phase3/data
OUT_DIR=~/phase3/results/locked/v1_lan_baseline
RAW_DIR=$OUT_DIR/raw/phase3
mkdir -p "$OUT_DIR" "$RAW_DIR"

BENCH_SIZES=(
    "ep_annihilation_con:500000:ep_con_500k.bin"
    "ep_annihilation_con:1000000:ep_con_1M.bin"
    "ep_annihilation_con:5000000:ep_con_5M.bin"
    "dual_tree:18:dual_tree_18.bin"
    "dual_tree:20:dual_tree_20.bin"
    "dual_tree:22:dual_tree_22.bin"
    "condup_expansion:1000:condup_1k.bin"
    "condup_expansion:5000:condup_5k.bin"
)
WORKER_COUNTS=(1 2 4 8)
REPS=10

run_one() {
    local bench=$1 size=$2 input=$3 workers=$4 rep=$5
    local label="${bench}_${size}_w${workers}_r${rep}"
    local metrics="$RAW_DIR/${label}.json"
    local out="$RAW_DIR/${label}.bin"

    # 1. Lanca workers nos primeiros $workers nos do pool
    for i in $(seq 0 $((workers-1))); do
        local host="${WORKERS_POOL[$i]}"
        ssh "$USER@$host" "pkill -x relativist || true; nohup relativist worker \
            --coordinator $COORD_IP:9000 --token '$TOKEN' \
            --log-format json > /tmp/worker_${i}.log 2>&1 &"
    done

    # 2. Espera curta para os workers conectarem
    sleep 1

    # 3. Roda o coordinator ate terminar
    relativist coordinator \
        --workers "$workers" \
        --bind "$COORD_BIND" \
        --token "$TOKEN" \
        -i "$DATA_DIR/$input" \
        -o "$out" \
        -m "$metrics" \
        --log-format json > "$RAW_DIR/${label}_coord.log" 2>&1

    # 4. Coleta logs dos workers
    for i in $(seq 0 $((workers-1))); do
        scp "$USER@${WORKERS_POOL[$i]}:/tmp/worker_${i}.log" \
            "$RAW_DIR/${label}_worker_${i}.log"
    done

    # 5. G1 estrutural contra referencia sequencial
    local seq_ref="$DATA_DIR/sequential_ref/${input}.seq.bin"
    diff <(relativist inspect -i "$out") \
         <(relativist inspect -i "$seq_ref") > /dev/null \
        && echo "CORRECT" || echo "FAIL"
}

for triple in "${BENCH_SIZES[@]}"; do
    IFS=':' read -r bench size input <<< "$triple"

    # Baseline sequencial: 10 reps no coordinator, sem workers
    for rep in $(seq 1 $REPS); do
        relativist reduce -i "$DATA_DIR/$input" \
            -o "$RAW_DIR/${bench}_${size}_seq_r${rep}.bin" \
            -m "$RAW_DIR/${bench}_${size}_seq_r${rep}.json"
    done

    # Distribuido: 10 reps por worker count
    for W in "${WORKER_COUNTS[@]}"; do
        for rep in $(seq 1 $REPS); do
            run_one "$bench" "$size" "$input" "$W" "$rep"
        done
    done
done

# Agregar JSONs em phase3_{detail,rounds,summary}.csv
python3 scripts/aggregate_phase3.py "$RAW_DIR" "$OUT_DIR"
```

> **Nota:** `scripts/aggregate_phase3.py` **ainda nao existe** — e o
> analogo para Phase 3 da logica de parsing de metricas dentro de
> `bench_phase2_locked.sh`. Escrever esse helper faz parte da
> preparacao de Phase 3. O schema de saida tem de bater com
> `phase2_{detail,rounds,summary}.csv` byte-por-byte, senao a
> subtracao quebra.

**Opcao B — Ansible playbook.** Se voce tem 9+ maquinas e planeja
rerodar a campanha multiplas vezes (ou rodar em dois labs
diferentes), um playbook Ansible e mais robusto. Estrutura minima:

```yaml
# phase3.yml
- name: Provisionar todos os nos
  hosts: all
  tasks:
    - copy: { src: target/release/relativist, dest: /usr/local/bin/relativist, mode: '0755' }
    - copy: { src: data/, dest: ~/phase3/data/ }
    - copy: { src: /tmp/rel-token, dest: ~/.rel-token, mode: '0600' }

- name: Rodar campanha Phase 3
  hosts: coordinator
  tasks:
    - shell: ./scripts/bench_phase3_locked.sh
```

Overkill para 5 maquinas (Opcao A e mais limpo), mas paga o custo
se o lab tem 30+ nos identicos.

**Tratamento de falhas durante a campanha.** Redes falham. Workers
crasham. O driver tem de tratar:

| Falha | Sintoma | Resposta |
|---|---|---|
| Worker inacessivel via ssh | `ssh: connect to host X port 22` | Retry 3x com backoff 10s. Se persistir, abortar e documentar. |
| Worker crashou no meio do run (OOM, kill) | Coordinator trava esperando | Coordinator precisa de timeout por run (`--timeout 1800`). No timeout, logar `TIMEOUT`, marcar rep como falhada, continuar. Reps falhadas tem de ser rerodadas antes do lock. |
| `output.bin` ausente no disco | Esperado mas nao escrito | Shutdown race L7-like (ver PHASE2-FINDINGS.md §6.1). Cheque log do coordinator por SIGTERM. |
| G1 falha num run | `diff` nao vazio | **Pare a campanha.** Um `correct=false` invalida o snapshot. Investigue: rebuild, recheque sha256 dos inputs, rode em `local` mode para isolar. |
| CV > 0.30 num config | Triage pos-campanha | Rerode o config com mais 10 reps. Se ainda > 0.30 apos 20 reps, marque `exclude` em `cv_triage.md` e footnote no artigo. |

**Nao passe `--continue` por cima de uma falha G1.** A disciplina do
snapshot e "todos os 4000+ reps corretos ou o snapshot e invalido".
Uma unica falha e bug que precisa ser corrigido antes de travar os
dados.

**Estimativa de wall clock.** Phase 2 levou 43 min 42 s para 400
runs num ThinkPad Docker-Desktop sobrealocado. Phase 3 em bare-metal
LAN sera **mais lento por run** (RTT real adiciona 5-20 ms por
rodada, as vezes mais para `dual_tree=22` strict que tem centenas de
rodadas), mas **mais rapido em agregado** porque cada maquina tem
CPU dedicada.

Budget aproximado:
- Baselines sequenciais (80 runs): ~5-10 min total.
- Runs distribuidos em W=1: Phase 2 + 10-30% de delay induzido pela rede.
- Runs em W=2/4/8: dominados por tamanho de payload split/merge; LAN
  Gbps aguenta o pico de 500 MB facilmente, entao ~Phase 2 + (num_rounds × RTT).

**Planeje 1.5-4 horas de wall clock sem supervisao.** Abra uma
janela de 6 horas para ter folga. Nao lance Phase 3 se alguem precisa
do switch nas proximas 2 horas.

#### 11.3.5 Coleta de dados

**Schema de saida.** Os arquivos
`phase3_{detail,rounds,summary}.csv` sob
`results/locked/v1_lan_baseline/` tem de bater com o schema de Phase
2 byte-por-byte (mesmo header, mesma ordem de colunas, mesma
precisao numerica), para que ferramentas de analise consumam os dois
intercambiavelmente. Referencia:
`results/locked/v1_local_baseline/phase2_summary.csv`.

Especificamente `phase3_summary.csv` precisa do header:

```
benchmark,input_size,mode,workers,repetitions,all_correct,wall_clock_mean,wall_clock_std,wall_clock_median,wall_clock_min,wall_clock_max,mips_mean,speedup_mean,efficiency_mean,overhead_ratio_mean,cv
```

Com `mode ∈ {sequential, tcp_network}`. Nao introduza
`tcp_network_lan` ou similar — e apenas `tcp_network` por SPEC-09
R25.

**Retencao de JSONs por run.** Cada run emite `metrics.json` do
lado do coordinator. Guarde **todos** em `raw/phase3/`:

```
results/locked/v1_lan_baseline/raw/phase3/
    ep_annihilation_con_500000_w1_r01.json
    ep_annihilation_con_500000_w1_r01_coord.log
    ep_annihilation_con_500000_w1_r01_worker_0.log
    ep_annihilation_con_500000_w1_r02.json
    ...
```

Evidencia forense se algum revisor perguntar "a rep 7 de
`ep_con_1M` foi contaminada por um spike?". Orcamento: ~5-20 KB por
JSON, ~100-500 KB por log de worker; 400 runs x 3 arquivos x 500 KB
≈ 600 MB. Reserve 2 GB.

**CV triage.** Apos a campanha, rode o mesmo script que Phase 1 e
Phase 2:

```bash
python3 scripts/cv_triage.py \
    --input results/locked/v1_lan_baseline/phase3_summary.csv \
    --output results/locked/v1_lan_baseline/cv_triage_phase3.md \
    --threshold 0.15
```

Padrao esperado sob LAN:
- Inputs pequenos (`condup_expansion=1000`) tem CV inflado porque os
  runs sao tao curtos que o jitter do RTT domina. Marcar `keep` com
  footnote.
- Inputs grandes (`ep_con=5M`, `dual_tree=22`) devem ter CV < 0.05.
  Se nao tem, algo no switch estava mal durante essas reps.
- `rerun` e para CV > 0.30 em qualquer tamanho. `exclude` so para
  CV > 0.50 que nao se reproduz apos 20 reps.

**Manifest do snapshot congelado.** Crie
`results/locked/v1_lan_baseline/manifest.md` com a mesma estrutura
de `v1_local_baseline/manifest.md` mas especifica para Phase 3:

```markdown
# v1_lan_baseline — Campaign Manifest

**Status:** COMPLETE — Phase 3 LAN campaign finished on <data>.

## Provenance
- Git tag: v0.10.0-bench (mesmo binario que v1_local_baseline)
- Commit SHA: <SHA>
- Binary sha256: <sha256 identico em cada no>
- Operator: Filipe Andrade Nascimento
- Campaign start: <timestamp>
- Campaign end: <timestamp>

## Cluster (LAN hardware)
- Switch: <modelo, velocidade, gerenciavel?>
- Coordinator (Maquina A): <CPU, RAM, OS, NIC>
- Workers (Maquinas B-I): <mesmo que coord se homogeneo; por-maquina se nao>
- Network: 1 Gbps Ethernet, VLAN unica, sem outros hosts ativos
- RTT baseline medido (ping -c 20):
  - A -> B: <min/avg/max/mdev ms>
  - ...
- Banda baseline medida (iperf3 -t 10):
  - A -> B: <Gbps>
  - ...

## Campaign knobs
- Bench × size: mesma matriz que v1_local_baseline Phase 2 (8 combos)
- Worker counts: {1, 2, 4, 8}  (ou {1, 2, 4} sob Opcao A)
- Repetitions: 10
- Mode: tcp_network

## Correctness methodology
- Estrutural via `relativist inspect` contra a referencia sequencial
  de v1_local_baseline Phase 2.

## Checksums (sha256)
<gerar dos CSVs finais>

## Row counts
<mesma tabela de sanity que Phase 2>

## Relationship to v1_local_baseline
- Phase 1 lenient (v1_local_baseline): mesmo
- Phase 2 Docker   (v1_local_baseline): mesmo
- Phase 3 LAN      (este manifest):     novo

A subtracao t_network = t_lan - t_localhost e computada por triple
(bench, size, workers) na geracao das figuras do artigo, nao aqui.
Este manifest so garante que t_lan foi medido sob as condicoes
declaradas.
```

**Congelar o snapshot.** Depois do CV triage passar e o manifest
estar completo, um commit atomico (mesma disciplina de
`v1_local_baseline`):

```bash
cd codigo/relativist
git add results/locked/v1_lan_baseline/
git commit -m "data: freeze v1_lan_baseline snapshot — Phase 3 LAN campaign"

# Nova tag (nao mover v0.10.0-bench)
git tag -a v0.11.0-lan -m "Phase 3 LAN baseline frozen"
git push origin main
git push origin v0.11.0-lan
```

**`v0.10.0-bench` fica onde esta.** Phase 3 ganha tag propria para
que os dois baselines (local e LAN) sejam referenciaveis
independentemente.

#### 11.3.6 Analise pos-campanha

**1. Tabela de overhead de rede (headline).** Subtracao
`(bench, size, workers, t_localhost, t_lan, t_network, frac)`:

```python
import csv

def load_summary(path):
    rows = {}
    with open(path) as f:
        for row in csv.DictReader(f):
            key = (row['benchmark'], row['input_size'], row['mode'], row['workers'])
            rows[key] = row
    return rows

local = load_summary('v1_local_baseline/phase2_summary.csv')
lan   = load_summary('v1_lan_baseline/phase3_summary.csv')

print("benchmark,size,workers,t_localhost,t_lan,t_network,net_frac")
for key in lan:
    if key[2] != 'tcp_network':
        continue
    loc_key = (key[0], key[1], 'tcp_localhost', key[3])
    if loc_key not in local:
        continue
    t_local = float(local[loc_key]['wall_clock_mean'])
    t_lan   = float(lan[key]['wall_clock_mean'])
    t_net   = t_lan - t_local
    frac    = t_net / t_lan if t_lan > 0 else 0
    print(f"{key[0]},{key[1]},{key[3]},{t_local:.4f},{t_lan:.4f},{t_net:.4f},{frac:.3f}")
```

Forma esperada do output: `net_frac ∈ [0.0, 0.5]` para a maioria dos
configs. Valores > 0.5 significam que rede domina (esperado para
inputs pequenos como `condup_expansion=1000`). Valores < 0.05
significam que o LAN esta tao rapido que fica indistinguivel de
loopback (improvavel em 1 Gbps, possivel em 10 Gbps).

**Valores negativos sao bug.** Se `t_lan < t_localhost`, a
subtracao esta errada — investigue se o baseline Phase 2 teve
contencao que Phase 3 nao teve.

**2. RTT por rodada (strict BSP).** Para o subconjunto strict
(`cascade_cross` e `dual_tree` pequenos):

```
t_round_lan           = t_lan        / rounds
t_round_local_strict  = t_local_strict / rounds   # phase1_strict_summary.csv
RTT_por_rodada        = t_round_lan - t_round_local_strict
```

Cross-check contra o ping RTT da Secao 11.3.1. O overhead por
rodada do protocolo e aproximadamente `2 × RTT_ping × log2(workers)`
(um broadcast-like down + um gather-like up por rodada). Se
`RTT_por_rodada ≈ 4 × RTT_ping` em W=4, e consistente. Se
`RTT_por_rodada >> 10 × RTT_ping`, tem ineficiencia em algum lugar.

**3. Integracao no artigo do TCC.** O artigo (`artigo/tcc_pt_br.tex`,
Secao 5 "Resultados e Discussoes") precisa de **tres figuras novas**
a partir dos dados de Phase 3:

- **Figura N — decomposicao de overhead.** Barras empilhadas por
  `(bench, W)` mostrando `t_seq` vs `t_local − t_seq` vs
  `t_localhost − t_local` vs `t_lan − t_localhost`. A ARG-004 no
  seu formato mais concreto.
- **Figura N+1 — teto de speedup em LAN.** Linhas de speedup vs
  workers, uma por modo (`local`, `tcp_localhost`, `tcp_network`).
  O gap entre `local` e `tcp_network` mostra o custo total de
  distribuir.
- **Figura N+2 — RTT por rodada (strict BSP).** Para cascade_cross
  e dual_tree sob strict, plot `rounds` no eixo x e
  `t_round_lan / t_round_localhost` no eixo y. Linha plana perto de
  1.0 significa que o protocolo nao amplifica; linha crescente
  indica custo que se acumula por rodada.

O REDATOR possui o LaTeX/TikZ final; esta secao so documenta de
onde os dados vem.

#### 11.3.7 Checklist — antes, durante, depois

**Antes (T - 24 h a T):**

- [ ] `v1_local_baseline` congelado e tag confirmada: `git show v0.10.0-bench`
- [ ] 5+ maquinas disponiveis, mesmo modelo de CPU, mesmo switch
- [ ] Build host com `rustc 1.94.1` + check out limpo de `v0.10.0-bench`
- [ ] `cargo build --release` sucede na build host
- [ ] sha256 do binario registrado
- [ ] Binario copiado para `/usr/local/bin/relativist` em todos os nos; versao bate
- [ ] Todos os inputs pre-gerados e rsync'd para cada worker; sha256 byte-identico
- [ ] Firewall `9000/tcp` aberto entre coordinator e cada worker
- [ ] NTP syncado em todos os nos (`chronyc tracking` com drift < 100 ms)
- [ ] `ping` e `iperf3` baseline anotados num arquivo de rascunho
- [ ] Token de auth gerado uma vez e distribuido via `scp`
- [ ] Sanity checks Secao 11.3.3 (1, 2, 3) todos verdes
- [ ] `scripts/bench_phase3_locked.sh` escrito, revisado, com modo `--dry-run`
- [ ] `scripts/aggregate_phase3.py` escrito (analogo Phase 3 de Phase 2)
- [ ] >= 5 GiB livres em disco no coordinator (para `raw/phase3/`)
- [ ] Ninguem mais precisa do switch nas proximas 6 horas
- [ ] Laptop que roda a campanha ligado na tomada

**Durante:**

- [ ] Lancar `bench_phase3_locked.sh` dentro de uma sessao `tmux`/`screen`
- [ ] Monitorar os 3 primeiros runs a olho; confirmar saida limpa
- [ ] Checar CV no primeiro config completo (apos ~10 reps); se > 0.15 em input grande, investigue imediatamente
- [ ] Nao abrir browser, jogo ou video call em nenhuma maquina do cluster
- [ ] Se algo falhar, nao retry cego — diagnostique, conserte, rode so os configs falhados

**Depois:**

- [ ] Todos os 400+ runs com `correct=true`
- [ ] CV triage completo; dispositions registradas em `cv_triage_phase3.md`
- [ ] `manifest.md` preenchido com timestamps, hardware, checksums reais
- [ ] `phase3_{detail,rounds,summary}.csv` presentes; row counts conferem
- [ ] `.gitattributes` sob `codigo/relativist/` ja fixa `results/locked/**` em LF — nao regenerar CSVs apos commit
- [ ] Commit atomico: `git add results/locked/v1_lan_baseline/ && git commit -m "data: freeze v1_lan_baseline snapshot"`
- [ ] Nova tag `v0.11.0-lan` apontando para o commit do snapshot
- [ ] Submodule pointer no repo top-level do TCC bumped
- [ ] `docs/PHASE3-FINDINGS.md` rascunhado com a tabela de overhead
- [ ] `progress.md` (nivel top e nivel Relativist) atualizados
- [ ] Avisar o orientador (Yuri) que Phase 3 esta pronta + apontar para as 3 figuras novas

#### 11.3.8 Riscos conhecidos e mitigacoes

| Risco | Prob | Impacto | Mitigacao |
|---|---|---|---|
| Switch compartilhado satura durante o run | Medio | Alto | Avisar janela; rodar off-hours; pegar switch quieto |
| Uma maquina com binario diferente | Baixo | Critico | Verificacao de sha256 no pre-flight (Secao 11.3.2 passo 2) |
| Coordinator morto por OOM em input grande | Baixo | Alto | Maquina com >= 4 GiB livres; `sysctl vm.overcommit_memory=1` |
| Fallback Wi-Fi no meio da campanha | Medio | Alto | `nmcli radio wifi off` em cada no antes de comecar |
| Leak do token num filesystem compartilhado | Baixo | Medio | `chmod 600 /tmp/rel-token` em todo lugar; regenerar apos campanha |
| Firewall bloqueia depois de reboot | Medio | Medio | Persistir regras `ufw`/`iptables` em `/etc/` |
| Drift de relogio > 5 s entre nos | Baixo | Baixo | `chrony` em cada no; verificar antes |
| Off-by-one na matriz do driver | Alto (primeira execucao) | Medio | Modo `--dry-run` que imprime a matriz sem executar |
| Resultados Phase 3 contradizem Phase 2 baseline | Baixo | Critico | Rerodar subset de Phase 2 nas maquinas de LAN (sem Docker) para confirmar portabilidade do baseline |
| Janela de 6 horas acaba no meio da campanha | Medio | Baixo | Driver tem de ser re-iniciavel. No restart, pular configs cujo `.json` ja existe em `raw/phase3/` |

#### 11.3.9 Referencias cruzadas

- **SPEC-09 Benchmarks** (`specs/SPEC-09-benchmarks.md`): R25 (modos), R27 (TcpNetwork MUST), R31 (10 reps), R39b (schema de `rounds.csv`), Secao 5.8 (por que TcpNetwork e mandatorio).
- **SPEC-07 Deployment** (`specs/SPEC-07-*.md`): R41 procedimento bare-metal (referenciado por R27).
- **SPEC-05 Grid Loop** (`specs/SPEC-05-merge.md`): R30a lenient vs strict BSP. Phase 3 usa lenient na matriz principal e strict no subconjunto `cascade_cross`/`dual_tree` pequenos.
- **SPEC-06 Protocol** (`specs/SPEC-06-*.md`): wire format, register handshake, validacao de token.
- **SPEC-10 Security** (`specs/SPEC-10-*.md`): Secao 3 auth por token, Secao 4 modelo de 3 niveis.
- **PHASE1-FINDINGS.md Secao L2** (este repo): fix arquitetural que torna strict BSP observavel em Phase 3.
- **PHASE2-FINDINGS.md Secao 7** (este repo): descricao do snapshot `v1_local_baseline` Phase 2 que Phase 3 subtrai.
- **v1_local_baseline manifest** (`results/locked/v1_local_baseline/manifest.md`): referencia para a estrutura e a disciplina de provenance do snapshot Phase 3.

### 11.4 Tabela de Correspondencia entre Fases

| Comando base             | Modo no CSV      | Origem do overhead                 |
|--------------------------|------------------|------------------------------------|
| `relativist reduce`      | `sequential`     | Nenhum (baseline)                  |
| `relativist local -w N`  | `local`          | Particionamento + merge in-process |
| `docker compose up`      | `tcp_localhost`  | + serializacao + TCP loopback      |
| `coordinator`/`worker` LAN | `tcp_network`  | + RTT de rede fisica + jitter      |

Isso permite isolar a contribuicao de cada camada no `overhead_ratio`
reportado no summary CSV.

### 11.5 Limitacoes Conhecidas

Tres limitacoes praticas apareceram durante as campanhas locais e de
Phase 2 (Docker) e todas foram mitigadas. O historico completo fica em
`docs/PHASE1-FINDINGS.md` e `docs/PHASE2-FINDINGS.md`; esta secao resume
o estado atual.

- **L2 (loop BSP colapsado em uma rodada) — RESOLVIDO via strict BSP
  mode em v0.10.0-bench.** Ate v0.9.x, `run_grid` em `src/merge/grid.rs`
  rodava `reduce_all(&mut merged_net)` na Fase 4 (RESOLVE BORDERS) apos
  cada merge, o que esgotava completamente a fila de redexes do net
  mergido — incluindo cascatas cross-partition recem-criadas pela
  resolucao das border redexes. Efeito colateral: todo run terminava em
  exatamente uma rodada BSP, independentemente da topologia, tornando
  ilusoria qualquer medicao de "custo por rodada" e divergindo do spec
  (SPEC-09 prometia "Rounds (grid) = d (minimum)" para DualTree). A
  correcao adiciona um modo opt-in `strict_bsp=true` ao `GridConfig`
  que substitui `reduce_all` por um novo `reduce_border_once` em
  `src/reduction/engine.rs`: a fila atual e processada exatamente uma
  vez, e quaisquer novas cascatas geradas ficam enfileiradas para a
  proxima rodada. Assim, nets com cascatas cross-partition iteram
  genuinamente ate a normal form, preservando G1 (SPEC-01) em ambos os
  modos. O modo default continua sendo `strict_bsp=false` (lenient),
  zero regressao nos 643+ testes existentes. A baseline v1 usa lenient
  como padrao; `cascade_cross` (todos os tamanhos) e `dual_tree`
  (tamanhos 6/10/14) tem dados adicionais em modo strict em
  `results/locked/v1_local_baseline/phase1_strict_rounds.csv` para a
  Phase 3 LAN. Veja `specs/SPEC-05-merge.md` Secao "Lenient vs Strict
  BSP" e use `--strict-bsp` na CLI para ativar.

- **L6 (teto de payload do protocolo) — RESOLVIDO.** Em v0.9.0, o
  protocolo impunha um limite de 256 MiB por frame
  (`DEFAULT_MAX_PAYLOAD_SIZE` em `src/protocol/frame.rs`), o que
  bloqueava 4 das 40 configuracoes de Phase 2: `dual_tree=22 w=1` e
  `ep_annihilation_con=5M w={1,2,4}`. A causa raiz era dupla: o
  `ContiguousIdStrategy` atribuia os agentes de id mais alto ao ultimo
  worker, forcando um `Vec<PortRef>` de tamanho total da rede mesmo
  quando aquele worker possuia poucos slots vivos; e o teto de 256 MiB
  era um guard-rail anti-DoS sem contrapartida na propriedade de
  confluencia do modelo IC. O fix tem duas partes ortogonais. **(a)**
  `CompactSubnet` em `src/partition/compact.rs` e um adaptador de
  `serialize_with`/`deserialize_with` em `Partition::subnet` que serializa
  apenas agentes vivos na forma `(id, agent, [ports; 3])` e reconstroi o
  arena denso no receptor — roundtrip preserva `agents`, `ports`,
  `redex_queue`, `next_id` e `root` byte-por-byte. **(b)** O cap foi
  elevado de 256 MiB para 1 GiB. Juntos, eliminam o overhead fixo do
  layout dense-indexed e comportam as redes totalmente densas que
  precisam legitimamente de mais de 256 MiB por frame. Pos-fix: Phase 2
  roda 40 de 40 configs com G1 = 100%, e benchmarks locais ganham
  40-100% de speedup nos casos onde o padding era dominante (ver
  `results/post_fix/B3_comparison.md`).

- **L7 (shutdown race do coordinator) — MITIGADO no driver.** Rodar
  `docker compose up --abort-on-container-exit --exit-code-from coordinator`
  matava o coordinator com SIGTERM (e depois SIGKILL, exit 137) assim
  que o primeiro worker saia, antes dele terminar de persistir
  `metrics.json` e `output.bin`. A reducao completava corretamente,
  mas os artefatos em disco nunca eram escritos. A mitigacao em
  `scripts/bench_docker_resume2.sh::run_docker_cycle()` usa
  `docker compose up -d` (detached) e bloqueia em
  `docker wait relativist-coordinator-1` ate o coordinator sair
  sozinho, sem nenhum flag de abort. Nao requer mudanca no binario;
  um SIGTERM handler interno no coordinator e um hardening opcional
  registrado em ROADMAP.

Para detalhes, causa raiz e implicacoes para Phase 3, consulte
`docs/PHASE2-FINDINGS.md` (Secao 3 para o historico L6/L7, Secao 6
para o fix). A trilha de evolucao do protocolo (workers dinamicos
habilitados por confluencia) esta em `docs/ROADMAP.md` itens 2.2 e
2.3.

#### 11.5.1 Verificacao G1 completa (abordagem B, opcional, overnight) para `condup_expansion` em 10k / 50k

A baseline `v1_local_baseline` usa a **abordagem A** como padrao para
`condup_expansion` em tamanhos grandes (10_000 e 50_000 agentes):
`--skip-g1` desliga o teste de isomorfismo estrutural (`nets_isomorphic`)
e mantem apenas o *weak check* — igualdade de contagem de agentes,
redexes e totais de cada regra entre a reducao sequencial e a do grid.
Esse weak check detecta qualquer divergencia de tamanho na normal form,
mas nao prova identidade ponto-a-ponto de topologia.

A razao do default e pragmatica: `nets_isomorphic` faz backtracking
O(N!) sobre o grafo, e `condup_expansion` e justamente o benchmark de
Perfil B onde cada agente inicial explode em varios filhos apos a
cascata CON-DUP, produzindo redes densas de dezenas de milhares de
agentes na normal form. Em tamanho 10k a verificacao completa nao
termina em tempo de teste integrado, e em 50k e intratavel.

Se voce quiser **fortalecer** (mas nao substituir) o weak check da
baseline v1 com uma evidencia adicional, rode os comandos da
**abordagem B** abaixo com a maquina ociosa — literalmente deixe o
computador reduzindo durante a noite. Nao precisa repetir 10 vezes
(1 repeticao e o suficiente para validar a topologia); o tempo
dominante e o proprio isomorfismo, nao a reducao:

```bash
mkdir -p results/optional

# Abordagem B — G1 completo para condup_expansion(10000)
# Expectativa: varias horas. Rode com a maquina ociosa.
./target/release/relativist bench \
    --benchmark condup_expansion \
    --sizes 10000 \
    --workers 2 \
    --repetitions 1 \
    --warmup 0 \
    --mode local \
    --csv-detail  results/optional/condup_10k_fullg1_detail.csv \
    --csv-rounds  results/optional/condup_10k_fullg1_rounds.csv \
    --csv-summary results/optional/condup_10k_fullg1_summary.csv

# Abordagem B — G1 completo para condup_expansion(50000)
# Expectativa: potencialmente intratavel (>12h). Documente o timeout
# como evidencia de intratabilidade se nao completar.
./target/release/relativist bench \
    --benchmark condup_expansion \
    --sizes 50000 \
    --workers 2 \
    --repetitions 1 \
    --warmup 0 \
    --mode local \
    --csv-detail  results/optional/condup_50k_fullg1_detail.csv \
    --csv-rounds  results/optional/condup_50k_fullg1_rounds.csv \
    --csv-summary results/optional/condup_50k_fullg1_summary.csv
```

Interpretacao dos resultados:

- Se a verificacao **completar com `correct=true`**, isso reforca a
  confianca no weak check usado na baseline v1 (nao substitui, pois
  cada run sai de um estado inicial levemente diferente devido ao
  shuffle do `--warmup 0`, mas e evidencia topologica direta).
- Se **nao completar em 12h** por size, documente o wall-clock e o
  estado parcial e reporte no TCC como **evidencia empirica de
  intratabilidade** do isomorfismo completo nesse perfil de carga —
  justificando a escolha da abordagem A como metodologia padrao.
- Se a verificacao **completar com `correct=false`**, pare tudo,
  abra issue no repo e trate como regressao critica: o weak check da
  baseline v1 tambem estaria inconsistente.

Nota: `--repetitions 1` e deliberado. A abordagem A roda 10 repeticoes
porque o objetivo la e estatistica de wall-clock (mean, std, CV); a
abordagem B e uma prova topologica unica, nao uma medicao temporal.

### 11.6 Tutorial: Reproduzir `v1_local_baseline` (campanha travada)

Esta subsecao e o passo-a-passo operacional para rodar a campanha
unificada Phase 1 + Phase 2 que gera o snapshot congelado
`results/locked/v1_local_baseline/`. E a referencia que a Phase 3 LAN
vai subtrair para isolar o custo de rede, entao a qualidade dos dados
aqui determina a validade de toda conclusao downstream.

**Quem deve rodar:** o proprio autor (Filipe), em maquina unica, sem
carga de fundo. Qualquer outra pessoa que queira reproduzir em outra
maquina deve usar `scripts/reproduce_local_baseline.sh` — veja
subsecao 11.6.4.

**Tempo total estimado:** 6-9 horas unattended (Phase 1 ~4-6h + Phase 2
~1.5-3h). Planeje rodar durante a noite ou durante um dia em que a
maquina possa ficar dedicada.

#### 11.6.1 Pre-flight checklist

Antes de comecar, confira item por item — cada item e uma condicao
necessaria para a reproducibilidade do snapshot.

**1. Estado do repositorio**

```bash
cd codigo/relativist

# Tag correta e HEAD limpo
git describe --tags --exact-match         # Deve imprimir: v0.10.0-bench
git status --short                        # Deve ser vazio (working copy limpo)
git log -1 --oneline                      # Anote o SHA completo
```

Se `git describe` imprime outra coisa, faca `git checkout v0.10.0-bench`
primeiro. Se `git status` lista arquivos modificados, faca stash ou
commit antes — nao rode a campanha com working copy sujo.

**2. Build release warning-free**

```bash
cargo build --release 2>&1 | tee /tmp/build.log
grep -i warning /tmp/build.log            # Deve ser vazio
ls -lh target/release/relativist.exe      # (Windows) ou target/release/relativist
```

Se aparecer qualquer warning, **pare** e corrija antes de rodar. Um
binario com warnings indica codigo que pode mudar silenciosamente em
atualizacoes futuras e contamina a rastreabilidade do snapshot.

**3. Toolchain e versoes**

```bash
rustc --version                           # Anote para o manifest
cargo --version
docker --version                          # Necessario para Phase 2
docker info | grep -i "memory\|cpus"      # Anote limites do engine
```

**4. Ambiente da maquina (Windows 11)**

| Item | Como configurar | Por que |
|---|---|---|
| Power plan | Control Panel -> Power Options -> **High performance** | Evita throttling por tempo ocioso |
| Windows Update | `Settings -> Windows Update -> Pause updates for 1 week` | Evita reboot inesperado + downloads no meio |
| Antivirus scan | Pausar scan agendado durante a janela da campanha | Scans saturam I/O e distorcem wall-clock |
| Browsers | Fechar Chrome, Firefox, Edge — todos | Cada aba e um processo V8 com GC aleatorio |
| IDE | Fechar VS Code/IntelliJ se aberto em outros repos | `rust-analyzer` em background reindexa |
| Sleep/hibernacao | `Settings -> System -> Power -> Screen and sleep -> Never` durante a janela | Suspensao aborta a campanha |
| Notificacoes | `Focus assist -> Alarms only` | Reduz ruido sistemico |

**5. Diretorio de destino**

```bash
# A campanha escreve em results/locked/v1_local_baseline/
# Confira que apenas README.md e manifest.md (templates) estao la:
ls results/locked/v1_local_baseline/
# Esperado: README.md manifest.md
# Se houver phase1_*.csv ou raw/ de runs anteriores, mova para backup:
#   mv results/locked/v1_local_baseline/phase1_* /tmp/old_snapshot/
```

#### 11.6.2 Executar Phase 1 (4-6 horas unattended)

Phase 1 roda os 12 benchmarks em modo `local` (in-process, sem Docker)
a 10 repeticoes cada, mais o pass strict-BSP para `cascade_cross` e
`dual_tree`. A saida final vai para `results/locked/v1_local_baseline/
phase1_{lenient,strict}_{detail,rounds,summary}.csv` e logs raw em
`raw/phase1/`.

**Comando (rode em um shell que possa ficar aberto):**

```bash
cd codigo/relativist

# Anote o inicio da campanha no manifest
echo "Phase 1 start: $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log

# Rode o driver. Ele e idempotente sobre os raw/*.log individuais,
# mas NAO sobre os CSV agregados finais (que sao reescritos ao fim).
bash scripts/bench_phase1_locked.sh 2>&1 | tee -a /tmp/v1_baseline.log

echo "Phase 1 end:   $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
```

**para copiar e colar**
```
echo "Phase 1 start: $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
bash scripts/bench_phase1_locked.sh 2>&1 | tee -a /tmp/v1_baseline.log
echo "Phase 1 end:   $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
```

O driver imprime uma linha por benchmark no formato
`[HH:MM:SS] LENIENT ep_annihilation (workers=1,2,4,8 reps=10)`. Ao
terminar ele concatena os CSVs individuais e imprime as contagens de
linhas finais.

**Validacao imediata (obrigatoria antes de passar para Phase 2):**

```bash
cd codigo/relativist

# (a) Nenhuma row com correct=false
awk -F, 'NR>1 && $6=="false"' \
    results/locked/v1_local_baseline/phase1_lenient_detail.csv | wc -l
awk -F, 'NR>1 && $6=="false"' \
    results/locked/v1_local_baseline/phase1_strict_detail.csv | wc -l
# Ambos DEVEM imprimir 0. Se nao, pare e investigue.

# (b) Contagem de linhas bate com o esperado
wc -l results/locked/v1_local_baseline/phase1_lenient_detail.csv
wc -l results/locked/v1_local_baseline/phase1_strict_detail.csv
wc -l results/locked/v1_local_baseline/phase1_lenient_summary.csv
wc -l results/locked/v1_local_baseline/phase1_strict_summary.csv

# (c) Nenhum benchmark ficou faltando no summary
awk -F, 'NR>1 {print $1}' \
    results/locked/v1_local_baseline/phase1_lenient_summary.csv \
    | sort -u
# Deve listar: cascade_cross, church_add, church_mul, condup_expansion,
# dual_tree, ep_annihilation, ep_annihilation_con, ep_annihilation_dup,
# erasure_propagation, mixed_net, tree_sum, tree_sum_balanced
```

Se qualquer validacao falhar, **nao prossiga para Phase 2**. Investigue
o `raw/phase1/*.log` do benchmark afetado, corrija e re-rode Phase 1
inteira. E mais barato descartar 4-6 horas agora do que contaminar o
snapshot.

#### 11.6.3 Executar Phase 2 (1.5-3 horas unattended)

Phase 2 roda as 8 combinacoes (benchmark × tamanho) sobre TCP-localhost
via Docker Compose, com 10 repeticoes × 4 contagens de workers + 8
baselines sequenciais nativos. Total: 40 datapoints.

**Pre-flight Docker:**

```bash
# Docker Desktop precisa estar rodando e ocioso
docker info | head                        # Deve imprimir info sem erro
docker ps                                 # Nao deve listar containers relativist-*
docker system prune -f                    # Limpa imagens/layers pendurados

# Cache de imagem (opcional, acelera o primeiro run)
docker compose build worker coordinator
```

**Comando:**

```bash
cd codigo/relativist
echo "Phase 2 start: $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
bash scripts/bench_phase2_locked.sh 2>&1 | tee -a /tmp/v1_baseline.log
echo "Phase 2 end:   $(date '+%Y-%m-%d %H:%M:%S %z')" | tee -a /tmp/v1_baseline.log
```

O driver usa a mesma ortografia de orquestracao que `bench_docker.sh`:
`docker compose up -d` -> `docker wait coordinator` -> parse
`metrics.json` -> proxima repeticao. Voce vera uma linha por
repeticao no log.

**Validacao imediata:**

```bash
cd codigo/relativist

# (a) Zero rows incorretas
awk -F, 'NR>1 && $6=="false"' \
    results/locked/v1_local_baseline/phase2_detail.csv | wc -l
# DEVE imprimir 0

# (b) Contagens
wc -l results/locked/v1_local_baseline/phase2_detail.csv
# Esperado: 329 (8 sequencial + 8*4*10 Docker + 1 header)
wc -l results/locked/v1_local_baseline/phase2_summary.csv
# Esperado: 41 (8 sequencial + 32 Docker + 1 header)

# (c) Todas as 8 combinacoes bench*size aparecem
awk -F, 'NR>1 {print $1 "_" $2}' \
    results/locked/v1_local_baseline/phase2_summary.csv \
    | sort -u | wc -l
# Esperado: 8
```

Se qualquer config travar (timeout de 1800s) ou Docker Desktop ficar
instavel, o driver para imediatamente (set -e). Nesse caso, o raw
`raw/phase2/metrics_<bench>_<size>_w<w>_r<rep>.json` do config
ofensor fica no disco para forensica. Re-rodar apenas o config
faltante exige adaptar o driver manualmente (ou rodar Phase 2 inteira
de novo se faltar tempo pra editar o script).

#### 11.6.4 Pos-campanha: preencher manifest, triagem CV, congelar

**1. Preencher `results/locked/v1_local_baseline/manifest.md`:**

O template tem placeholders `<FILL>` para ~15 campos. Preencha todos:

```bash
cd codigo/relativist

# Provenance
git rev-parse v0.10.0-bench                                  # Commit SHA
rustc --version                                              # Toolchain
# Start/end: extraia de /tmp/v1_baseline.log as linhas "Phase 1 start", etc.

# Hardware (Windows 11)
systeminfo | grep -E "OS Name|OS Version|System Model"
wmic cpu get name                                            # CPU model
wmic cpu get NumberOfCores,NumberOfLogicalProcessors
wmic memorychip get capacity                                 # RAM
docker --version

# Checksums
sha256sum results/locked/v1_local_baseline/phase1_lenient_detail.csv
sha256sum results/locked/v1_local_baseline/phase1_lenient_rounds.csv
sha256sum results/locked/v1_local_baseline/phase1_lenient_summary.csv
sha256sum results/locked/v1_local_baseline/phase1_strict_detail.csv
sha256sum results/locked/v1_local_baseline/phase1_strict_rounds.csv
sha256sum results/locked/v1_local_baseline/phase1_strict_summary.csv
sha256sum results/locked/v1_local_baseline/phase2_detail.csv
sha256sum results/locked/v1_local_baseline/phase2_rounds.csv
sha256sum results/locked/v1_local_baseline/phase2_summary.csv

# Row counts
for f in results/locked/v1_local_baseline/phase{1,2}_*_detail.csv \
         results/locked/v1_local_baseline/phase2_detail.csv \
         results/locked/v1_local_baseline/phase{1,2}_*_summary.csv \
         results/locked/v1_local_baseline/phase2_summary.csv; do
    printf "%-60s %s\n" "$(basename "$f")" "$(wc -l < "$f")"
done
```

Edite `manifest.md` substituindo cada `<FILL>` pelo valor coletado.

**2. Rodar a triagem CV:**

```bash
python3 scripts/cv_triage.py
# Gera results/locked/v1_local_baseline/cv_triage.md com as rows
# CV > 0.15 flagadas com disposition (keep/rerun/exclude).
# Abra o arquivo e revise manualmente; mude dispositions se precisar.
```

**3. Commit atomico do snapshot:**

```bash
cd codigo/relativist

# Todos os arquivos do snapshot em um commit so
git add results/locked/v1_local_baseline/

git commit -m "data: freeze v1_local_baseline snapshot

Phase 1: <N> rows lenient + <M> rows strict, 0 correct=false.
Phase 2: 329 rows detail (40 configs * 10 reps + 8 sequencial), 0 correct=false.
Binary: tag v0.10.0-bench (commit <SHA>).
Hardware: <MODEL>, <CORES>c/<THREADS>t, <RAM>GB, Windows 11.
Campaign window: <START> .. <END>.
sha256 checksums in manifest.md."

git push origin main
```

**4. Bump do submodule no top-level do TCC:**

```bash
cd ..                                     # Sai do submodule
git add codigo/relativist
git commit -m "Sync Relativist submodule: v1_local_baseline snapshot frozen"
git push origin main
```

**5. (Opcional) Reproduzir em outra maquina**

Se tiver acesso a uma segunda maquina (ex: notebook), rode:

```bash
cd codigo/relativist
git fetch origin && git checkout v0.10.0-bench
cargo build --release
bash scripts/reproduce_local_baseline.sh
```

O script roda Phase 1 + Phase 2 em `results/reproduction/<data>/` e
gera `comparison.md` comparando row counts e contagens de
`correct=true` contra a referencia. Wall-clock deve divergir (hardware
diferente), mas colunas estruturais DEVEM casar exatamente.

#### 11.6.5 Troubleshooting

| Sintoma | Diagnostico | Acao |
|---|---|---|
| Phase 1 loga `correct=false` em algum benchmark | Bug de correctness (SPEC-01 G1 falhou) | Pare, abra issue no repo, nao prossiga. Cheque `raw/phase1/<bench>.log`. |
| Phase 2 trava em um config | Timeout de 1800s ou Docker Desktop travou | Verifique `docker ps`. Se tem container zumbi, `docker compose down --remove-orphans` e re-rode Phase 2 inteira. |
| `docker wait` retorna exit code != 0 | Coordinator crashou ou saiu por erro | Veja `raw/phase2/metrics_*.json`. Se `metrics.json` nao existe, foi crash; se existe mas `correct=false`, e bug de redex. |
| CV > 0.30 em varios configs | Maquina tinha carga de fundo | Rode Phase 1 inteira de novo em maquina mais limpa. Marcar individualmente com `keep` no triage **nao** resolve se o padrao e sistemico. |
| wall-clock muito diferente do esperado (2x, 3x) | Power plan voltou para "Balanced" ou CPU throttling por calor | Confira `Control Panel -> Power Options`. Se a maquina esta quente, aguarde esfriar e re-rode. |
| Phase 1 leva > 8h | `condup_expansion` em 50k com G1 full por engano | Confirme que o driver usa `--skip-g1` nos tamanhos 10k/50k. Esta no script por padrao. |
| manifest.md perdido ou mal preenchido | Voce editou e perdeu os valores | O template esta versionado; `git checkout results/locked/v1_local_baseline/manifest.md` recupera. |

### 11.7 Tutorial: Rodar a campanha de stress `v1_stress`

Esta subsecao e o passo-a-passo para rodar a campanha de stress que
estende `v1_local_baseline` para sizes maiores (`ep_annihilation_con` ate
50 M, `dual_tree` ate `d=25`). O objetivo e produzir o dado "antes" dos
itens 2.22-2.26 do ROADMAP (otimizacoes de overhead de rede): a mesma
maquina, o mesmo binario `v0.10.0-bench`, apenas sizes maiores.

**Quem deve rodar:** o proprio autor, apos terminar uma reproducao
limpa de `v1_local_baseline` e antes de implementar qualquer item
2.22-2.26. As medicoes "antes / depois" so sao comparaveis se forem
na **mesma maquina** e sob a **mesma hygiene** de ambiente.

**Por que stress e uma campanha separada de `v1_local_baseline`:**

1. `v1_local_baseline` e um snapshot congelado versionado como referencia
   cientifica da Phase 3 LAN. Nao pode crescer ou mudar depois de
   congelado — se precisasse incluir sizes maiores, seria uma v2.
2. A campanha de stress explora regimes onde a heuristica de hardware
   comeca a quebrar (U-series throttling, 1 GiB frame cap, Docker
   WSL2 memory pressure). Alguns configs **podem falhar** — isso e
   esperado e documentado, nao e um bug. Congelar falhas dentro da
   baseline principal contaminaria os dados de Phase 3.
3. A campanha de stress usa 5 repeticoes (nao 10), porque o custo
   por repeticao e maior; usar 10 empurraria o wall-clock da campanha
   para 8-12 h, o que nao e economico dado o proposito "dado antes de
   otimizacao".

**Layout da campanha:**

| Fase | Benchmarks × sizes | Workers | Reps | Observacao |
|---|---|---|---|---|
| Phase 1 stress (in-process) | `ep_annihilation_con × {10M,20M,50M}`, `dual_tree × {23,24,25}` | seq + local {1,2,4,8} | 5 | Sem Docker. |
| Phase 2 stress (Docker) | `ep_annihilation_con × {10M,20M}`, `dual_tree × {23,24,25}` | {1,2,4,8} | 5 | Completo. |
| Phase 2 stress (Docker) | `ep_annihilation_con × {50M}` | {4,8} apenas | 5 | w=1 e w=2 sao puladas: a particao excede o 1 GiB frame cap sob bincode v1 + CompactSubnet. Documentado como limitacao no ROADMAP 2.23. |

**Tempo total estimado:** 4-6 h unattended (Phase 1 ~1-2h + Phase 2 ~3-4h).
Planeje rodar durante a noite ou durante um dia dedicado.

**Diferencas de implementacao vs. `bench_phase2_locked.sh`:**

O script `bench_phase2_stress_locked.sh` corrige um bug de shutdown do
Docker Compose observado no smoke test de 20 M em 2026-04-11:

- `bench_phase2_locked.sh` usa `docker compose up
  --abort-on-container-exit --exit-code-from coordinator`. Em sizes de
  stress o coordinator demora mais para flushar `metrics.json` do que
  os workers demoram para sair, entao o `--abort-on-container-exit`
  SIGKILLa o coordinator no meio do flush e `metrics.json` nunca chega
  ao disco.
- `bench_phase2_stress_locked.sh` usa `docker compose up -d` +
  `docker wait <coordinator_id>` + `docker compose down`, deixando o
  coordinator sair naturalmente e flushar `metrics.json` antes do
  teardown.

#### 11.7.1 Pre-flight checklist

Antes de comecar, confira cada item. Os itens sao identicos a
`v1_local_baseline` (11.6.1) com duas excecoes: Phase 1 stress **nao**
precisa de Docker (mas Phase 2 stress precisa), e a campanha roda em
`results/extended/v1_stress/` (nao em `results/locked/`).

**1. Estado do repositorio**

```bash
cd codigo/relativist

git describe --tags --exact-match         # Deve imprimir: v0.10.0-bench
git status --short                        # Deve ser vazio
git log -1 --oneline                      # Anote o SHA
```

Se HEAD nao estiver em `v0.10.0-bench`, faca `git checkout v0.10.0-bench`.
A campanha de stress **tem** que rodar contra a mesma tag que
`v1_local_baseline`, senao as comparacoes "stress vs baseline" misturam
binarios diferentes.

**2. Environment hygiene (obrigatorio para dados congelados)**

```bash
# Power plan deve ser Ultimate Performance (mesmo que v1_local_baseline).
powercfg /getactivescheme
# Output esperado contem: (Desempenho Maximo) ou (Ultimate Performance)

# Se estiver em Balanced, ative o Ultimate Performance:
powercfg -duplicatescheme e9a42b02-d5df-448d-aa00-03f14749eb61
powercfg /setactive <GUID_novo_da_linha_anterior>
powercfg /getactivescheme   # confira que trocou
```

Fechar antes de kickar a campanha:
- IDE (VS Code, JetBrains, Zed, etc.)
- Browsers (Chrome, Firefox, Edge)
- Qualquer aplicacao com tray icon que sincroniza com a nuvem (Dropbox,
  OneDrive, Google Drive)

Pausar Windows Update ate o fim da campanha.

**3. Build release**

```bash
cargo build --release
ls -la target/release/relativist.exe    # deve existir
```

**4. Docker Desktop (so para Phase 2 stress)**

```bash
docker compose ps               # deve imprimir header + linhas vazias
docker compose build            # pre-build pra nao contar no wall-clock
```

Se voce nao vai rodar Phase 2 stress, pode pular esta etapa.

**5. Espaco em disco**

```bash
df -h .     # precisa de ~3-5 GB livres para raw/phase2/metrics_*.json
```

#### 11.7.2 Executar Phase 1 stress (1-2 horas unattended)

```bash
cd codigo/relativist
./scripts/bench_phase1_stress_locked.sh
```

O script vai:

1. Detectar o binario em `target/release/relativist.exe`.
2. Criar `results/extended/v1_stress/` e `raw/phase1/`.
3. Rodar `ep_annihilation_con` nos sizes `10M,20M,50M` com workers `1,2,4,8`
   (mais sequential auto-adicionado pela suite).
4. Rodar `dual_tree` nos sizes `23,24,25` com workers `1,2,4,8`.
5. Concatenar os CSVs raw em `phase1_stress_detail.csv`,
   `phase1_stress_rounds.csv` e `phase1_stress_summary.csv`.

Saida esperada ao final:

```
[HH:MM:SS] === Phase 1 Stress Campaign complete ===
[HH:MM:SS] Detail:  N rows -> .../phase1_stress_detail.csv
[HH:MM:SS] Rounds:  M rows -> .../phase1_stress_rounds.csv
[HH:MM:SS] Summary: K rows -> .../phase1_stress_summary.csv
```

Valide rapido:

```bash
# Nenhum correct=false
awk -F, 'NR>1 && $6=="false"' results/extended/v1_stress/phase1_stress_detail.csv
# (output vazio)

# Contagem esperada do summary: 2 benches * 3 sizes * 5 modos (seq + 1,2,4,8) = 30 linhas + header
wc -l results/extended/v1_stress/phase1_stress_summary.csv
```

#### 11.7.3 Executar Phase 2 stress (3-4 horas unattended)

Com Docker Desktop rodando:

```bash
cd codigo/relativist
./scripts/bench_phase2_stress_locked.sh
```

O script vai:

1. Rodar `docker compose build` (a menos que voce use `--skip-build`).
2. Gerar baselines sequenciais nativos (fora do Docker) para cada
   `bench × size` distinto — usados para calcular speedup vs
   sequential nas linhas do Docker.
3. Para cada `bench × size × workers`:
   - Copiar o input para `data/input.bin`.
   - Fazer `docker compose up -d --scale worker=W`.
   - Chamar `docker wait coordinator` (ate o container sair
     naturalmente e flushar `metrics.json`).
   - Ler `data/metrics.json`, validar G1 com `inspect`.
   - `docker compose down --remove-orphans`.
4. Escrever `phase2_stress_detail.csv`, `phase2_stress_rounds.csv` e
   `phase2_stress_summary.csv`.

Saida esperada:

```
[HH:MM:SS] ==========================================
[HH:MM:SS]   Phase 2 Stress Campaign Complete
[HH:MM:SS] ==========================================
[HH:MM:SS] Start: 2026-04-11 HH:MM:SS -0300
[HH:MM:SS] End:   2026-04-11 HH:MM:SS -0300
[HH:MM:SS] Output files:
[HH:MM:SS]   .../phase2_stress_detail.csv  (N rows)
[HH:MM:SS]   .../phase2_stress_summary.csv (M rows)
[HH:MM:SS]   .../phase2_stress_rounds.csv  (K rows)
```

**Configs esperados:** 6 `bench × size` com 4 workers + 1 `bench × size`
(ep_con=50M) com 2 workers = 24 + 2 = **26 configs Docker**. Mais 7
baselines sequenciais. Total: **33 linhas de summary + header = 34**.

Validacao:

```bash
# Se houver correct=false, investigue antes de prosseguir
awk -F, 'NR>1 && $6=="false"' results/extended/v1_stress/phase2_stress_detail.csv

# Configs que pularam (exit_code != 0) apareceram como all_correct=false
# no summary. Espera-se zero falhas sob o binario v0.10.0-bench, mas se
# alguma aparecer, cheque o log raw de Docker Compose do ultimo run.

# Row count esperado do detail: 26 configs * 5 reps + 7 seq * 5 reps + header = 166
wc -l results/extended/v1_stress/phase2_stress_detail.csv
```

#### 11.7.4 Pos-campanha: preencher manifest

Copie o template do `v1_local_baseline` e atualize:

```bash
cp results/locked/v1_local_baseline/manifest.md \
   results/extended/v1_stress/manifest.md
```

Edite `results/extended/v1_stress/manifest.md` para refletir a
campanha de stress:

- Status: COMPLETE ou documentar quais configs falharam
- Campaign knobs: 5 reps (nao 10), sizes diferentes
- Adicione uma secao "Differences from v1_local_baseline" explicando
  (a) sizes maiores, (b) 5 reps, (c) o shutdown fix do Docker, (d) que
  esta e a medicao "antes" dos itens ROADMAP 2.22-2.26
- Checksums: gere sha256 dos CSVs novos

```bash
cd results/extended/v1_stress
sha256sum phase1_stress_*.csv phase2_stress_*.csv > checksums.sha256
cat checksums.sha256
```

#### 11.7.5 Troubleshooting

| Sintoma | Diagnostico | Acao |
|---|---|---|
| `ep_annihilation_con=50M w=4` ou `w=8` falha no Docker com `metrics.json` ausente | Coordinator SIGKILL por OOM (WSL2 VM com 15 GiB, 50 M agentes sob bincode v1 usa ~3-4 GB so de particao serializada) | Documente no manifest como limitacao conhecida. Mostre que com 2.23 (wire compaction) o footprint cai abaixo de 1 GB. |
| `dual_tree=25 w=1` falha por frame cap | Particao > 1 GiB sob bincode v1 | Esperado — w=1 concentra tudo em uma particao. Documente. |
| `docker wait` trava | Coordinator nao esta saindo — provavel deadlock do protocolo | Cheque `docker compose logs coordinator` na outra shell; se o coordinator esta preso em `reduce_all` ou `collect`, aguarde ou interrompa com Ctrl+C. |
| Wall-clock de Phase 1 stress explode para >4 h | CPU throttling por calor | Interrompa, deixe a maquina esfriar 30 min, re-rode do zero. |
| Phase 1 stress `correct=false` | Bug novo de correctness no v0.10.0-bench em sizes grandes | **Pare e investigue**. Se reproducivel, e um L-item critico: a baseline local nao esta mais valida nessa faixa de sizes. |

---

### 11.8 Demonstracao aritmetica: `church_sum_of_squares`

Esta subsecao cobre o **unico** benchmark do Relativist cujo proposito e
explicitamente **demonstrativo**, nao comparativo: a grid resolve o
problema classico de somar quadrados inteiros
`sum_{i=1..N} i^2 = N*(N+1)*(2N+1)/6` (formula de Arquimedes/Faulhaber),
e o resultado decodificado e conferido contra a formula fechada. O
objetivo e mostrar que a plataforma IC distribuida executa um calculo
aritmetico reconhecivel de ponta a ponta, nao medir desempenho.

**Por que este benchmark NAO entra em `v1_local_baseline` nem em
`v1_stress`:**

- O problema e ilustrativo para o texto/defesa do TCC, nao dominado por
  uma regra de interacao especifica que as campanhas de performance
  isolam.
- Tempos de wall-clock dele nao sao comparaveis contra os benchmarks
  estruturais (`ep_annihilation`, `dual_tree`, `cascade_cross`, etc.) —
  a quantidade de agentes finais cresce cubicamente em `N`, nao
  linearmente, porque o resultado e Church(sum) e o sum cresce como
  `N^3`.
- Decode nao-canonico por reducao otima de `mul` composto com `add`
  produziria nets com multiplas fronteiras DUP aninhadas que o decoder
  atual (`decode_shared_chain`) nao sabe caminhar. Para manter a
  verificacao tratavel, os quadrados `i^2` sao **pre-encodados** em Rust
  como numerais de Church antes de serem injetados no net; a grid ainda
  reduz a cadeia inteira de `add` (trabalho substancial, Profile B,
  expansao dominante), mas a fase de "quadratura" e local. Honestamente
  documentado como limitacao do decoder, nao como decisao de design.

**Problema matematico:**

```
sum_{i=1..N} i^2  =  N * (N + 1) * (2N + 1) / 6
```

Valores esperados para as `default_sizes`:

| `N` | `sum i^2`  |
|-----|------------|
| 5   | 55         |
| 10  | 385        |
| 30  | 9.455      |
| 50  | 42.925     |
| 100 | 338.350    |

**Dimensionamento do net:**

| `N` | Agentes iniciais (~) | Agentes apos reducao (~) | Uso tipico                  |
|-----|----------------------|--------------------------|-----------------------------|
| 5   | ~130                 | ~111                     | Unit test de smoke          |
| 10  | ~800                 | ~771                     | Demo rapida sequencial      |
| 30  | ~20_000              | ~18_900                  | Demo grid local pequena     |
| 50  | ~88_000              | ~85_800                  | Demo grid local media       |
| 100 | ~690_000             | ~676_700                 | Demo "big number", grid 8 w |

A cadeia `Church(1) + Church(4) + Church(9) + ... + Church(N^2)`
pre-encoda `N` numerais de Church (totais iniciais agregam
`sum_{i=1..N} (2*i^2 + 1)`) e aplica a reducao da cadeia direita de
`add`. O resultado final e `Church(sum_{i=1..N} i^2)` com agent count
~`2 * N*(N+1)*(2N+1)/6 + 1`.

**Smoke sequential:**

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
church_sum_of_squares     10       0   0.00000X     X.X   1.0000     1.0000

Total datapoints: 1  |  All correct: true
```

`All correct: true` confirma `decode_nat_or_shared(net) == Some(385)`
(385 e `1^2 + 2^2 + ... + 10^2`).

**Smoke grid local (in-process):**

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

Para cada tamanho, a verificacao do benchmark roda
`decode_nat_or_shared(seq) == decode_nat_or_shared(dist)` e cai para
`nets_isomorphic(seq, dist)` se o decode falhar — a saida final do CLI
imprime `All correct: true` quando a grid produziu o numero esperado.

**Smoke grid Docker (`tcp_localhost`), opcional:**

```bash
./target/release/relativist bench \
  --benchmark church_sum_of_squares \
  --mode tcp_localhost \
  --sizes 30,50 \
  --repetitions 1 --warmup 0 --workers 4
```

`N=100` nao e recomendado no `tcp_localhost` no primeiro pass porque o
net final (`~680 k` agentes) aproxima o frame cap de 1 GiB sob
`bincode` v1 + `CompactSubnet` (ver ROADMAP 2.23). Se precisar,
documente a falha como limitacao conhecida e rode localmente no modo
`local` (in-process).

**Formato de saida (descrito pelo benchmark):**

```
Sum of squares 1..N^2 = <valor esperado>
```

Onde `<valor esperado> = N*(N+1)*(2N+1)/6`. Por exemplo:

```
Sum of squares 1..10^2 = 385
Sum of squares 1..30^2 = 9455
Sum of squares 1..100^2 = 338350
```

**Nota de honestidade academica:**

Este benchmark e o unico do suite com proposito explicitamente
demonstrativo. Ele **nao** e incluido nas campanhas congeladas
`v1_local_baseline` e `v1_stress`, e **nao** deve ser usado para
comparacoes de desempenho contra os benchmarks estruturais. Ele existe
para (i) produzir uma figura/demo que o leitor do TCC reconhece como
"a grid distribuida computou um numero de verdade" e (ii) validar
end-to-end que a pilha de encoding aritmetico + grid + decode funciona
sob os tres modos (sequential, local, tcp_*).

---

## 12. Pipeline Completa

Exemplo de pipeline completa: gerar, inspecionar, reduzir de duas formas, comparar.

```bash
# 1. Gerar rede mixed-rules com 20 pares de cada tipo
relativist generate mixed-rules -n 20 -o mixed20.bin

# 2. Inspecionar a rede original
relativist inspect -i mixed20.bin
# Agents: 240, Redexes: 120

# 3. Reduzir sequencialmente
relativist reduce -i mixed20.bin -o mixed20_seq.bin

# 4. Inspecionar resultado sequencial
relativist inspect -i mixed20_seq.bin
# Agents: 80 (todas ERA), Redexes: 0, Normal Form: yes

# 5. Reduzir via grid com 4 workers
relativist local -w 4 -i mixed20.bin -o mixed20_grid.bin -m mixed20_metrics.json

# 6. Inspecionar resultado do grid
relativist inspect -i mixed20_grid.bin
# Deve ter o mesmo resultado: 80 ERA, 0 redexes
```

### Pipeline de Benchmark para o TCC

```bash
# Campanha de benchmark completa para o artigo
mkdir -p results

# Profile A: EP-Annihilation (deveria ser embaracosamente paralelo)
relativist bench \
  --benchmark ep_annihilation,ep_annihilation_con,ep_annihilation_dup \
  --sizes 100,500,1000,5000,10000 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/profile_a_detail.csv \
  --csv-rounds results/profile_a_rounds.csv \
  --csv-summary results/profile_a_summary.csv

# Profile B: Expansion + Collapse
relativist bench \
  --benchmark condup_expansion,mixed_net,church_add,church_mul \
  --sizes 10,50,100,500 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/profile_b_detail.csv \
  --csv-rounds results/profile_b_rounds.csv \
  --csv-summary results/profile_b_summary.csv

# Profile C: Sequential Dependency
relativist bench \
  --benchmark dual_tree,erasure_propagation \
  --sizes 4,6,8,10 \
  --workers 1,2,4,8 \
  --warmup 2 \
  --repetitions 10 \
  --csv-detail results/profile_c_detail.csv \
  --csv-rounds results/profile_c_rounds.csv \
  --csv-summary results/profile_c_summary.csv
```

---

## 13. Formatos de Arquivo

### .bin (Binario — bincode)

Formato binario compacto para redes grandes. Mais rapido para ler/escrever.

```bash
relativist generate ep-annihilation -n 10000 -o net.bin
```

### .ic (Texto — legivel)

Formato texto legivel por humanos. Util para inspecao e debug de redes pequenas.

```bash
relativist generate ep-annihilation -n 3 -o net.ic
cat net.ic
```

Estrutura:
```
agent a<ID> <SYMBOL>
wire a<ID>.<PORT> a<ID>.<PORT>
wire a<ID>.<PORT> free<N>
```

Portas: `principal`, `left` (aux1), `right` (aux2).

---

## 14. update — Atualizar Relativist

Verifica se ha uma versao mais recente e atualiza automaticamente.

### Verificar se ha atualizacao

```bash
relativist update --check
```

Saida:
```
Current version: 0.9.0
Latest version:  0.9.1

Update available: 0.9.0 -> 0.9.1
```

### Atualizar automaticamente

```bash
relativist update
```

O comando baixa o binario correto para seu OS, verifica o checksum SHA256, e substitui o executavel atual.

**Requisitos:** `gh` (GitHub CLI) autenticado para repositorios privados, ou `curl` para repositorios publicos.

---

## 15. completions — Autocompletar no Shell

Gera scripts de autocompletar para seu shell. Os subcomandos e flags sao preenchidos com Tab.

```bash
# Bash
relativist completions bash > ~/.bash_completion.d/relativist
source ~/.bash_completion.d/relativist

# Zsh
relativist completions zsh > ~/.zfunc/_relativist

# Fish
relativist completions fish > ~/.config/fish/completions/relativist.fish

# PowerShell
relativist completions powershell >> $PROFILE
```

---

## 16. Desenvolvimento: Verificacoes Pre-Push

Antes de fazer commit/push ou criar tags de release, **sempre** execute estas verificacoes localmente.
Sao as mesmas que o CI (GitHub Actions) executa — se passarem localmente, a pipeline passa.

### Checklist rapido (copie e cole)

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo build --release
```

Se tudo passar sem erros, seu codigo esta pronto para push.

### Passo a passo

**1. Formatacao (rustfmt)**

```bash
cargo fmt --check
```

Se houver diferencas, corrija com `cargo fmt` e faca o commit.

**2. Linter (clippy)**

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Flags importantes:
- `--all-targets`: checa lib, testes, benchmarks e binarios
- `--all-features`: habilita features opcionais (tls, metrics, otel)
- `-D warnings`: trata warnings como erros (mesmo comportamento do CI)

**3. Testes**

```bash
cargo test
```

Todos os 639+ testes devem passar, com 0 warnings.

**4. Build release**

```bash
cargo build --release
```

Garante que o binario final compila sem erros.

### Antes de criar uma tag de release

```bash
# 1. Verificacoes completas
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo build --release

# 2. Atualizar versao no Cargo.toml (se necessario)
# version = "0.9.0"

# 3. Commit e tag
git add -A && git commit -m "release: vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags
```

A tag `v*` dispara automaticamente:
- **CI** (`ci.yml`): fmt + clippy + test + build
- **Release** (`release.yml`): compila binarios Linux/Windows, cria GitHub Release com checksums
- **Docker** (`docker.yml`): build e push da imagem para GHCR

---

## 17. Referencia Rapida

```
relativist --version              # Versao
relativist --help                 # Ajuda geral

# Gerar redes
relativist generate <TIPO> -n <N> -o <ARQUIVO>

# Inspecionar
relativist inspect -i <ARQUIVO>

# Reducao sequencial
relativist reduce -i <ENTRADA> [-o <SAIDA>]

# Simulacao de grid
relativist local -w <WORKERS> -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>]

# Aritmetica Church
relativist compute add <A> <B> [--workers <N>]
relativist compute mul <A> <B> [--workers <N>]
relativist compute exp <A> <B> [--workers <N>]

# Benchmarks
relativist bench [--benchmark <B>] [--sizes <S>] [--workers <W>]
                 [--warmup <N>] [--repetitions <N>]
                 [--csv-detail <F>] [--csv-rounds <F>] [--csv-summary <F>]

# Atualizar
relativist update              # Baixar e instalar ultima versao
relativist update --check      # Apenas verificar se ha atualizacao

# Shell completions
relativist completions bash|zsh|fish|powershell

# Docker
docker build -t relativist .
docker run --rm relativist <COMANDO>
docker run --rm -v /dados:/data relativist <COMANDO> -o /data/...
```
