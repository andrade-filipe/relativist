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
9. [Docker](#9-docker)
10. [Pipeline Completa: Gerar, Inspecionar, Reduzir, Comparar](#10-pipeline-completa)
11. [Formatos de Arquivo](#11-formatos-de-arquivo)
12. [Desenvolvimento: Verificacoes Pre-Push](#12-desenvolvimento-verificacoes-pre-push)
13. [Referencia Rapida](#13-referencia-rapida)

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
VERSION=0.6.0 curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

### Opcao 2: Docker

```bash
docker pull ghcr.io/andrade-filipe/relativist
docker run --rm ghcr.io/andrade-filipe/relativist --version
```

### Opcao 3: Download manual

Baixe o binario para seu sistema em:
https://github.com/andrade-filipe/relativist/releases

- **Windows (recomendado):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.exe` (download direto, sem extrair)
- **Windows (alternativa):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.zip` (extrair o .exe)
- **Linux:** `relativist-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz`

Coloque o binario numa pasta do seu PATH.

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
# relativist 0.6.0

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

## 9. Docker

### Construir a imagem

```bash
cd codigo/relativist
docker build -t relativist .
```

### Verificar

```bash
docker run --rm relativist --version
# relativist 0.0.1
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

## 10. Pipeline Completa

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

## 11. Formatos de Arquivo

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

## 12. Desenvolvimento: Verificacoes Pre-Push

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
# version = "0.7.0"

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

## 13. Referencia Rapida

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

# Docker
docker build -t relativist .
docker run --rm relativist <COMANDO>
docker run --rm -v /dados:/data relativist <COMANDO> -o /data/...
```
