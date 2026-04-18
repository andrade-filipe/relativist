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

## Especificacao formal

- Schema binario: `src/io/binary.rs`
- Schema texto: `src/io/text.rs`
- Requisitos: `specs/SPEC-12-user-io.md`
