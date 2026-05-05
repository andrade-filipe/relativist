# 1 — Getting Started

Este guia instala o `relativist` na sua maquina e explica os conceitos minimos de Interaction Combinators (IC) que o restante da trilha pressupoe.

## 1.1 Instalacao

Escolha **uma** das quatro opcoes. Todas entregam o mesmo binario final.

### Opcao 1 — Script de instalacao (Linux/macOS) — recomendada

```bash
curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

O script detecta seu OS/arquitetura, baixa o binario pre-compilado do GitHub Releases, verifica o checksum SHA256 e instala em `/usr/local/bin` (ou `~/.local/bin`).

Para instalar uma versao especifica:

```bash
VERSION=0.9.0 curl -sSfL https://raw.githubusercontent.com/andrade-filipe/relativist/main/scripts/install.sh | sh
```

### Opcao 2 — Docker

```bash
docker pull ghcr.io/andrade-filipe/relativist
docker run --rm ghcr.io/andrade-filipe/relativist --version
```

Guia completo de uso com volumes montados: [05 — Modo Distribuido TCP](05-distributed-tcp.md#docker).

### Opcao 3 — Download manual

Baixe o binario para seu sistema em <https://github.com/andrade-filipe/relativist/releases>:

- **Linux (recomendado Debian/Ubuntu):** `relativist-vX.Y.Z-x86_64.deb`
  ```bash
  sudo dpkg -i relativist-vX.Y.Z-x86_64.deb
  ```
- **Linux (alternativa):** `relativist-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz` — extrair e colocar no `PATH`.
- **Windows (recomendado):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.exe` — download direto, sem extrair.
- **Windows (alternativa):** `relativist-vX.Y.Z-x86_64-pc-windows-msvc.zip` — extrair o `.exe`.

> **Nota Windows (SmartScreen).** Como o executavel ainda nao possui assinatura digital, o Windows pode exibir "O Windows protegeu seu PC". Clique com botao direito no `.exe` → Propriedades → marque *Desbloquear* → OK. Ou, no dialogo, clique *Mais informacoes* → *Executar assim mesmo*. Isso e normal para executaveis sem code signing. Veja tambem [troubleshooting](../reference/troubleshooting.md#windows-smartscreen).

### Opcao 4 — Compilar do codigo fonte

Requer Rust 1.75+ (toolchain stable):

```bash
cargo install --git https://github.com/andrade-filipe/relativist
```

Ou para desenvolvimento local:

```bash
cd codigo/relativist
cargo build --release
# Binario em target/release/relativist (Linux/Mac)
# Binario em target\release\relativist.exe (Windows)
```

## 1.2 Verificar a instalacao

```bash
relativist --version
# relativist 0.9.0 (ou superior)

relativist --help
```

Se voce compilou do fonte e quer validar, rode os testes:

```bash
cargo test
# Esperado: 690+ passando
```

## 1.3 Conceitos basicos de Interaction Combinators

O Relativist trabalha com **redes IC** (Lafont, 1997). Uma rede IC e composta por:

- **Agentes** — nos com uma porta principal + ate duas auxiliares. Tres simbolos:

  | Simbolo | Nome                 | Portas                        |
  |---------|----------------------|-------------------------------|
  | γ (CON) | Constructor          | 1 principal + 2 auxiliares    |
  | δ (DUP) | Duplicator           | 1 principal + 2 auxiliares    |
  | ε (ERA) | Eraser               | 1 principal + 0 auxiliares    |

- **Wires** — conexoes entre portas.
- **Redex** — par de agentes conectados pelas **portas principais**. E candidato a reacao.
- **Normal form** — rede sem redexes (resultado final).

## 1.4 As 6 regras de interacao

| Regra    | Par   | Efeito                                     |
|----------|-------|--------------------------------------------|
| γ-γ      | mesmo | Aniquilacao cross-connect (4 fios)        |
| δ-δ      | mesmo | Aniquilacao parallel (4 fios)             |
| ε-ε      | mesmo | Void — ambos removidos                     |
| γ-δ      | diff  | Comutacao — cria 4 agentes novos          |
| γ-ε      | diff  | Erasure — cria 2 ERAs                     |
| δ-ε      | diff  | Erasure — cria 2 ERAs                     |

> **Propriedade-chave:** **strong confluence.** Se voce tem dois redexes disjuntos, pode reduzi-los em qualquer ordem (ou em paralelo) e chega ao mesmo resultado. Esta e a base teorica que permite distribuir a reducao (SPEC-00 e SPEC-01).

## 1.5 Perfis de carga de trabalho

Os benchmarks do Relativist sao organizados em tres perfis que aparecem repetidamente nos guias e resultados:

- **Profile A — Embarrassingly Parallel.** Todos os redexes sao independentes. Uma unica rodada basta.
- **Profile B — Expansion + Collapse.** γ-δ cria novos agentes antes de aniquila-los. Multiplas rodadas.
- **Profile C — Sequential Dependency.** Cascata nivel-a-nivel. Muitas rodadas, alto overhead de borda.

A campanha congelada do TCC (`v1_local_baseline`) usa os tres perfis — veja [benchmarks](../benchmarks/README.md).

---

**Proximo guia →** [02 — Primeira Reducao](02-first-reduction.md): gerar, inspecionar e reduzir sua primeira rede IC.
