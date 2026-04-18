# Guias do Relativist — Trilha de Aprendizado

Bem-vindo(a). Esta pasta organiza o caminho de aprendizado do Relativist em **sete guias curtos**, pensados para serem lidos em ordem. Cada guia cobre um conceito por vez, com exemplos executaveis e ponteiros para a especificacao formal.

## Antes de comecar

- **O que e o Relativist?** Um redutor distribuido de Interaction Combinators (Lafont 1997) escrito em Rust. Workers reduzem particoes localmente; um coordinator mescla bordas e itera ate a forma normal.
- **Para quem e?** Pesquisadores, estudantes de sistemas distribuidos, curiosos pelo HVM2/Bend, ou qualquer pessoa que queira entender como `strong confluence` habilita reducao deterministica em paralelo.
- **Pre-requisitos:** familiaridade basica com terminal (Bash/PowerShell). Nao e preciso saber Rust — voce vai usar binarios prontos.

## Trilha

| # | Guia | Voce aprende a... | Tempo |
|---|------|-------------------|-------|
| 1 | [Getting Started](01-getting-started.md) | Instalar o `relativist` (script, Docker, compilar) e entender os 3 simbolos + 6 regras de IC | 15 min |
| 2 | [Primeira Reducao](02-first-reduction.md) | Gerar uma rede (`generate`), inspeciona-la (`inspect`) e reduzi-la (`reduce`) | 10 min |
| 3 | [Grid Local](03-local-grid.md) | Simular a distribuicao em-processo com `local -w N` e entender o ciclo BSP | 15 min |
| 4 | [Aritmetica Church](04-church-arithmetic.md) | Codificar `add`/`mul` em IC via `compute`, com workers paralelos | 10 min |
| 5 | [Modo Distribuido TCP](05-distributed-tcp.md) | Subir `coordinator` + `worker` em maquinas (ou containers) diferentes | 20 min |
| 6 | [Protocolo Delta (v2)](06-delta-protocol.md) | Reduzir trafego de borda com `--delta-mode` (SPEC-19) | 10 min |
| 7 | [Bundle Zero-Copy (v2)](07-zero-copy.md) | Reduzir alocacoes no pipeline de merge com `--features zero-copy` (SPEC-18) | 5 min |

## Depois da trilha

- **[Referencia de CLI](../reference/cli.md)** — Toda flag, todo subcomando.
- **[Formatos de Arquivo](../reference/file-formats.md)** — `.bin` (bincode) e `.ic` (texto).
- **[Invariantes](../reference/invariants.md)** — G1, D3, D6 e o que eles garantem.
- **[Troubleshooting](../reference/troubleshooting.md)** — Windows, Docker, memoria, TCP.
- **[Benchmarks & Campanhas](../benchmarks/README.md)** — Metodologia e reproducao da baseline `v1_local_baseline`.

## Convencoes usadas nos guias

- **Exemplos em Bash.** No Windows, use Git Bash ou WSL2. Onde houver diferenca de sintaxe, a seccao marca como `# Windows (PowerShell)`.
- **Blocos fechados.** Cada comando e autocontido — copiar e colar funciona (desde que o `relativist` esteja no `PATH`).
- **Linhas curtas.** Nada de scripts gigantes inline; drivers grandes vivem em `scripts/` ou sao escritos em varias etapas.
- **Ponteiros para specs.** Quando um conceito precisa da formalizacao (teoremas, requisitos), o guia aponta para o arquivo `specs/SPEC-XX-*.md` correspondente — nao duplica o texto.

> **Idioma:** os guias estao em portugues-BR. As especificacoes (`specs/`) ficam em ingles para alinhar com a literatura academica.
