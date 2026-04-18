# 2 — Primeira Reducao

Neste guia voce gera uma rede IC, inspeciona seu conteudo e executa a reducao **sequencial** (baseline) ate a forma normal. Sao tres subcomandos: `generate`, `inspect` e `reduce`.

> Pre-requisito: `relativist` instalado (ver [01 — Getting Started](01-getting-started.md)).

## 2.1 `generate` — Gerar redes parametricas

O subcomando `generate` cria redes-exemplo em disco. Voce escolhe o tipo e o tamanho:

```bash
relativist generate <TIPO> -n <TAMANHO> -o <ARQUIVO>
```

### Tipos disponiveis

| Tipo                  | Perfil | Descricao                              |
|-----------------------|--------|----------------------------------------|
| `ep-annihilation`     | A      | `N` pares ERA-ERA (aniquilacao trivial)  |
| `ep-annihilation-con` | A      | `N` pares CON-CON (aniquilacao cross)    |
| `ep-annihilation-dup` | A      | `N` pares DUP-DUP (aniquilacao parallel) |
| `con-dup-expansion`   | B      | `N` pares CON-DUP (expansao + colapso)   |
| `dual-tree`           | C      | Duas arvores de profundidade `N`         |
| `mixed-rules`         | B      | `N` pares de cada uma das 6 regras       |
| `erasure-propagation` | C      | Cadeia de `N` CON com ERA na ponta       |
| `tree-sum`            | A/B    | Soma de `N` uns via Church add           |

### Exemplos

```bash
# 100 pares ERA-ERA em formato binario
relativist generate ep-annihilation -n 100 -o ep100.bin

# dual-tree de profundidade 6 em formato texto
relativist generate dual-tree -n 6 -o dual6.ic

# mixed-rules com 10 pares de cada tipo
relativist generate mixed-rules -n 10 -o mixed10.bin

# Cadeia de erasure com 50 CONs
relativist generate erasure-propagation -n 50 -o erasure50.bin

# Rede CON-DUP expansion (Profile B)
relativist generate con-dup-expansion -n 100 -o condup100.bin
```

### Formato texto (`.ic`)

O formato `.ic` e legivel por humanos — bom para depuracao. Exemplo com 3 pares ERA-ERA:

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

Detalhes completos de `.bin` e `.ic` em [reference/file-formats.md](../reference/file-formats.md).

## 2.2 `inspect` — Inspecionar uma rede

`inspect` mostra estatisticas sem modificar nada.

```bash
relativist inspect -i <ARQUIVO>
```

### Antes da reducao

```bash
relativist generate ep-annihilation -n 100 -o ep100.bin
relativist inspect -i ep100.bin
```

```
=== Relativist Inspect ===
Agents:  200
  CON: 0
  DUP: 0
  ERA: 200
Redexes: 100
Normal Form: no
```

### Apos a reducao

```bash
relativist reduce -i ep100.bin -o ep100_reduced.bin
relativist inspect -i ep100_reduced.bin
```

```
=== Relativist Inspect ===
Agents:  0
  CON: 0
  DUP: 0
  ERA: 0
Redexes: 0
Normal Form: yes
```

### Rede mista

```bash
relativist generate mixed-rules -n 5 -o mixed5.bin
relativist inspect -i mixed5.bin
```

```
=== Relativist Inspect ===
Agents:  60
  CON: 20
  DUP: 20
  ERA: 20
Redexes: 30
Normal Form: no
```

## 2.3 `reduce` — Reducao sequencial

`reduce` aplica `reduce_all` ate nao sobrar nenhum redex. Nao ha distribuicao nem paralelismo — e o **baseline** contra o qual as outras modalidades (grid local, TCP, delta) sao comparadas.

```bash
relativist reduce -i <ENTRADA> [-o <SAIDA>]
```

### Exemplos

```bash
# Reducao basica, sem saida persistida
relativist generate ep-annihilation -n 1000 -o ep1000.bin
relativist reduce -i ep1000.bin
```

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
# Cascata de erasure, gravando resultado e inspecionando
relativist generate erasure-propagation -n 20 -o erasure20.bin
relativist reduce -i erasure20.bin -o erasure20_reduced.bin
relativist inspect -i erasure20_reduced.bin
```

## 2.4 Pipeline completa (smoke test)

Um exemplo integrado — util quando voce quer confirmar que a instalacao funciona de ponta a ponta:

```bash
# 1. Gerar rede mixed-rules
relativist generate mixed-rules -n 20 -o mixed20.bin

# 2. Inspecionar original
relativist inspect -i mixed20.bin
# Agents: 240, Redexes: 120

# 3. Reduzir sequencialmente
relativist reduce -i mixed20.bin -o mixed20_seq.bin

# 4. Inspecionar o resultado
relativist inspect -i mixed20_seq.bin
# Agents: 80 (todas ERA), Redexes: 0, Normal Form: yes
```

---

**Proximo guia →** [03 — Grid Local](03-local-grid.md): execute o mesmo `reduce` em paralelo simulado com `local -w N`.
