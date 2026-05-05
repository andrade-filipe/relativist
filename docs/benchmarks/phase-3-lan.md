# Phase 3 — TcpNetwork (maquinas reais)

Phase 3 executa o protocolo BSP em **maquinas fisicas diferentes** conectadas por rede Ethernet real. Expoe o custo que nenhum outro modo consegue reproduzir: latencia RTT, throughput limitado, jitter e contencao de NIC. E a campanha que transforma Relativist de "implementacao correta de ICs distribuidos" em "evidencia empirica sobre o custo real de distribuir reducao de ICs num grid".

> **Pre-requisitos.** Phase 3 v2 corre em **dois eixos** que travam baselines independentes contra o canonico Phase 2 docker:
>
> - **Axis 1 — bincode + delta protocol (build default).** Replica `results/locked/v2_post_d012_baseline_2026-05-05/` em LAN real. A medida principal e `t_network = t_lan - t_localhost` para a mesma config; o subtraendo vem do baseline canonico no mesmo binario.
> - **Axis 2 — zero-copy + delta protocol (`--features zero-copy`).** Mesma matriz; o ganho esperado e reducao do componente `compute` (deserialize CPU) e leve aumento do componente `network` (rkyv archive size > bincode).
>
> Os dois eixos so fazem sentido depois que `v2_post_d012_baseline_2026-05-05` esta travado e o binario compila os dois feature sets (`cargo build --release` e `cargo build --release --features zero-copy`). Nao comece Phase 3 sem ter o canonico v2 travado.

## 1. O que Phase 3 mede

### Medida primaria — overhead de LAN por subtracao

Para cada triple `(bench, size, workers)` em cada eixo:

```
t_network = t_lan  -  t_localhost
            ^^^^^^    ^^^^^^^^^^^^
            aqui      v2_post_d012_baseline_2026-05-05/summary.csv  (Axis 1)
                      v2_zero_copy_baseline_<data>/summary.csv      (Axis 2 — a travar)
```

Phase 3 produz `t_lan` em LAN real para cada eixo; o `t_localhost` correspondente vem do baseline canonico do mesmo binario/feature-set. A diferenca e a fracao de wall-clock atribuida a latencia de fio + banda + jitter — tudo que Docker em loopback nao consegue reproduzir. E o numero que o artigo do TCC reporta como **"custo de rede do grid"** decomposto por feature-set.

Para a subtracao valer: *tudo* tem de ser identico menos a rede. Mesmo binario, mesmas features ativas, mesmos bytes de input, mesma estrategia de particao, mesmas 10 repeticoes, mesmo modo BSP (lenient por padrao). Qualquer drift invalida o resultado.

> **Observacao para v2.** Em v2, `t_localhost` ja vem decomposto em `compute_time_secs + network_time_secs + merge_time_secs` (D-012 instrumentation, RF-04/05 closures). A subtracao nao se faz so no `wall_clock_mean` — faz-se **componente a componente**, expondo qual fase ganha/perde com o salto para LAN. Para `ep_500k w=1` localhost (canonico): `wall = 0.460 s = compute 0.10 + network 0.39 + merge 0.04`. Em LAN, espera-se `network` crescer significativamente; `compute` e `merge` devem ficar invariantes.

### Medida secundaria — RTT por rodada sob strict BSP

O modo `--strict-bsp` (SPEC-05 R30a), ja validado em Phase 1, garante que `cascade_cross(N) = N` rodadas com `workers >= 2` e `dual_tree(d) = d` rodadas. Em Phase 3, as mesmas topologias produzem as mesmas contagens — mas cada rodada agora inclui um RTT real:

```
t_round_lan  ≈  t_round_localhost  +  RTT_round
             ≈  t_round_localhost  +  2 * RTT_wire * (split_msg + merge_msg)
```

Essa medida permite caracterizar o protocolo de grid sob rede realista.

### O que Phase 3 NAO e

- **Nao e** benchmark de throughput absoluto. Phase 1 ja mostrou que o ciclo BSP tem overhead algoritmico alto ([L1](limitations.md)); Phase 3 nao tenta esconder isso.
- **Nao e** teste sob rede adversa. Sem injecao de packet loss, sem WAN, sem contencao proposital de NIC. LAN = melhor caso distribuido.
- **Nao e** teste do fix L2. L2 e validado por Phase 1 strict. Phase 3 apenas *usa* strict mode para extrair o sinal de RTT limpo.

## 2. Hardware e rede necessarios

### Maquinas

Para cumprir SPEC-09 R27 ("pelo menos 4 e 8 workers"), voce precisa de **9 maquinas**: 1 coordinator + 8 workers.

| Opcao | Maquinas                 | Configs LAN    | Cumpre R27?                                           |
|-------|--------------------------|----------------|-------------------------------------------------------|
| A     | 5 (1 coord + 4 workers)  | W ∈ {1, 2, 4}  | Parcial (4 sim, 8 nao — documentar exclusao)         |
| B     | 9 (1 coord + 8 workers)  | W ∈ {1, 2, 4, 8} | Sim, integralmente                                  |
| C     | 3 (1 coord + 2 workers)  | W ∈ {1, 2}     | Nao. Apenas validacao de protocolo, nao Phase 3 oficial |

**Alvo para o TCC:** Opcao B. Opcao A e aceitavel com ressalva no manifest. Opcao C e so smoke test.

### Requisitos por maquina (homogeneos)

| Componente | Minimo                                | Motivo                                         |
|------------|---------------------------------------|------------------------------------------------|
| CPU        | 4 cores fisicos                       | Worker roda `reduce_all` single-threaded        |
| RAM        | 4 GiB livres                          | Pior caso `ep_annihilation_con=5M` w=1: ~1.5 GiB |
| Disco      | 2 GiB livres                          | `output.bin` + `metrics.json` por run           |
| OS         | Linux (Debian/Ubuntu/Fedora/Arch)     | Tooling assume POSIX (ssh/scp/rsync)            |
| NIC        | 1 Gbps ethernet, mesmo switch, mesma VLAN | Wi-Fi introduz jitter 5-10x                  |

### Homogeneidade

A subtracao `t_network = t_lan - t_localhost` assume que a unica diferenca entre Phase 2 e Phase 3 e a presenca do fio. Se as maquinas de Phase 3 tiverem CPU ou binario diferentes, voce mede uma mistura de delta-hardware + delta-rede, e a isolacao cai.

Caminho limpo:

1. Um modelo de CPU para todas as maquinas (desktops identicos).
2. Compile o binario **uma vez** na build host a partir da tag `v0.10.0-bench`; copie via `scp`. **Nao recompile por maquina.**
3. Antes de comecar, confirme mesmo `relativist --version` e mesmo sha256 do binario em todos os nos.

### Topologia de referencia

```
      +---------------+
      | Coordinator   |  coord  (10.0.0.10)
      +-------+-------+
              |
              |  1 Gbps Ethernet, um switch, uma VLAN
              |
    +---------+-----------+------------+-----------+
    |         |           |            |           |
  +-+--+   +--+-+      +--+-+      +---+-+     +---+-+
  | W1 |   | W2 |  ..  | W4 |      | W6  |     | W8  |
  |.11 |   |.12 |      |.14 |      |.16  |     |.18  |
  +----+   +----+      +----+      +-----+     +-----+
```

Regras:

- **Um switch**, nao roteador.
- **Mesma VLAN / mesmo broadcast L2.**
- **Cabo, nao Wi-Fi.**
- **Sem trafego pesado externo** durante a campanha.
- **IP estatico ou DHCP com lease reservado.**

### Medir baseline de rede

**RTT piso** — nenhum `t_network` pode ser menor que `RTT * num_round_trips`. Se for, bug de medicao.

```bash
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    echo "=== $W ==="
    ping -c 20 -i 0.2 -q "$W" | tail -2
done
```

Tipico cabeado: `avg 0.1-0.5 ms`, `max 0.5-2 ms`, `mdev < 0.2 ms`. Se `avg > 2 ms` ou `mdev > 1 ms`, investigue.

**Banda efetiva:**

```bash
# Em cada worker
iperf3 -s &

# Do coordinator
iperf3 -c 10.0.0.11 -t 10
```

Espere `>= 900 Mbps` num link nominal de 1 Gbps.

## 3. Setup de software

### 1. Build host (uma vez por eixo)

Em v2 voce produz **dois binarios** — um por eixo. O hash de cada binario entra no manifest do eixo correspondente.

```bash
git clone git@github.com:andrade-filipe/relativist.git
cd relativist
git checkout v0.20.0-pre.1   # ou a tag v2 alvo da campanha

rustup override set 1.94.1 || rustup install 1.94.1

# --- Axis 1: bincode + delta protocol (default) ---
cargo build --release
cp target/release/relativist target/release/relativist-axis1
./target/release/relativist-axis1 --version
sha256sum target/release/relativist-axis1

# --- Axis 2: zero-copy + delta protocol ---
cargo build --release --features zero-copy
cp target/release/relativist target/release/relativist-axis2
./target/release/relativist-axis2 --version
sha256sum target/release/relativist-axis2
```

Guarde os dois sha256s — um entra no manifest do Axis 1, outro no manifest do Axis 2. **Importante:** em uma campanha real, distribua e rode **um eixo por vez** para nao misturar binarios na malha. Trocar o binario em todos os nos entre eixos e parte do protocolo.

### 2. Distribuir o binario

```bash
for HOST in 10.0.0.10 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    scp target/release/relativist "$USER@$HOST:/tmp/relativist"
    ssh "$USER@$HOST" "sudo install -m 755 /tmp/relativist /usr/local/bin/relativist && relativist --version"
done
```

Confira sha256 em todos os nos:

```bash
for HOST in 10.0.0.10 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    echo -n "$HOST: "
    ssh "$USER@$HOST" "sha256sum /usr/local/bin/relativist" | awk '{print $1}'
done
```

### 3. Abrir porta 9000/tcp

```bash
# ufw
sudo ufw allow from 10.0.0.0/24 to any port 9000 proto tcp
sudo ufw reload

# iptables
sudo iptables -A INPUT -p tcp --dport 9000 -s 10.0.0.0/24 -j ACCEPT
```

Nao exponha `9000/tcp` para a internet.

### 4. Sincronizar NTP

```bash
sudo apt install -y chrony
sudo systemctl enable --now chronyd
chronyc tracking
```

Drift < 100 ms.

### 5. Gerar e distribuir token

```bash
relativist coordinator --workers 1 --bind 127.0.0.1:9999 \
    --token auto --token-file /tmp/rel-token \
    --dry-run 2>/dev/null || true
chmod 600 /tmp/rel-token

for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    scp /tmp/rel-token "$USER@$W:/tmp/rel-token"
    ssh "$USER@$W" "chmod 600 /tmp/rel-token"
done
```

Gere **novo token** por campanha.

### 6. Pre-estagiar inputs

Gere a matriz (mesma que Phase 2) + subset strict:

```bash
cd ~/phase3/data

relativist generate ep-annihilation-con -n 500000  -o ep_con_500k.bin
relativist generate ep-annihilation-con -n 1000000 -o ep_con_1M.bin
relativist generate ep-annihilation-con -n 5000000 -o ep_con_5M.bin
relativist generate dual-tree -d 18 -o dual_tree_18.bin
relativist generate dual-tree -d 20 -o dual_tree_20.bin
relativist generate dual-tree -d 22 -o dual_tree_22.bin
relativist generate con-dup-expansion -n 1000 -o condup_1k.bin
relativist generate con-dup-expansion -n 5000 -o condup_5k.bin

# Subset strict-BSP
relativist generate cascade-cross -n 10   -o cc_10.bin
relativist generate cascade-cross -n 50   -o cc_50.bin
relativist generate cascade-cross -n 100  -o cc_100.bin
relativist generate cascade-cross -n 500  -o cc_500.bin
relativist generate cascade-cross -n 1000 -o cc_1k.bin
relativist generate dual-tree -d 6  -o dual_tree_6.bin
relativist generate dual-tree -d 10 -o dual_tree_10.bin
relativist generate dual-tree -d 14 -o dual_tree_14.bin
```

Rsync para cada worker:

```bash
for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    rsync -av --progress ~/phase3/data/ "$USER@$W:~/phase3/data/"
done
```

**Confira sha256 byte-identico em todas as maquinas:**

```bash
cd ~/phase3/data && sha256sum *.bin > /tmp/input_sha256.txt

for W in 10.0.0.11 10.0.0.12 10.0.0.13 10.0.0.14; do
    diff <(ssh "$USER@$W" "cat /tmp/input_sha256.txt") /tmp/input_sha256.txt \
        && echo "$W OK" || echo "$W DRIFT"
done
```

**Drift aqui e a causa #1 de bugs silenciosos em Phase 3.** Pegue no pre-flight.

## 4. Sanity checks — antes da campanha

**Check 1 — TCP loopback com binario real.**

```bash
# Terminal 1
relativist coordinator --workers 1 --bind 127.0.0.1:9000 \
    --token "$(cat /tmp/rel-token)" \
    -i ~/phase3/data/ep_con_500k.bin \
    -o /tmp/out_loop.bin -m /tmp/metrics_loop.json --log-format json

# Terminal 2
relativist worker --coordinator 127.0.0.1:9000 \
    --token "$(cat /tmp/rel-token)" --log-format json
```

Esperado: coordinator sai com 0, `/tmp/out_loop.bin` existe, `/tmp/metrics_loop.json` tem `rounds` de tamanho 1 (lenient).

**Check 2 — smoke cross-machine.**

```bash
# Coordinator (10.0.0.10)
relativist coordinator --workers 2 --bind 0.0.0.0:9000 \
    --token "$(cat /tmp/rel-token)" \
    -i ~/phase3/data/ep_con_500k.bin \
    -o /tmp/out_smoke.bin -m /tmp/metrics_smoke.json --log-format json

# Em 10.0.0.11 e 10.0.0.12 (via ssh)
TOKEN=$(cat /tmp/rel-token)
relativist worker --coordinator 10.0.0.10:9000 --token "$TOKEN"
```

Diagnosticos:

| Sintoma                                          | Causa provavel       |
|--------------------------------------------------|----------------------|
| Worker trava em `connecting to 10.0.0.10:9000`   | Firewall/rota — `telnet 10.0.0.10 9000` |
| Worker conecta mas falha no register             | Token mismatch       |
| Coordinator trava em `waiting for N workers`     | Register incompleto  |
| Coordinator sai com `protocol error`             | Binario heterogeneo — rever sha256 |

**Check 3 — G1 equivalencia contra sequencial.**

```bash
relativist reduce -i ~/phase3/data/ep_con_500k.bin -o /tmp/out_seq.bin

relativist inspect -i /tmp/out_smoke.bin > /tmp/dist.txt
relativist inspect -i /tmp/out_seq.bin   > /tmp/seq.txt
diff /tmp/dist.txt /tmp/seq.txt && echo "G1 OK" || echo "G1 FAIL"
```

Se `G1 FAIL`, **pare.** Nao rode a campanha com G1 quebrado.

## 5. Axis 1 — bincode + delta protocol (passo a passo)

Axis 1 usa o binario `relativist-axis1` (default features). Replica em LAN o que `v2_post_d012_baseline_2026-05-05` mediu em docker localhost.

### 5.1 Coordinator (10.0.0.10)

```bash
# Variaveis comuns
TOKEN=$(cat /tmp/rel-token)
INPUT=~/phase3/data/ep_con_500k.bin
OUT=/tmp/axis1_out.bin
METRICS=/tmp/axis1_metrics.json

# Coordinator com features default (Axis 1)
relativist-axis1 coordinator \
    --workers 4 --bind 0.0.0.0:9000 \
    --auth-token "$TOKEN" \
    --delta-mode \
    --chunk-size 10000 \
    --max-pending-lifetime 16 \
    --recv-buffer 4194304 --send-buffer 4194304 \
    -i "$INPUT" -o "$OUT" -m "$METRICS" \
    --log-format json
```

Esperado: bloqueia em `waiting for 4 workers to register` ate os workers conectarem; depois imprime `round 0 dispatched`, `round 1 collected`, ..., `Global Normal Form` e sai com 0.

### 5.2 Workers (10.0.0.11..14)

Em **cada** worker, em terminais paralelos (ou via tmux/screen):

```bash
TOKEN=$(cat /tmp/rel-token)
relativist-axis1 worker \
    --coordinator 10.0.0.10:9000 \
    --auth-token "$TOKEN" \
    --recv-buffer 4194304 --send-buffer 4194304 \
    --log-format json
```

### 5.3 Loop de campanha (4 configs × 4 worker counts × 10 reps = 160 runs por bench)

Driver shell esqueletico (a versao real esta em `scripts/bench_phase3_locked.sh`, a escrever; o esqueleto abaixo e o contrato funcional):

```bash
#!/usr/bin/env bash
set -euo pipefail

TOKEN=$(cat /tmp/rel-token)
RESULTS=~/phase3/results/axis1
mkdir -p "$RESULTS"

for BENCH in ep_annihilation_con dual_tree condup_expansion; do
  for SIZE in 500000 1000000 5000000; do
    for W in 1 2 4 8; do
      INPUT=~/phase3/data/${BENCH}_${SIZE}.bin
      [ -f "$INPUT" ] || continue
      for REP in $(seq 1 10); do
        SLOT="${BENCH}_${SIZE}_w${W}_rep${REP}"
        # Spawn workers (W workers) via SSH em paralelo
        for I in $(seq 1 "$W"); do
          HOST="10.0.0.$((10 + I))"
          ssh "$USER@$HOST" "relativist-axis1 worker \
              --coordinator 10.0.0.10:9000 --auth-token $TOKEN \
              --log-format json" >"$RESULTS/${SLOT}_w${I}.log" 2>&1 &
        done
        # Lance o coordinator
        relativist-axis1 coordinator --workers "$W" --bind 0.0.0.0:9000 \
            --auth-token "$TOKEN" --delta-mode \
            -i "$INPUT" \
            -o "$RESULTS/${SLOT}_out.bin" \
            -m "$RESULTS/${SLOT}_metrics.json" \
            --log-format json >"$RESULTS/${SLOT}_coord.log" 2>&1
        wait
        # Validacao G1 cedo (sem esperar agregacao)
        relativist-axis1 inspect -i "$RESULTS/${SLOT}_out.bin" \
            >"$RESULTS/${SLOT}_inspect.txt"
      done
    done
  done
done
```

### 5.4 Validacao em rep — G1

Cada run, alem do output, compara contra a reducao sequencial:

```bash
# Uma vez, gere a reducao sequencial de cada input
for INPUT in ~/phase3/data/*.bin; do
    OUT_SEQ="${INPUT%.bin}_seq.bin"
    relativist-axis1 reduce -i "$INPUT" -o "$OUT_SEQ"
done

# Apos o run, em cada SLOT:
diff <(relativist-axis1 inspect -i "${SLOT}_out.bin") \
     <(relativist-axis1 inspect -i "${INPUT%.bin}_seq.bin") \
     && echo "$SLOT G1 OK" \
     || echo "$SLOT G1 FAIL"
```

Phase 3 v2 espera **0 falhas** em todas as 160+ corridas, batendo o `all_correct=true` 32/32 do baseline canonico Phase 2.

### 5.5 Saidas esperadas

Cada `metrics.json` carrega:

```json
{
  "wall_clock_secs": 0.873,
  "compute_time_secs": 0.18,
  "network_time_secs": 0.62,
  "merge_time_secs": 0.07,
  "total_interactions": 1234567,
  "mips_mean": 1.41,
  "rounds": [...]
}
```

A subtracao componente-a-componente contra `v2_post_d012_baseline_2026-05-05/summary.csv` (mesmo bench/size/workers, mesmo `delta_mode=true`):

```
delta_compute = compute_lan - compute_localhost   ~0      (esperado)
delta_merge   = merge_lan   - merge_localhost     ~0      (esperado)
delta_network = network_lan - network_localhost   >> 0    (a medida principal)
```

`delta_network` em segundos, dividido por `rounds`, da o RTT efetivo medio por rodada.

## 6. Axis 2 — zero-copy + delta protocol (passo a passo)

Axis 2 usa o binario `relativist-axis2` (compilado com `--features zero-copy`). A diferenca operacional vs Axis 1 e:

1. **Build flag.** `cargo build --release --features zero-copy`. Verifique em `relativist-core/Cargo.toml [features]`: `zero-copy = ["dep:rkyv"]`.
2. **Runtime flag.** Adicione `--use-zero-copy` no coordinator E nos workers. Ambos os lados precisam ter sido compilados com a feature; o handshake rejeita mismatch (SPEC-18 R20-R27).
3. **Same input file.** Os `.bin` files de Phase 3 (gerados em §3.6) sao identicos para os dois eixos.

### 6.1 Coordinator (Axis 2)

```bash
TOKEN=$(cat /tmp/rel-token)
INPUT=~/phase3/data/ep_con_500k.bin
OUT=/tmp/axis2_out.bin
METRICS=/tmp/axis2_metrics.json

relativist-axis2 coordinator \
    --workers 4 --bind 0.0.0.0:9000 \
    --auth-token "$TOKEN" \
    --delta-mode \
    --use-zero-copy \
    --chunk-size 10000 \
    --max-pending-lifetime 16 \
    --recv-buffer 4194304 --send-buffer 4194304 \
    --compression-threshold 1024 \
    -i "$INPUT" -o "$OUT" -m "$METRICS" \
    --log-format json
```

### 6.2 Workers (Axis 2)

```bash
TOKEN=$(cat /tmp/rel-token)
relativist-axis2 worker \
    --coordinator 10.0.0.10:9000 \
    --auth-token "$TOKEN" \
    --use-zero-copy \
    --recv-buffer 4194304 --send-buffer 4194304 \
    --log-format json
```

### 6.3 Confirmando o caminho rkyv

Os logs dos workers e coordinator em modo JSON ja carregam o flag de feature. Para confirmar que o caminho zero-copy esta efetivamente em uso (em vez do silencioso fallback bincode):

```bash
# Procure no log do coordinator pelo flag FLAG_ARCHIVED no header dos frames recebidos
jq 'select(.fields.frame_flags // "") | contains("rkyv")' axis2_coord.log | head -5

# Cross-check: handshake deveria registrar feature_zero_copy=true em ambos os lados
jq 'select(.fields.event == "handshake_complete") | .fields' axis2_coord.log
```

### 6.4 Loop de campanha — diff vs Axis 1

O driver de Axis 2 e o mesmo `bench_phase3_locked.sh` com tres mudancas:

```bash
diff axis1_driver.sh axis2_driver.sh
# < BIN=relativist-axis1
# > BIN=relativist-axis2
# < ZC_FLAG=
# > ZC_FLAG=--use-zero-copy
# < RESULTS=~/phase3/results/axis1
# > RESULTS=~/phase3/results/axis2
```

E em cada chamada o `ZC_FLAG` e adicionado ao coordinator e worker.

### 6.5 Saidas esperadas vs Axis 1

Para o mesmo `(bench, size, workers)`:

| Componente            | Axis 1 (bincode)  | Axis 2 (rkyv)         | Direcao esperada |
|-----------------------|-------------------|-----------------------|------------------|
| `compute_time_secs`   | baseline          | **menor** (~0.0-0.1x) | rkyv elimina deserialize CPU |
| `network_time_secs`   | baseline          | igual ou levemente maior | archive size > bincode size; LZ4 fecha o gap |
| `merge_time_secs`     | baseline          | igual                 | merge nao toca rkyv |
| `wall_clock_secs`     | baseline          | menor em redes grandes (>1 MB) | onde deserialize dominava |

Em redes pequenas (<100 KB de particao), o ganho e marginal e pode ser dentro do ruido (CV ~5-9%). Documente isso no manifest.

## 7. Orquestracao, agregacao e analise

Por volume, o texto detalhado da orquestracao (script SSH de ~400 linhas, Ansible alternativo, CV triage, pos-campanha) fica versionado em:

- **Driver shell:** `scripts/bench_phase3_locked.sh` (a escrever; comparte estrutura com `scripts/bench_docker_v2.sh`).
- **Aggregator:** `scripts/aggregate_phase3.py` (a escrever; analogo do parser de metrics em `bench_docker_v2.sh`).
- **Schema de saida:** deve bater byte-por-byte com `summary.csv` do baseline canonico v2 — mesmo header (16 colunas), mesma ordem, mesma precisao numerica. As colunas `compute_time_secs`, `network_time_secs`, `merge_time_secs`, `mips_mean` precisam estar populadas (D-012 instrumentation).

O esqueleto do `run_one()` bash, a matriz `BENCH_SIZES`/`WORKER_COUNTS`/`REPS=10` e o ciclo de validacao por run (worker spawn via ssh → coordinator → coleta de logs → inspect contra referencia) ficam congelados em `docs/PHASE3-FINDINGS.md` apos as duas campanhas rodarem.

## 8. Manifest do snapshot — um por eixo

Apos campanha + CV triage, crie **dois manifests** — um por eixo — em diretorios separados:

```
results/locked/
  v2_lan_axis1_<data>/
    summary.csv
    detail.csv
    rounds.csv
    per_rep_metrics/
    run.log
    MANIFEST.md
  v2_lan_axis2_<data>/   (mesma estrutura)
```

Template para `MANIFEST.md` (use `results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md` como referencia exata):

```markdown
# v2_lan_axis1_<data> — Campaign Manifest

**Status:** COMPLETE — Phase 3 LAN Axis 1 (bincode + delta) campaign on <data>.

## Provenance
| Field | Value |
|---|---|
| Repository | github.com/andrade-filipe/relativist |
| Branch | v2-development |
| Tag | v0.20.0-pre.1 (or applicable) |
| Commit SHA | <SHA> |
| Binary sha256 | <hash from §3.1> |
| Features active | streaming (SPEC-21), delta (SPEC-19), arena (SPEC-22), transport (SPEC-17) |
| Features NOT active | zero-copy (Axis 2 only) |
| Operator | Filipe Andrade Nascimento |
| Run start/end | <timestamps> |

## Cluster
- Switch: <modelo>
- Coordinator: <CPU, RAM, OS, NIC>
- Workers: idem (sha256 binario identico em todos)
- Network: 1 Gbps, VLAN unica
- RTT baseline: <ping min/avg/max>
- Banda baseline: <iperf3 Gbps>

## Campaign knobs
- Bench x size: 8 combos (mesmo do baseline canonico Phase 2)
- Workers: {1, 2, 4, 8}
- Repetitions: 10
- Mode: tcp_network
- chunk-size: 10000
- max-pending-lifetime: 16
- recycle-policy: disable-under-delta (default)
- delta-mode: true

## Checksums (sha256) — CSVs finais
- summary.csv: <hash>
- detail.csv: <hash>
- rounds.csv: <hash>
- run.log: <hash>

## Relationship to v2_post_d012_baseline_2026-05-05
- Phase 3 LAN subtrai Phase 2 Docker (`t_localhost` componente-a-componente) para extrair `t_network`.
- Comparison metric: `network_time_secs` LAN vs localhost; `compute_time_secs` deve ficar invariante.
```

O manifest de Axis 2 e identico, trocando:
- `Features active`: adicionar `zero-copy (SPEC-18 §3.5)`.
- `Features NOT active`: remover zero-copy.
- `Binary sha256`: hash do `relativist-axis2`.

Congele cada eixo com commit atomico + tag dedicada (ex.: `v0.20.0-lan-axis1`, `v0.20.0-lan-axis2`). Nao re-tagueie `v0.20.0-pre.1`.

## 9. Troubleshooting

### TCP / firewall

| Sintoma                                                     | Causa provavel                                  | Resolucao                                          |
|-------------------------------------------------------------|-------------------------------------------------|----------------------------------------------------|
| Worker trava em `connecting to 10.0.0.10:9000`              | Firewall bloqueando 9000/tcp                    | `telnet 10.0.0.10 9000` ou `nc -zv 10.0.0.10 9000`. Re-aplicar regra `ufw`/`iptables`. |
| Worker conecta mas falha em `register`                      | Token mismatch                                  | `cat /tmp/rel-token` em todos os nos; rsync se diferente. |
| Coordinator trava em `waiting for N workers`                | Worker falhou silenciosamente                   | Olhe os `*_w*.log` no driver; SSH retornou 0 mas o processo morreu. |
| `protocol error: ProtocolVersionMismatch`                   | Binarios heterogeneos                           | sha256 do binario em todos os nos; recopie a partir do build host. |
| `feature mismatch: zero-copy expected`                      | Axis 2 com binario nao-zero-copy                | Recompile com `--features zero-copy`; redistribua.  |
| `connection reset by peer` no meio de uma rodada            | Keepalive insuficiente sob NAT                  | Aumentar `--keepalive 10`; ou desativar NAT entre nos. |

### Tokio runtime / async

| Sintoma                                                     | Causa provavel                                  | Resolucao                                          |
|-------------------------------------------------------------|-------------------------------------------------|----------------------------------------------------|
| `thread 'tokio-runtime-worker' panicked at 'Cannot block...'` | Spawn_blocking dentro de async fora do contexto | Atualizar para o binario do release; bug de v2-pre.|
| `Task::abort` errors in logs                                | Worker timeout durante TCP read                 | Aumentar `collect_timeout` no coordinator (SPEC-06 R30); checar `iperf3` para link saturado. |
| Workers ficam idle e coordinator nao avanca                 | Pull dispatch sem `RequestWork`                 | Verificar PROTOCOL_VERSION ≥ 5 em ambos os lados.   |

### Clock skew entre maquinas

A subtracao `t_lan - t_localhost` nao depende de clocks sincronizados (cada `wall_clock` e medido por `Instant::now()` no proprio coordinator). Mas **logs JSON correlacionados** entre nos exigem ordem temporal coerente:

```bash
# Sincronizar via chrony — em cada no
sudo apt install -y chrony
sudo systemctl enable --now chronyd
chronyc tracking | grep "System time"     # |offset| < 100 ms
```

Drift > 5 s torna a correlacao de eventos entre coord e workers ilegivel (mensagem que sai do worker no t=12.0 chega ao coord no t=11.5 do coord). Isso nao invalida `wall_clock` mas torna debugging dificil.

### Riscos conhecidos da campanha

| Risco                                    | Prob  | Impacto  | Mitigacao                              |
|------------------------------------------|-------|----------|----------------------------------------|
| Switch satura no meio do run             | Medio | Alto     | Avisar janela; rodar off-hours         |
| Uma maquina com binario diferente        | Baixo | Critico  | sha256 no pre-flight                   |
| Coordinator morto por OOM                | Baixo | Alto     | `>= 4 GiB` livres; `vm.overcommit=1`   |
| Fallback Wi-Fi no meio da campanha       | Medio | Alto     | `nmcli radio wifi off` em cada no      |
| Leak de token                            | Baixo | Medio    | `chmod 600` em todo lugar              |
| Firewall resetado apos reboot            | Medio | Medio    | Persistir `ufw`/`iptables`             |
| Drift de relogio > 5 s                   | Baixo | Baixo    | Chrony + verificacao                   |
| Janela de 6h acaba no meio               | Medio | Baixo    | Driver re-iniciavel, skip configs com `.json` existente |
| Build de Axis 2 com feature errada       | Baixo | Critico  | `cargo build --release --features zero-copy` em separado; sha256 distinto |
| Mistura acidental de Axis 1 e Axis 2     | Baixo | Critico  | Trocar binario em todos os nos antes de iniciar o segundo eixo |

## 10. Referencias cruzadas

- **SPEC-09** (`specs/SPEC-09-benchmarks.md`): R25 modos, R27 TcpNetwork MUST, R31 10 reps.
- **SPEC-07** (`specs/SPEC-07-deployment.md`): R41 bare-metal procedure.
- **SPEC-05** (`specs/SPEC-05-merge.md`): R30a lenient vs strict BSP.
- **SPEC-06** (`specs/SPEC-06-wire-protocol.md`): wire format, handshake, token, PROTOCOL_VERSION.
- **SPEC-10** (`specs/SPEC-10-security.md`): modelo de auth de 3 niveis.
- **SPEC-17** (`specs/SPEC-17-transport-abstraction.md`): TCP transport, keepalive, buffer tuning.
- **SPEC-18** (`specs/SPEC-18-wire-format-v2.md`): bincode v2, zero-copy archive (Axis 2).
- **SPEC-19** (`specs/SPEC-19-delta-protocol.md`): delta mode, BorderGraph, R37 version handshake.
- **SPEC-21** (`specs/SPEC-21-streaming-generation.md`): chunk-size + max-pending-lifetime semantica.
- **SPEC-22** (`specs/SPEC-22-arena-management.md`): recycle-policy + dense/sparse routing.
- **`results/locked/v2_post_d012_baseline_2026-05-05/MANIFEST.md`**: subtraendo canonico Phase 2 docker. Template estrutural para os manifests Phase 3.
- **`docs/analysis/D011-final-baseline-analysis-2026-05-04.md`** §6 verdict: estado atual da decomposicao compute/network/merge — Phase 3 LAN preenche o componente que ainda nao foi medido.

## 11. Analise pos-campanha

Tres figuras entram no artigo do TCC (Secao 5 — Resultados):

1. **Decomposicao de overhead** — barras empilhadas por `(bench, W)`: `t_seq` / `t_local − t_seq` / `t_localhost − t_local` / `t_lan − t_localhost`.
2. **Teto de speedup em LAN** — linhas de speedup vs workers, uma por modo (`local`, `tcp_localhost`, `tcp_network`).
3. **RTT por rodada (strict BSP)** — `rounds` no eixo x, `t_round_lan / t_round_localhost` no eixo y.

Exemplo minimo de subtracao em Python (v2 — decomposicao componente-a-componente):

```python
import csv
def load(path):
    with open(path) as f:
        return {(r['benchmark'], r['input_size'], r['mode'], r['workers']): r
                for r in csv.DictReader(f)}

# Subtraendo: baseline Phase 2 docker (canonico v2)
local = load('results/locked/v2_post_d012_baseline_2026-05-05/summary.csv')

# Minuendo: campanha Phase 3 LAN (axis 1 ou axis 2)
lan   = load('results/locked/v2_lan_axis1_<data>/summary.csv')

print('benchmark,size,workers,wall_loc,wall_lan,d_wall,d_compute,d_network,d_merge')
for key, row in lan.items():
    if row['mode'] != 'tcp_network':
        continue
    loc = local.get((key[0], key[1], 'tcp_localhost', key[3]))
    if not loc:
        continue
    d_wall    = float(row['wall_clock_mean']) - float(loc['wall_clock_mean'])
    d_compute = float(row['compute_time_mean']) - float(loc['compute_time_mean'])
    d_network = float(row['network_time_mean']) - float(loc['network_time_mean'])
    d_merge   = float(row['merge_time_mean']) - float(loc['merge_time_mean'])
    print(f"{key[0]},{key[1]},{key[3]},"
          f"{loc['wall_clock_mean']},{row['wall_clock_mean']},"
          f"{d_wall:.4f},{d_compute:.4f},{d_network:.4f},{d_merge:.4f}")
```

Sanity checks:

- `d_compute ≈ 0` — reducao roda na CPU local; trocar localhost por LAN nao deveria mexer aqui. Drift > 10% sugere thermal envelope diferente entre maquinas.
- `d_merge ≈ 0` — merge tambem e CPU local no coordinator; mesmo argumento.
- `d_network > 0` e domina `d_wall` — esperado.
- `d_wall < 0` — bug. Investigue (cache effects, repos diferentes).
- `network_frac = (network_lan / wall_lan)` em `[0.5, 0.95]` para configs com particao grande; menor para bench micro como `condup_expansion 1k` (ver D-011 RF-02).

Para Axis 2 (zero-copy), a comparacao correlata e `axis2_lan vs axis1_lan` para isolar o ganho da feature: `d_compute` ficar negativo (rkyv elimina deserialize) com `d_network` levemente positivo (archive size > bincode) e o resultado esperado.
