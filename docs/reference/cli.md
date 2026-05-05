# Referencia CLI

Tabela autoritativa de todos os subcomandos e flags do binario `relativist`. Para tutoriais end-to-end, veja [../guides/](../guides/README.md).

```
relativist --version        # Versao
relativist --help           # Ajuda geral
relativist <CMD> --help     # Ajuda por subcomando
```

## Subcomandos

| Subcomando     | Descricao                                                      | Guia didatico                               |
|----------------|----------------------------------------------------------------|---------------------------------------------|
| `generate`     | Cria uma rede IC para benchmarks e testes                       | [02](../guides/02-first-reduction.md)       |
| `inspect`      | Inspeciona uma rede (agentes, redexes, normal form)            | [02](../guides/02-first-reduction.md)       |
| `reduce`       | Reduz uma rede sequencialmente                                 | [02](../guides/02-first-reduction.md)       |
| `local`        | Simula uma grid in-process (N workers, sem TCP)                | [03](../guides/03-local-grid.md)            |
| `compute`      | Aritmetica de Church end-to-end (add/mul/exp)                  | [04](../guides/04-church-arithmetic.md)     |
| `coordinator`  | Inicia um coordinator TCP                                      | [05](../guides/05-distributed-tcp.md)       |
| `worker`       | Inicia um worker TCP conectado a um coordinator                | [05](../guides/05-distributed-tcp.md)       |
| `bench`        | Executa a suite de benchmarks                                  | [benchmarks/](../benchmarks/README.md)      |
| `update`       | Baixa e instala a ultima versao do binario                     | §4                                          |
| `completions`  | Gera scripts de autocompletar para shells                      | §5                                          |

---

## `generate`

Gera uma rede em `.bin` (bincode) ou `.ic` (texto).

```bash
relativist generate <TIPO> -n <N> -o <ARQUIVO>
```

| Tipo                      | Descricao                                                           |
|---------------------------|---------------------------------------------------------------------|
| `ep-annihilation`         | `N` pares ERA-ERA                                                   |
| `ep-annihilation-con`     | `N` pares CON-CON                                                   |
| `ep-annihilation-dup`     | `N` pares DUP-DUP                                                   |
| `mixed-rules`             | Mix balanceado de CON-CON, DUP-DUP, CON-ERA, DUP-ERA                |
| `condup-expansion`        | `N` expansores CON-DUP (Profile B, comutacao pesada)                |
| `cascade-cross`           | Cadeia de `N` redexes em sequencia para cruzar particoes (strict)   |
| `dual-tree`               | Arvore binaria de profundidade `d`                                  |
| `erasure-propagation`     | Propagacao de ERA ate profundidade `d`                              |
| `tree-sum`                | Arvore de somas Church                                              |
| `tree-sum-balanced`       | Arvore de somas Church balanceada                                   |

Flags comuns:

| Flag         | Default   | Descricao                            |
|--------------|-----------|--------------------------------------|
| `-n <N>`     | varia     | Tamanho do benchmark                 |
| `-o <PATH>`  | stdout    | Saida (`.bin` ou `.ic`)              |
| `--format`   | auto      | `binary` ou `text` (override extensao)|

## `inspect`

Imprime contagem de agentes, redexes e se a rede esta em forma normal.

```bash
relativist inspect -i <ARQUIVO>
```

## `reduce`

Reduz sequencialmente ate forma normal.

```bash
relativist reduce -i <ENTRADA> [-o <SAIDA>]
```

| Flag          | Default | Descricao                                |
|---------------|---------|------------------------------------------|
| `-i <PATH>`   | —       | Rede de entrada                           |
| `-o <PATH>`   | none    | Escreve o resultado                       |
| `--max-steps` | ilimitado | Limite de passos (seguranca)            |

## `local`

Simula uma grid in-process com `N` workers (sem TCP, sem Docker).

```bash
relativist local -w <N> -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>]
```

| Flag           | Default | Descricao                                            |
|----------------|---------|------------------------------------------------------|
| `-w, --workers`| 4       | Numero de workers                                    |
| `-i <PATH>`    | —       | Rede de entrada                                      |
| `-o <PATH>`    | none    | Escreve o resultado                                  |
| `-m <PATH>`    | none    | Escreve metrics JSON (rounds, tempo por fase)        |
| `--strict-bsp` | false   | Modo strict (fila processada exatamente uma vez)     |
| `--delta-mode` | false   | Ativa SPEC-19 delta protocol ([guia 06](../guides/06-delta-protocol.md)) |

## `compute`

Aritmetica de Church end-to-end: encode -> reduce -> decode.

```bash
relativist compute add <A> <B> [--workers <N>]
relativist compute mul <A> <B> [--workers <N>]
relativist compute exp <A> <B> [--workers <N>]
```

| Flag           | Default | Descricao                                          |
|----------------|---------|----------------------------------------------------|
| `--workers <N>`| 1       | `>=2` ativa grid local; `1` = sequencial           |
| `--mode`       | local   | `sequential`, `local`, ou `tcp_localhost`          |

`compute exp` nao decodifica por conta de DUP ciclico (ver [L5](../benchmarks/limitations.md#l5)).

## `coordinator`

Inicia um coordinator TCP.

```bash
relativist coordinator --workers <N> --port <PORT> -i <ENTRADA> [-o <SAIDA>] [-m <METRICAS>] [flags]
```

| Flag                | Default  | Descricao                                                |
|---------------------|----------|----------------------------------------------------------|
| `-w, --workers <N>` | —        | Quantos workers esperar antes de comecar                  |
| `-p, --port <P>`    | —        | Porta TCP para listen                                    |
| `-i <PATH>`         | —        | Rede de entrada                                          |
| `-o <PATH>`         | none     | Escreve resultado                                        |
| `-m <PATH>`         | none     | Escreve metrics JSON                                     |
| `--bind <ADDR>`     | 0.0.0.0  | Interface de bind                                        |
| `--auth-token <T>`  | none     | Token compartilhado para auth (ver SPEC-10)              |
| `--tls`             | off      | Ativa TLS (requer `--cert`, `--key`)                     |
| `--cert <PATH>`     | —        | Cert PEM (com `--tls`)                                   |
| `--key <PATH>`      | —        | Key PEM (com `--tls`)                                    |
| `--strict-bsp`      | false    | Modo strict                                              |
| `--delta-mode`      | false    | Ativa SPEC-19 delta protocol                             |
| `--shutdown-grace`  | 10s      | Tempo para flushar metrics antes de sair                 |

## `worker`

Inicia um worker TCP conectando a um coordinator.

```bash
relativist worker --coordinator <HOST:PORT> [flags]
```

| Flag                      | Default | Descricao                                             |
|---------------------------|---------|-------------------------------------------------------|
| `--coordinator <ADDR>`    | —       | Endereco do coordinator                               |
| `--auth-token <T>`        | none    | Deve casar com o do coordinator                       |
| `--tls`                   | off     | Ativa TLS                                             |
| `--ca <PATH>`             | —       | CA cert (com `--tls`)                                 |
| `--reconnect-attempts <N>`| 3       | Retries de conexao inicial                            |

## `bench`

Executa a suite de benchmarks. Ver [benchmarks/README.md](../benchmarks/README.md) para detalhes.

```bash
relativist bench [flags]
```

| Flag                         | Default              | Descricao                                         |
|------------------------------|----------------------|---------------------------------------------------|
| `--benchmark <LISTA>`        | todos                | Lista separada por virgula                        |
| `--sizes <LISTA>`            | default do bench     | Sizes separados por virgula                       |
| `--workers <LISTA>`          | 1,2,4,8              | Worker counts; 0 = sequential                     |
| `--warmup <N>`               | 2                    | Rodadas iniciais descartadas                      |
| `--repetitions <N>`          | 10                   | Repeticoes por config                             |
| `--mode <MODO>`              | local                | `sequential`, `local`, `tcp_localhost`            |
| `--strict-bsp`               | false                | Strict BSP                                        |
| `--csv-detail <PATH>`        | none                 | CSV por-repeticao                                 |
| `--csv-rounds <PATH>`        | none                 | CSV por-rodada BSP                                |
| `--csv-summary <PATH>`       | none                 | CSV agregado                                      |
| `--skip-g1`                  | false                | Pula isomorfismo O(N!); mantem weak check (L5)    |

## `update`

Verifica e instala a ultima versao.

```bash
relativist update --check      # Apenas verifica
relativist update              # Baixa e substitui o executavel
```

Requer `gh` (repositorios privados) ou `curl` (publicos). Verifica checksum SHA256 antes de substituir.

## `completions`

Gera scripts de autocompletar:

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

## Flags v2 (multi-subcomando)

As flags abaixo foram introduzidas em v2 e aplicam-se a `coordinator`, `local` e/ou `bench`. Os defaults preservam comportamento v1 quando aplicavel; ative apenas o que voce realmente quer medir/usar.

### Streaming + arena (SPEC-21 / SPEC-22)

| Flag                          | Aplica a                       | Default                  | Descricao                                                                                  | Guia                                       |
|-------------------------------|--------------------------------|--------------------------|--------------------------------------------------------------------------------------------|--------------------------------------------|
| `--chunk-size <N>`            | `coordinator`, `local`, `bench`| 10000 (None em `bench`)  | Tamanho do `AgentBatch` no streaming. `4294967295` (`u32::MAX`) desliga streaming.         | [09](../guides/09-streaming-generation.md) |
| `--max-pending-lifetime <N>`  | `coordinator`, `local`, `bench`| 16                       | Numero maximo de batches que uma forward reference pode ficar nao-resolvida.               | [09](../guides/09-streaming-generation.md) |
| `--streaming-strategy <S>`    | `coordinator`, `local`         | `round-robin`            | Strategy de allocate_batch. `round-robin` ou `fennel`.                                     | [09](../guides/09-streaming-generation.md) |
| `--fennel-alpha <F>`          | `coordinator`, `local`         | (none)                   | Penalidade de capacidade do Fennel; requer `--streaming-strategy fennel`.                  | [09](../guides/09-streaming-generation.md) |
| `--dispatch-mode <M>`         | `coordinator`, `local`         | `auto`                   | `auto` / `push` / `pull`. SPEC-21 R34.                                                     | [09](../guides/09-streaming-generation.md) |
| `--recycle-policy <P>`        | `bench`                        | `disable-under-delta`    | Politica de recycle do free-list. `disable-under-delta` / `border-clean` / `disable`.       | [10](../guides/10-arena-management.md)     |
| `--representation <R>`        | `bench`                        | `dense`                  | Construcao de subnet via `Net` denso ou `SparseNet`. `dense` / `sparse`.                    | [10](../guides/10-arena-management.md)     |
| `--csv-sparse <PATH>`         | `bench`                        | none                     | Sub-CSV para acceptance gate dual_tree (SPEC-09 §3.4.5).                                    | [10](../guides/10-arena-management.md)     |

### Elastic grid (SPEC-20)

| Flag                          | Aplica a                  | Default | Descricao                                                                          | Guia                                       |
|-------------------------------|---------------------------|---------|------------------------------------------------------------------------------------|--------------------------------------------|
| `--hybrid`                    | `coordinator`, `local`    | off     | Coordinator atua como worker (self-partition, `WorkerId = 0`).                     | [08](../guides/08-elastic-grid.md)         |
| `--elastic-join`              | `coordinator`, `local`    | off     | Drena conexoes pendentes entre rodadas. Auto-on com `--hybrid` ou `--elastic-departure`. | [08](../guides/08-elastic-grid.md)   |
| `--elastic-departure`         | `coordinator`, `local`    | off     | Recupera particoes de workers que caem. Auto-ativa `--retain-partitions`.          | [08](../guides/08-elastic-grid.md)         |
| `--retain-partitions`         | `coordinator`, `local`    | off     | Forca o coord a guardar `retained_initial` + `retained_last_acked`.                | [08](../guides/08-elastic-grid.md)         |
| `--checkpoint-partitions`     | `coordinator`, `local`    | off     | Persistencia em disco das retained partitions (planejado).                         | [08](../guides/08-elastic-grid.md)         |
| `--initial-wait-timeout <S>`  | `coordinator`, `local`    | 30      | Janela inicial em segundos antes de entrar em SoloReducing (com `--hybrid`).       | [08](../guides/08-elastic-grid.md)         |
| `--join-window-min-ms <MS>`   | `coordinator`             | 50      | Janela minima de drain de joins entre rodadas.                                     | [08](../guides/08-elastic-grid.md)         |
| `--join-window-max-ms <MS>`   | `coordinator`             | 500     | Janela maxima.                                                                     | [08](../guides/08-elastic-grid.md)         |
| `--solo-budget <N>`           | `coordinator`, `local`    | 10000   | Interacoes por batch no `SoloReducing`. `u32::MAX` desativa polling.               | [08](../guides/08-elastic-grid.md)         |

### Wire format / transporte (SPEC-17 / SPEC-18 / SPEC-19)

| Flag                          | Aplica a                  | Default | Descricao                                                                          | Guia                                       |
|-------------------------------|---------------------------|---------|------------------------------------------------------------------------------------|--------------------------------------------|
| `--delta-mode`                | `coordinator`, `local`    | off     | Ativa o protocolo delta (workers stateful + BorderGraph).                          | [06](../guides/06-delta-protocol.md)       |
| `--use-zero-copy`             | `coordinator`, `worker`   | off     | Solicita rkyv archive em hot-path messages. Requer build com `--features zero-copy`. | [07](../guides/07-zero-copy.md)         |
| `--compression-threshold <B>` | `coordinator`, `worker`   | 1024    | LZ4 frame compression threshold (bytes). `0` comprime tudo.                        | [07](../guides/07-zero-copy.md)            |
| `--transport <T>`             | `coordinator`, `worker`   | `tcp`   | Backend de transporte. `tcp` ou `unix` (UDS).                                      | [05](../guides/05-distributed-tcp.md)      |
| `--socket-path <PATH>`        | `coordinator`, `worker`   | (none)  | Path da UDS quando `--transport unix`.                                             | [05](../guides/05-distributed-tcp.md)      |
| `--no-tcp-nodelay`            | `coordinator`, `worker`   | off     | Desativa TCP_NODELAY (ativa Nagle). Default e `nodelay=on`.                        | [05](../guides/05-distributed-tcp.md)      |
| `--send-buffer <B>`           | `coordinator`, `worker`   | 4 MiB   | SO_SNDBUF (bytes).                                                                 | [05](../guides/05-distributed-tcp.md)      |
| `--recv-buffer <B>`           | `coordinator`, `worker`   | 4 MiB   | SO_RCVBUF (bytes).                                                                 | [05](../guides/05-distributed-tcp.md)      |
| `--keepalive <S>`             | `coordinator`, `worker`   | 30      | TCP keepalive idle em segundos. `0` desativa.                                      | [05](../guides/05-distributed-tcp.md)      |

> **Nota.** As flags por subcomando documentadas nas tabelas anteriores (`generate`, `inspect`, `reduce`, `local`, `compute`, `coordinator`, `worker`, `bench`) continuam validas. As flags de v2 acima estao **adicionadas** em cima do que ja existia em v1; este e um overview unificado.

---

## Verificacoes pre-push (desenvolvimento)

Antes de commit/push ou criar tag de release, rode as mesmas checagens que o CI faz:

```bash
cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test && cargo build --release
```

Passo-a-passo:

| Comando                                                | Propósito                                     |
|--------------------------------------------------------|-----------------------------------------------|
| `cargo fmt --check`                                    | rustfmt; corrija com `cargo fmt`              |
| `cargo clippy --all-targets --all-features -- -D warnings` | Linter com warnings como erro             |
| `cargo test`                                           | Todos os 690+ testes passam, 0 warnings       |
| `cargo build --release`                                | Binario final compila                         |

Antes de criar tag:

```bash
# 1. Verificacoes completas (acima)
# 2. Atualizar version = "X.Y.Z" em Cargo.toml
# 3. Commit e tag
git add -A && git commit -m "release: vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags
```

A tag `v*` dispara CI (`ci.yml`), Release (`release.yml`, binarios Linux/Windows + checksums) e Docker (`docker.yml`, push para GHCR).
