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
conectadas por rede Ethernet/Wi-Fi. Expoe o custo real de um grid
distribuido: latencia de RTT, throughput limitado, jitter, contencao
de NIC.

**Pre-requisitos:**
- 2+ maquinas na mesma LAN (ou em VPN com rotas TCP abertas)
- Mesma versao do `relativist` instalada em todas (`relativist --version`)
- Porta TCP do coordinator liberada no firewall (ex: `9000/tcp`)
- Input net identico em todas as maquinas (transferir com `scp`/rsync)
- Sincronizacao de relogio razoavel (NTP) para logs coerentes

**Setup manual (exemplo: 1 coordinator + 2 workers em 3 maquinas):**

*Maquina A (coordinator) — IP 192.168.1.10:*

```bash
# Liberar porta (Linux ufw)
sudo ufw allow 9000/tcp

# Gerar input
relativist generate ep-annihilation-con -n 1000000 -o input.bin

# Iniciar coordinator gerando token automatico
relativist coordinator \
  --workers 2 \
  --bind 0.0.0.0:9000 \
  --token auto --token-file /tmp/rel-token \
  -i input.bin \
  -o output.bin \
  -m metrics.json \
  --log-format json
```

O coordinator imprime o token base64 no stdout e o grava em
`/tmp/rel-token`. Copie esse valor para as maquinas dos workers
(via `scp` seguro, nao por canal em claro).

*Maquina B (worker 1) — IP 192.168.1.11:*

```bash
# Receber o token da maquina A (ou copiar manualmente)
scp user@192.168.1.10:/tmp/rel-token /tmp/rel-token
TOKEN=$(cat /tmp/rel-token)

# Conectar ao coordinator
relativist worker \
  --coordinator 192.168.1.10:9000 \
  --token "$TOKEN" \
  --log-format json
```

*Maquina C (worker 2) — IP 192.168.1.12:*

```bash
scp user@192.168.1.10:/tmp/rel-token /tmp/rel-token
TOKEN=$(cat /tmp/rel-token)

relativist worker \
  --coordinator 192.168.1.10:9000 \
  --token "$TOKEN" \
  --log-format json
```

Quando todos os workers conectam, o coordinator executa o grid BSP
sobre TCP real, grava `output.bin` e `metrics.json` (no host A), e
todos os processos terminam.

**Verificacao G1 pos-execucao:**

```bash
# Na maquina A, reduzir sequencialmente o mesmo input
relativist reduce -i input.bin -o output_seq.bin

# Comparar (estrutural rapido)
relativist inspect -i output.bin > /tmp/dist.txt
relativist inspect -i output_seq.bin > /tmp/seq.txt
diff /tmp/dist.txt /tmp/seq.txt
```

**Orquestracao de uma campanha em Phase 3:**

Uma campanha exige rodar muitas combinacoes (benchmark × tamanho ×
workers × repeticoes) ao longo de varias maquinas. Duas estrategias:

1. **SSH + script unico:** um script na maquina A usa `ssh worker1` e
   `ssh worker2` para lancar os workers remotamente, espera o
   coordinator finalizar, coleta `metrics.json` e repete. E o mesmo
   padrao do `bench_docker.sh`, trocando `docker compose up` por
   `ssh ... relativist worker ...`.
2. **Orquestrador dedicado:** Ansible, Nomad, ou Kubernetes agendando
   os processos. Mais robusto para LANs grandes; excessivo para 2–3
   nos.

Para o TCC atual, o objetivo minimo e produzir comparacoes
pontuais entre Phase 1, Phase 2 e Phase 3 no mesmo conjunto de
benchmarks (ex: `ep_annihilation_con` tamanho 500k com 2, 4 e 8
workers), ja suficientes para decompor o overhead em:
`overhead_total = overhead_algoritmico + overhead_tcp + overhead_rede`.

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

Duas limitacoes praticas apareceram durante a campanha de Phase 2
(Docker) e ambas foram mitigadas apos v0.9.0. O historico completo
fica em `docs/PHASE2-FINDINGS.md`; esta secao resume o estado atual.

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
