---
title: Distributed grid over TCP
summary: Run a real coordinator plus workers over TCP (loopback, Docker, or LAN), set the bind address, and enable token auth.
keywords: [distributed, tcp, coordinator, worker, bind, loopback, docker, lan, token, auth, security, bsp, protocol, retry, metrics]
modules: [protocol, merge]
specs: [SPEC-06, SPEC-10, SPEC-05]
audience: [user, llm]
status: guide
updated: 2026-06-26
---

# Distributed grid over TCP

This guide runs the BSP cycle over **real TCP**, with the coordinator and
workers as separate processes. The mechanism is identical to
[local-grid.md](local-grid.md) — only process launch and the network hop
differ. Three scenarios share the same commands: loopback (`127.0.0.1`), Docker
containers on one host, and separate machines on a LAN.

For the BSP model see [../architecture/overview.md](../architecture/overview.md).

## mode-selection

| Mode | Command | When |
|------|---------|------|
| Sequential | `reduce` | Baseline, small net. |
| Local (in-process) | `local -w N` | Simulate the grid, no TCP. |
| TCP loopback | `coordinator` + `worker` | Exercise the TCP protocol locally. |
| TCP Docker | `docker compose up` | Isolate processes in containers. |
| TCP LAN | `coordinator` + `worker` on different hosts | Real distributed grid. |

## coordinator

The master node: load the net, partition it, distribute to workers, merge the
result. It **blocks** until all N workers connect, then starts round 1.

```bash
relativist coordinator -w <N> -b <HOST:PORT> -i <INPUT> [-o <OUTPUT>] [-m <METRICS>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-w, --workers <N>` | required (`>=1`) | Workers to wait for. |
| `-i, --input <PATH>` | required | Input net (`.bin`). |
| `-b, --bind <ADDR>` | `127.0.0.1:9000` | Bind address; `IP:PORT`, `HOST:PORT`, or `tailscale[:PORT]`. |
| `-o, --output <PATH>` | none | Write the reduced net. |
| `-m, --metrics <PATH>` | none | Write metrics (`.json`/`.csv`). |
| `--max-rounds <N>` | unlimited | Cap BSP rounds. |
| `--strict-bsp` | `false` | One genuine round per drain (see local-grid). |
| `--token <auto\|BASE64>` | none | Auth token (see below). |
| `--token-file <PATH>` | `./relativist-token` | Where `auto` writes the token. |
| `--log-format <text\|json>` | TTY auto | Log output format. |

Bind to `127.0.0.1` for loopback only; bind to `0.0.0.0` to accept LAN/Docker
workers. Full flag list (transport, TLS, delta, elastic):
[../reference/cli.md](../reference/cli.md).

## worker

The compute node: connect, receive partitions, reduce, return the result. It
retries the connection with exponential backoff (~30s).

```bash
relativist worker -c <HOST:PORT> [--token <BASE64>]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-c, --coordinator <HOST:PORT>` | required | Coordinator address. |
| `--token <BASE64>` | none | Auth token; must match the coordinator. |
| `--daemon` | `false` | Reconnect after each job (SPEC-16). |
| `--log-format <text\|json>` | TTY auto | Log output format. |

## loopback-one-coordinator-two-workers

Open **3 terminals** (or background with `&`).

Terminal 1 (coordinator):

```bash
relativist generate ep-annihilation-con -n 10000 -o ep10k.bin
relativist coordinator -w 2 -b 127.0.0.1:9000 \
  -i ep10k.bin -o ep10k_grid.bin -m ep10k_metrics.json
```

Terminal 2 and Terminal 3 (one worker each):

```bash
relativist worker -c 127.0.0.1:9000
```

Once both workers connect, the coordinator runs the grid, prints the execution
summary, and all processes exit. `ep10k_metrics.json` holds per-round metrics
(bytes sent/received, compute, merge, network).

## bind-address

- **Loopback:** `-b 127.0.0.1:9000`. Only same-host workers connect.
- **LAN / Docker:** `-b 0.0.0.0:9000`. Workers reach the coordinator at its
  routable IP (e.g. `-c 192.168.1.100:9000`). Binding `0.0.0.0` **without** a
  token prints a security warning.

## token-auth

Token auth (SPEC-10) gates worker connections. Have the coordinator generate
one and persist it to a file:

```bash
relativist coordinator -w 2 -b 0.0.0.0:9000 \
  --token auto --token-file /tmp/rel-token -i input.bin
```

The base64 token is printed to stdout and written to `/tmp/rel-token`. Workers
pass the same token:

```bash
TOKEN=$(cat /tmp/rel-token)
relativist worker -c coord-host:9000 --token "$TOKEN"
```

For the full three-tier security model and TLS, see
[../operations/security-observability.md](../operations/security-observability.md).

## next-steps

- **Docker / docker-compose** (build the image, mount volumes, scale workers):
  [../operations/docker.md](../operations/docker.md).
- **Security and observability** (token tiers, TLS, metrics, tracing):
  [../operations/security-observability.md](../operations/security-observability.md).
- **High-level arithmetic over the grid:**
  [church-arithmetic.md](church-arithmetic.md).

---

**Next ->** [church-arithmetic.md](church-arithmetic.md): `add`/`mul`/`exp`
encoded in IC.
