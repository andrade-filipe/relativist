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
