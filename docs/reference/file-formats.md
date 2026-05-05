# Formatos de Arquivo

O Relativist le e escreve redes IC em dois formatos. Ambos carregam a mesma informacao — a diferenca e eficiencia versus legibilidade.

## `.bin` — Binario (bincode)

Formato compacto baseado em `bincode` v1. Ideal para redes grandes, transporte TCP e reprodutibilidade.

```bash
relativist generate ep-annihilation -n 10000 -o net.bin
```

- **Codificacao:** `bincode` v1 (little-endian, sem schema incorporado).
- **Tipo:** `relativist_core::net::Net` serializado.
- **Pros:** Ordem de magnitude menor que `.ic`; leitura O(N).
- **Contras:** Nao inspecionavel com `cat`.

> Existe uma variante **compacta** para transporte — `CompactSubnet` em `src/partition/compact.rs`. Ela serializa apenas agentes vivos, reduzindo overhead de arenas densas. A rota de disco usa `Net` completo; apenas particoes TCP usam `CompactSubnet`.

## `.ic` — Texto

Formato texto legivel por humanos. Util para depurar redes pequenas, testar geracao e discutir exemplos em papers.

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

- **`<ID>`:** inteiro nao-negativo, unico por agente.
- **`<SYMBOL>`:** `CON`, `DUP` ou `ERA`.
- **`<PORT>`:** `principal`, `left` (aux1) ou `right` (aux2).
- **`free<N>`:** porta livre (sem outro agente conectado); numerada a partir de 0.

### Exemplo completo

Rede com 3 pares ERA-ERA:

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

Cada par `(a0,a1)`, `(a2,a3)`, `(a4,a5)` esta conectado pelas **portas principais** → tres redexes.

## Quando usar cada formato

| Situacao                                       | Formato |
|------------------------------------------------|---------|
| Benchmarks, reducao, transporte TCP            | `.bin`  |
| Depurar geracao de testes                      | `.ic`   |
| Exemplos didaticos em paper, slides, issue     | `.ic`   |
| Campanhas com centenas de milhoes de agentes   | `.bin`  |

## Conversao entre formatos

`relativist generate ... -o <nome>.<ext>` escolhe automaticamente pelo sufixo. Para reduzir uma rede `.ic`:

```bash
relativist generate ep-annihilation -n 5 -o net.ic
relativist reduce -i net.ic -o net_reduced.bin
```

Os subcomandos do core (`reduce`, `local`, `inspect`, `coordinator`, `worker`) aceitam qualquer um dos dois formatos como entrada.

## PROTOCOL_VERSION — historico e wire compatibility

A constante `PROTOCOL_VERSION` rege o handshake de wire entre coordinator e worker (SPEC-06). Workers e coordinator **precisam** ter a mesma versao; mismatch e rejeitado no `Register` com `RegisterNack { reason: ProtocolVersionMismatch }` (SPEC-19 R37, SPEC-20 R0d).

| Version | Spec driver                  | O que mudou                                                                                                                  |
|---------|------------------------------|-------------------------------------------------------------------------------------------------------------------------------|
| 1       | SPEC-06 v1                   | Wire v1 inicial: bincode v1 fixed-int, frame header v1, sem compressao.                                                       |
| 2       | [SPEC-18](../../specs/SPEC-18-wire-format-v2.md) | Wire format v2: bincode v2 (varint), `PortRef` compacto (2-5 bytes), LZ4 threshold (>1 MB), frame header v2 com flags rkyv/lz4. Break intencional. |
| 3       | [SPEC-22 §3.1 R9a](../../specs/SPEC-22-arena-management.md) | `Net.free_list: Vec<AgentId>` adicionado ao layout serializado (D-009). v2 deserializers rejeitam v3 nets com `UnsupportedVersion`. |
| 4       | [SPEC-19](../../specs/SPEC-19-delta-protocol.md) | Wave delta protocol: novas variantes `Message` (`InitialPartition`, `RoundStart`, `RoundResult`, `FinalStateRequest`, `FinalStateResult`, `BorderState`). |
| 5       | [SPEC-21 §3.7 R37c / §3.8 A2](../../specs/SPEC-21-streaming-generation.md) | Streaming generation: `Message` ganha `RequestWork { worker_id }` e `NoMoreWork` (pull dispatch). |
| 6       | (proxima onda v2)            | Reservado para SPEC-20 elastic-grid wire variants (`JoinRequest`/`JoinAck`/`LeaveRequest`/`LeaveAck`) quando o ramo TCP for travado em release.                          |

**Sequencing rules.** SPEC-21 R37c documenta a politica de bumps: cada spec que toca o `Message` enum incrementa `PROTOCOL_VERSION`, e o predecessor REJEITA wire payloads com a versao posterior (`UnsupportedVersion`) — nada de silencioso reinterpretation de tags. Persisted `.bin` files de baselines congelados (ex.: `results/locked/v1_local_baseline/*.bin`) ficam **ilegiveis** por binarios de versoes superiores; isso e aceitavel porque baselines congelados nao alimentam codigo posterior — a regeneracao via `relativist generate` produz arquivos com o schema corrente.

## Especificacao formal

- Schema binario: `src/io/binary.rs`
- Schema texto: `src/io/text.rs`
- Requisitos: `specs/SPEC-12-user-io.md`
- Wire format v2 (PORT_VERSION 2): `specs/SPEC-18-wire-format-v2.md`
- Free-list serde (PROTOCOL_VERSION 3, R9a): `specs/SPEC-22-arena-management.md` §3.1
- Delta protocol Message variants (PROTOCOL_VERSION 4): `specs/SPEC-19-delta-protocol.md`
- Streaming RequestWork/NoMoreWork (PROTOCOL_VERSION 5, R37c): `specs/SPEC-21-streaming-generation.md` §3.7
