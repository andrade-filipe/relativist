# 07 · Zero-Copy Wire Format (`--features zero-copy`)

Guia para o **wire format v2 com zero-copy**, especificado em [SPEC-18](../../specs/SPEC-18-wire-format-v2.md). SPEC-18 substitui bincode v1 (fixed-int) por bincode v2 (varint), adiciona compressao LZ4 opcional e oferece um caminho **zero-copy** via rkyv para hot-path messages (`AssignPartition`, `PartitionResult`).

> **Status:** feature-gated. `--features zero-copy` e **opt-in** — nao e default. Beneficio pratico so aparece quando o custo de deserializacao domina (particoes grandes, rede rapida, CPU do receptor limitada).

## 1. Quando usar

Ative zero-copy quando:

- Particoes sao **grandes** (>1 MB) e deserializacao aparece como hotspot no `perf` do worker/coordinator.
- Rede e **rapida** (10 GbE, loopback) e CPU do receptor vira o gargalo.
- Voce esta medindo Phase 3 LAN e quer `t_network = t_lan - t_localhost` mais nitido (sem contaminar com variacao de deserialize).

Se a particao inteira cabe em poucos KB, o custo de deserializacao e negligenciavel e a feature nao paga o overhead de manter duas pilhas de derives.

## 2. Ideia central

### Fluxo padrao (bincode v2)

```
Worker -> Coord:
  struct Partition { ... }
    -> bincode::serialize -> bytes
    -> LZ4 compress (opcional, >1 MB threshold)
    -> TCP send

Coord recv:
    -> TCP recv -> bytes
    -> LZ4 decompress
    -> bincode::deserialize -> struct Partition { ... }  (ALLOCATION + COPY)
```

### Fluxo zero-copy (rkyv)

```
Worker -> Coord:
  struct Partition (com derives rkyv::Archive, Serialize)
    -> rkyv::to_bytes -> bytes (layout archivado)
    -> LZ4 compress (opcional)
    -> TCP send  [flag header: rkyv=1]

Coord recv:
    -> TCP recv -> bytes
    -> LZ4 decompress
    -> rkyv::archived_root::<Partition>(&bytes)   <-- ACESSA DIRETO, sem copy/alloc
```

O receptor acessa campos do archive **em cima do buffer recebido**. Nao ha passe de deserializacao; ha apenas um ponteiro e alguma aritmetica de offsets.

## 3. Como ativar

### Build com a feature

```bash
# Build release com zero-copy habilitado
cargo build --release --features zero-copy

# Rodar tests com a feature (sanity check)
cargo test --features zero-copy
```

### Feature combinada

```bash
# Zero-copy + TLS + observabilidade
cargo build --release --features zero-copy,tls,observability
```

### Config em runtime

A feature e compile-time. Em runtime, o emissor decide se usa o caminho rkyv via `GridConfig.zero_copy_hot_path` (bool). Se o emissor decidir mandar rkyv, o receptor **precisa** ter sido compilado com a mesma feature — o handshake rejeita mismatch.

```rust
// src/config.rs (sketch)
pub struct GridConfig {
    /// Use rkyv zero-copy path for AssignPartition and PartitionResult.
    /// Only effective when the crate was built with `--features zero-copy`.
    pub zero_copy_hot_path: bool,
    // ...
}
```

## 4. O que muda no wire

### Frame header v2

| Campo         | Bits | Significado                                 |
|---------------|------|---------------------------------------------|
| magic         | 32   | `"RELV"` (sanity check)                     |
| version       | 8    | 2                                           |
| flags         | 8    | bit 0 = LZ4 comprimido; bit 1 = rkyv archived |
| payload_len   | 32   | bytes uteis pos-compressao                  |
| payload_crc32 | 32   | CRC do payload (pre-decompressao)           |
| payload       | ...  | bincode v2 **ou** rkyv archive              |

Bit 1 de `flags` distingue bincode (0) de rkyv (1). Isso permite ter workers legacy + novos na mesma malha, desde que cada lado saiba ler ambos (o emissor escolhe, o receptor adapta).

### PortRef compacto

SPEC-18 troca `PortRef` de fixed 8 bytes para varint 2-5 bytes. Isso e ortogonal ao zero-copy — aplica-se tambem ao caminho bincode v2 puro.

### LZ4 threshold

Payloads >1 MB passam por LZ4 por padrao (bit 0 de flags). Payloads pequenos nao sao comprimidos (o overhead de header da LZ4 anularia o ganho).

## 5. Beneficios medidos

Referencia (ordem de grandeza, documentada no SPEC-18):

| Cenario                                | Deserialize CPU | Wire size |
|----------------------------------------|-----------------|-----------|
| bincode v1 (v1 default)                | baseline        | baseline  |
| bincode v2 + CompactSubnet             | ~1.0x           | 0.6x      |
| bincode v2 + CompactSubnet + LZ4       | ~1.1x (lz4 cpu) | 0.3x      |
| rkyv (--features zero-copy)            | **~0.0x**       | ~1.2x     |
| rkyv + LZ4                             | **~0.1x**       | 0.5x      |

"Zero deserialize CPU" e o ganho caracteristico do rkyv. O trade e wire size um pouco maior (o archive carrega offsets/metadata explicitos), por isso LZ4 em cima fecha o gap.

## 6. CompactSubnet + rkyv (nota)

`CompactSubnet` (SPEC-04) e um adapter serde que serializa apenas agentes vivos. Sob o caminho rkyv, `CompactSubnet` **nao** e usado — rkyv serializa o `Partition` direto (Net completo com arena + array de portas).

Isso e aceitavel: o custo que `CompactSubnet` evita (deserialize + realloc da arena densa) **ja e zero** no caminho rkyv. O archive pode ser maior, mas LZ4 compensa.

## 7. Limitacoes

- **Build-time feature.** Binario sem `--features zero-copy` nao consegue ler archives rkyv. Upgrade de uma malha existente requer rolling restart com binarios novos.
- **Derive doubled.** Tipos tocados (Net, Partition, CompactSubnet, Agent, Symbol, PortRef, IdRange, WorkerRoundStats) carregam derives de ambos serde e rkyv. Mudancas de schema precisam manter os dois consistentes.
- **Sem transparent fallback.** Se `zero_copy_hot_path=true` mas o peer nao tem a feature, o handshake falha cedo. Nao ha downgrade silencioso.
- **Alinhamento.** rkyv exige buffer alinhado a 16 bytes para alguns tipos. O receiver path em `src/protocol/frame.rs` cuida disso via `AlignedVec`, mas custom integrations precisam respeitar.

## 8. Proximo passo

- [SPEC-18](../../specs/SPEC-18-wire-format-v2.md) — spec completa com bincode v2, PortRef varint, LZ4, rkyv archive, frame header v2.
- [06-delta-protocol.md](06-delta-protocol.md) — o modo delta reduz o **numero** de mensagens; zero-copy reduz o **custo por mensagem**. As duas features sao complementares.
