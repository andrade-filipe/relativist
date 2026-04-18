# Phase 3 — TcpNetwork (maquinas reais)

Phase 3 executa o mesmo protocolo BSP em **maquinas fisicas diferentes** conectadas por rede Ethernet real. Expoe o custo que nenhum outro modo consegue reproduzir: latencia RTT, throughput limitado, jitter e contencao de NIC. E a campanha que transforma Relativist de "implementacao correta de ICs distribuidos" em "evidencia empirica sobre o custo real de distribuir reducao de ICs num grid".

> **Pre-requisito forte:** Phase 3 so faz sentido depois que `v1_local_baseline` (tag `v0.10.0-bench`) esta congelada em `results/locked/v1_local_baseline/`. A medida principal e a subtracao `t_network = t_lan - t_localhost`, e ela exige que Phase 2 Docker ja tenha produzido `phase2_summary.csv` **no mesmo binario**. Nao comece Phase 3 sem ter Phase 2 travada.

## 1. O que Phase 3 mede

### Medida primaria — overhead de LAN por subtracao

Para cada triple `(bench, size, workers)`:

```
t_network = t_lan  -  t_localhost
            ^^^^^^    ^^^^^^^^^^^^
            aqui      v1_local_baseline/phase2_summary.csv
```

`t_localhost` ja esta congelado em `v1_local_baseline`. Phase 3 produz `t_lan`. A diferenca e a fracao de wall-clock atribuida a latencia de fio + banda + jitter — tudo que Docker em loopback nao consegue reproduzir. E o numero que o artigo do TCC reporta como **"custo de rede do grid"**.

Para a subtracao valer: *tudo* tem de ser identico menos a rede. Mesmo binario (tag `v0.10.0-bench`), mesmos bytes de input, mesma estrategia de particao, mesmas 10 repeticoes, mesmo modo BSP (lenient por padrao). Qualquer drift invalida o resultado.

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

### 1. Build host (uma vez)

```bash
git clone git@github.com:andrade-filipe/relativist.git
cd relativist
git checkout v0.10.0-bench

rustup override set 1.94.1 || rustup install 1.94.1
cargo build --release

./target/release/relativist --version
sha256sum target/release/relativist
```

Guarde esse sha256 — ele entra no manifest da Phase 3.

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

## 5. Orquestracao, coleta, analise

Por volume, o texto detalhado da orquestracao (script SSH de ~400 linhas, Ansible alternativo, schema `phase3_*.csv`, CV triage, pos-campanha) fica versionado diretamente em:

- **Driver shell:** `scripts/bench_phase3_locked.sh` (a escrever).
- **Aggregator:** `scripts/aggregate_phase3.py` (a escrever — analogo do parser de metrics em `bench_phase2_locked.sh`).
- **Schema de saida:** deve bater byte-por-byte com `phase2_summary.csv` (mesmo header, mesma ordem, mesma precisao numerica).

O esqueleto do `run_one()` bash e a matriz `BENCH_SIZES`/`WORKER_COUNTS`/`REPS=10`, assim como o ciclo de validacao por run (worker spawn via ssh → coordinator → coleta de logs → inspect contra referencia), estao arquivados em `docs/PHASE3-FINDINGS.md` apos a campanha rodar.

## 6. Manifest do snapshot

Apos campanha + CV triage, crie `results/locked/v1_lan_baseline/manifest.md` com a mesma estrutura do `v1_local_baseline/manifest.md`:

```markdown
# v1_lan_baseline — Campaign Manifest

**Status:** COMPLETE — Phase 3 LAN campaign finished on <data>.

## Provenance
- Git tag: v0.10.0-bench
- Commit SHA: <SHA>
- Binary sha256: <hash>
- Operator: Filipe Andrade Nascimento
- Campaign start/end: <timestamps>

## Cluster
- Switch: <modelo>
- Coordinator: <CPU, RAM, OS, NIC>
- Workers: idem
- Network: 1 Gbps, VLAN unica
- RTT baseline: <ping min/avg/max>
- Banda baseline: <iperf3 Gbps>

## Campaign knobs
- Bench x size: 8 combos
- Workers: {1,2,4,8}
- Repetitions: 10
- Mode: tcp_network

## Checksums (sha256) — CSVs finais
- phase3_detail.csv: <hash>
- phase3_summary.csv: <hash>
- phase3_rounds.csv: <hash>

## Relationship to v1_local_baseline
- Phase 3 LAN subtrai Phase 2 Docker (t_localhost) para extrair t_network.
```

Congele com commit atomico + tag nova (`v0.11.0-lan`). Nao mova `v0.10.0-bench`.

## 7. Riscos conhecidos

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

## 8. Referencias cruzadas

- **SPEC-09** (`specs/SPEC-09-benchmarks.md`): R25 modos, R27 TcpNetwork MUST, R31 10 reps.
- **SPEC-07** (`specs/SPEC-07-deployment.md`): R41 bare-metal procedure.
- **SPEC-05** (`specs/SPEC-05-merge.md`): R30a lenient vs strict BSP.
- **SPEC-06** (`specs/SPEC-06-wire-protocol.md`): wire format, handshake, token.
- **SPEC-10** (`specs/SPEC-10-security.md`): modelo de auth de 3 niveis.
- **PHASE1-FINDINGS.md** (este repo): item L2, fix arquitetural do strict BSP.
- **PHASE2-FINDINGS.md** (este repo): snapshot Phase 2 que Phase 3 subtrai.
- **`results/locked/v1_local_baseline/manifest.md`**: template de manifest.

## 9. Analise pos-campanha

Tres figuras entram no artigo do TCC (Secao 5 — Resultados):

1. **Decomposicao de overhead** — barras empilhadas por `(bench, W)`: `t_seq` / `t_local − t_seq` / `t_localhost − t_local` / `t_lan − t_localhost`.
2. **Teto de speedup em LAN** — linhas de speedup vs workers, uma por modo (`local`, `tcp_localhost`, `tcp_network`).
3. **RTT por rodada (strict BSP)** — `rounds` no eixo x, `t_round_lan / t_round_localhost` no eixo y.

Exemplo minimo de subtracao em Python:

```python
import csv
def load(path):
    with open(path) as f:
        return {(r['benchmark'], r['input_size'], r['mode'], r['workers']): r
                for r in csv.DictReader(f)}

local = load('v1_local_baseline/phase2_summary.csv')
lan   = load('v1_lan_baseline/phase3_summary.csv')

for key, row in lan.items():
    if row['mode'] != 'tcp_network':
        continue
    loc = local.get((key[0], key[1], 'tcp_localhost', key[3]))
    if not loc:
        continue
    t_loc = float(loc['wall_clock_mean'])
    t_lan = float(row['wall_clock_mean'])
    frac = (t_lan - t_loc) / t_lan if t_lan > 0 else 0
    print(f"{key[0]},{key[1]},{key[3]},{t_loc:.4f},{t_lan:.4f},{t_lan-t_loc:.4f},{frac:.3f}")
```

`net_frac ∈ [0.0, 0.5]` para a maioria dos configs. Negativo = bug.
