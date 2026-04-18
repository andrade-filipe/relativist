# Troubleshooting

Sintomas comuns e o que fazer. Para erros especificos de campanha, ver [benchmarks/limitations.md](../benchmarks/limitations.md) (L1-L7) e os troubleshooting por campanha em [benchmarks/campaigns/](../benchmarks/campaigns/).

## Instalacao

### Windows SmartScreen bloqueia `relativist.exe`

**Sintoma.** Ao rodar o binario baixado, o Windows exibe "Windows protected your PC" ou "unrecognized app".

**Causa.** O binario nao e assinado com um cert EV. SmartScreen flagga executaveis novos com baixa reputacao.

**Solucao.** Click em "More info" -> "Run anyway". Alternativamente, buildar localmente com `cargo build --release` — o binario gerado pelo proprio compilador nao dispara SmartScreen.

Isso esta registrado como UX issue aberto (ver ROADMAP 2.37-2.39 — Tauri GUI + distribution).

### `rustc` nao encontrado ou versao antiga

```bash
rustup update stable
rustc --version    # MSRV atual esta no Cargo.toml
```

### Cargo build falha com `link.exe not found` (Windows)

Instale Build Tools do Visual Studio (workload "Desktop development with C++") ou MSYS2 com `mingw-w64`.

## Execucao local

### `relativist: command not found`

Adicione `target/release/` ao PATH, ou rode com path explicito: `./target/release/relativist`.

### `inspect` mostra `Normal Form: no` apos `reduce`

Nao deveria acontecer em redes terminantes. Verifique:

1. Se voce passou `--max-steps <N>` muito baixo em `reduce`.
2. Se a rede tem ciclos nao-terminantes (fora do escopo do TCC; ver premissa P6).
3. Se `redex_queue` ficou com itens stale apos um delete de agente (bug — abra issue).

### `compute exp` retorna numero errado ou "decode failed"

Esperado — `compute exp` resolve corretamente, mas o decoder atual nao caminha DUP ciclico. Ver [L5](../benchmarks/limitations.md#l5). Use `inspect` para confirmar que a estrutura esta correta.

## Docker

### `docker compose up` nao encontra `data/input.bin`

```bash
# Garanta que o volume esta montado e o arquivo existe
ls data/input.bin
```

### `docker-compose.yml` monta caminho errado no Windows (Git Bash)

**Sintoma.** Git Bash converte `/data` para `C:\Program Files\Git\data`, volume nao monta.

**Solucao.** Exporte `MSYS_NO_PATHCONV=1` antes de `docker compose` quando o comando tiver paths estilo unix:

```bash
MSYS_NO_PATHCONV=1 docker compose up
```

Ou use PowerShell (sem conversao MSYS).

### Coordinator sai antes de flushar `metrics.json`

**Sintoma.** `metrics.json` ausente apos `docker compose up --abort-on-container-exit --exit-code-from coordinator`.

**Causa.** O flag `--abort-on-container-exit` SIGKILLa o coordinator assim que o primeiro worker sai.

**Mitigacao.** Use `docker compose up -d` + `docker wait relativist-coordinator-1` em vez de `--abort-on-container-exit`. Padrao usado por `scripts/bench_docker_resume2.sh`. Ver [L7](../benchmarks/limitations.md#l7).

### `docker system prune` travou o Docker Desktop

Reinicie o Docker Desktop. Se o disco encheu (`no space left on device`), rode:

```bash
docker system df
docker builder prune -af    # Libera cache de build
docker image prune -af      # Libera imagens nao usadas
```

## TCP / Distribuido

### Worker conecta mas e rejeitado

**Sintoma.** Log do worker: `authentication failed` ou `version mismatch`.

**Causas comuns.**

1. `--auth-token` nao casa entre coordinator e worker.
2. Binarios de versoes diferentes (wire protocol incompativel). Rode `relativist --version` nos dois lados.
3. `--features zero-copy` em um lado so (ver [guia 07](../guides/07-zero-copy.md)).

### Worker nao alcanca o coordinator

Teste conectividade em baixo nivel:

```bash
# Do worker para o coordinator:
nc -vz <COORD_HOST> 9000
# Esperado: (host) 9000 open
```

Se timeout: firewall (Windows Defender Firewall, iptables). Libere a porta TCP 9000 (ou a que voce esta usando) no host do coordinator.

### `peak_memory_bytes` ausente no metrics JSON

Esperado em Windows/macOS. O campo e Linux-only (via `/proc/self/status`).

## Benchmarks

### `correct=false` em alguma linha do CSV

**Pare imediatamente.** Isso e regressao de G1. Acoes:

1. Rode `cargo test` — 690+ testes devem passar.
2. Veja o `raw/phase1/<bench>.log` (ou `raw/phase2/metrics_*.json`) do config afetado.
3. Abra issue no repo com o datapoint + log.

### Wall-clock muito maior que esperado

1. Power plan voltou para Balanced? `powercfg /getactivescheme` (Windows).
2. CPU throttling por calor? Deixe esfriar 30 min.
3. Carga de fundo (browser, IDE, AV scan)? Ver [v1-local-baseline §1.4](../benchmarks/campaigns/v1-local-baseline.md#14-environment-hygiene-windows-11).

### CV > 0.15 em varios configs

Maquina tinha carga de fundo. Re-rode Phase 1 em ambiente limpo. Marcar individualmente com `keep` **nao** resolve padrao sistemico. Ver [v1-local-baseline §4.2](../benchmarks/campaigns/v1-local-baseline.md#42-triagem-cv) para triagem.

### Frame cap excedido (1 GiB)

Sintoma: `protocol: frame size exceeds limit`. Causa: particao serializada > 1 GiB sob bincode v1 + CompactSubnet. Ocorre em sizes de stress (50M com w=1/w=2). Ver [L6](../benchmarks/limitations.md#l6); mitigacao futura em ROADMAP 2.23 (wire compaction) e [SPEC-18](../../specs/SPEC-18-wire-format-v2.md).

## Memoria

### Coordinator OOM em sizes grandes

WSL2 VM default tem 15 GiB. Em 50M agentes bincode v1 usa ~3-4 GB so de particao serializada — se o coordinator mantem duas copias (entrada + merge), explode.

Mitigacoes:

- Aumente a memoria do WSL2 (`.wslconfig` no Windows, `memory=24GB`).
- Use `--features zero-copy` se disponivel (SPEC-18 evita a copia do deserialize).
- Use `--delta-mode` (SPEC-19) — coordinator nao carrega o net mergeado ate o final.

### Worker swappa no Linux

`free -h` + `vmstat 1`. Se `si/so` > 0 durante o benchmark, voce esta swappando. Aumente RAM, reduza `workers`, ou reduza `size`.

## Tests

### `cargo test` falha apos pull de novo branch

```bash
cargo clean
cargo test
```

Se ainda falhar, checar se `Cargo.lock` foi commitado. Para hotfixes, pode precisar regerar: `rm Cargo.lock && cargo build`.

### Teste especifico intermitente

Rode com backtrace completo:

```bash
RUST_BACKTRACE=full cargo test -- --nocapture --test-threads=1 <NOME_DO_TESTE>
```

`--test-threads=1` isola concorrencia; abrir issue se reproduz deterministicamente em single-thread.

## Relatar bug

- Anexar: `relativist --version`, `rustc --version`, OS + version, comando completo, output, `raw/` log relevante.
- Se bug de correctness (G1): incluir o `.bin` do input (ou o comando `generate` que o produziu).
- Issues: [github.com/andrade-filipe/relativist/issues](https://github.com/andrade-filipe/relativist/issues).
