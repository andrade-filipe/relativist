# SPEC-12: User I/O & Examples

**Status:** Draft v1
**Depends on:** SPEC-00 (Glossary), SPEC-02 (Net Representation), SPEC-07 (Deployment), SPEC-09 (Benchmarks), SPEC-13 (System Architecture)
**Gray zones resolved:** ---
**Research consumed:** PESQ-002 (Apache Ignite architecture, CLI patterns), PESQ-024 (Architecture Recommendations, Section 7 CLI Design, Section 4 Data Flow)
**Discussions consumed:** ---
**Arguments consumed:** ---
**Code analyses consumed:** AC-005 (Haskell benchmark framework, BenchDef generators: mkEPNet, mkTree, mkTreeBalanced, mkExpansionNet, mkDualTreeNet)

---

## 1. Purpose

This spec defines the user-facing I/O layer of Relativist: the input formats for IC nets (binary, text DSL, JSON), the output formats for reduced nets and execution metrics, the CLI subcommands for local reduction (`reduce`), net inspection (`inspect`), and net generation (`generate`), the pre-built example net generators available through the CLI, and the human-readable output summary format. This is the spec that makes Relativist usable as a standalone tool independent of the benchmark suite (SPEC-09) and the distributed grid (SPEC-05/SPEC-06). The `reduce` and `inspect` subcommands enable testing and debugging of the core reduction engine (SPEC-03) without any network infrastructure.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Binary Format (.bin)** | The primary serialization format for IC nets: serde + bincode encoding of the `Net` struct (SPEC-02). Fastest to parse, most compact, used by benchmarks and programmatic workflows. File extension: `.bin`. |
| **Text DSL (.ic)** | A human-readable text format for defining IC nets by hand. One agent per line, wires defined by named port references. File extension: `.ic`. Designed for small examples, documentation, and debugging. Not intended for large nets. |
| **JSON Format (.json)** | serde JSON serialization of the `Net` struct. Verbose but interoperable with external tools, visualization pipelines, and web-based consumers. File extension: `.json`. |
| **Input Format** | Any of the three supported formats (binary, text DSL, JSON) from which a `Net` can be loaded. The format is auto-detected by file extension or overridden by a `--format` flag. |
| **Output Format** | The format in which a reduced `Net` is written. Defaults to the same format as the input. Overridable via `--format` on the output path. |
| **Net Summary** | A structured report of a net's key statistics: agent count, wire count, redex count, agent type distribution (CON/DUP/ERA), free port count, and whether the net is in Normal Form. Produced by the `inspect` subcommand and printed after reduction. |
| **Reduction Summary** | A human-readable report printed to stdout after any reduction (local or distributed). Contains input/output statistics, timing, interaction count, MIPS, and optional grid metrics (rounds, workers, speedup). |
| **Example Net** | A pre-built parametric net generator accessible via the `generate` subcommand. Each example corresponds to a benchmark profile from SPEC-09 but is exposed as a first-class CLI feature for ad-hoc use. |
| **Normal Form** | A net with zero redexes remaining (SPEC-02, R16; SPEC-03). The reduction summary reports whether the output net has reached Normal Form. |

---

## 3. Requirements

### 3.1 Input Formats

**R1.** Relativist MUST support loading IC nets from three input formats: Binary (.bin), Text DSL (.ic), and JSON (.json). **(MUST)**

#### 3.1.1 Binary Format (.bin) -- Primary

**R2.** The binary format MUST use serde + bincode (v2) serialization of the `Net` struct as defined in SPEC-02. **(MUST)**

**R3.** The binary format MUST use the file extension `.bin`. **(MUST)**

**R4.** Binary deserialization MUST produce a `Net` that satisfies the roundtrip identity: `deserialize(serialize(net)) == net` (structural equality, cf. SPEC-02, R26). **(MUST)**

**R5.** The binary format SHOULD be the default for benchmarks, programmatic input, and the `generate` subcommand output, as it is the fastest to parse and most compact. **(SHOULD)**

#### 3.1.2 Text DSL (.ic) -- Human-Readable

**R6.** The text DSL MUST use the file extension `.ic`. **(MUST)**

**R7.** The text DSL grammar MUST follow this specification (in pseudo-BNF). **(MUST)**

```
file        ::= line*
line        ::= comment | agent_decl | wire_decl | root_decl | blank
comment     ::= '#' <any chars until newline>
blank       ::= <whitespace only>

agent_decl  ::= 'agent' IDENT SYMBOL
SYMBOL      ::= 'CON' | 'DUP' | 'ERA'
IDENT       ::= [a-zA-Z_][a-zA-Z0-9_]*

wire_decl   ::= 'wire' port_ref port_ref
port_ref    ::= agent_port | free_port
agent_port  ::= IDENT '.' PORT_NAME
PORT_NAME   ::= 'principal' | 'left' | 'right' | 'p0' | 'p1' | 'p2'
free_port   ::= 'free(' INT ')'

root_decl   ::= 'root' port_ref

INT         ::= [0-9]+
```

**R8.** The parser MUST map port names to `PortId` values: `principal` / `p0` -> 0, `left` / `p1` -> 1, `right` / `p2` -> 2. **(MUST)**

**R9.** The parser MUST reject ERA agents with auxiliary port references (`left`, `right`, `p1`, `p2`), since ERA has arity 0 (SPEC-02, Section 4.2). **(MUST)**

**R10.** The parser MUST assign `AgentId` values sequentially in declaration order (first `agent` declaration gets ID 0, second gets ID 1, etc.). **(MUST)**

**R11.** The parser MUST validate the parsed net against invariants T1 (port linearity) and I2 (reference validity) from SPEC-01. If validation fails, the parser MUST return a descriptive error with the line number of the offending declaration. **(MUST)**

**R12.** The text DSL MUST support comments (lines starting with `#`) and blank lines for readability. **(MUST)**

**R13.** An example text DSL file for a CON-CON annihilation pair. **(Informative)**

```
# CON-CON annihilation: two constructors connected at principal ports.
# Auxiliary ports are connected cross-wise.
# After reduction: 0 agents, free ports reconnected.
agent a CON
agent b CON
wire a.principal b.principal
wire a.left b.left
wire a.right b.right
```

**R14.** An example text DSL file for a CON-DUP commutation pair. **(Informative)**

```
# CON-DUP commutation: constructor meets duplicator.
# After reduction: 4 new agents (2 CON + 2 DUP), cross-connected.
agent c CON
agent d DUP
wire c.principal d.principal
wire c.left free(0)
wire c.right free(1)
wire d.left free(2)
wire d.right free(3)
```

**R15.** The text DSL serializer (write direction) MUST produce output that, when re-parsed, yields a structurally equivalent `Net`. **(MUST)**

#### 3.1.3 JSON Format (.json) -- Interop

**R16.** The JSON format MUST use serde JSON serialization of the `Net` struct. File extension: `.json`. **(MUST)**

**R17.** JSON support MAY be deferred to a post-v1 release if development time is constrained. If deferred, the CLI MUST print a clear error message: `"JSON format not yet supported; use .bin or .ic"`. **(MAY for implementation; MUST for error message if absent)**

### 3.2 Output Formats

**R18.** The `reduce` subcommand MUST write the reduced net to the path specified by `--output`, in the format determined by the output file's extension (or the `--format` flag). **(MUST)**

**R19.** If `--output` is not specified, the `reduce` subcommand MUST NOT write a net file; it MUST only print the Reduction Summary to stdout. **(MUST)**

**R20.** After any reduction (local via `reduce`, or distributed via `coordinator`), Relativist MUST print a human-readable Reduction Summary to stdout (see Section 3.6). **(MUST)**

**R21.** The `reduce` subcommand SHOULD support a `--metrics` flag that writes a JSON metrics object to a file. **(SHOULD)**

The metrics JSON object MUST contain at minimum:

```json
{
  "agents_before": 1000,
  "agents_after": 42,
  "wires_before": 1500,
  "wires_after": 63,
  "redexes_before": 500,
  "redexes_after": 0,
  "normal_form": true,
  "total_interactions": 958,
  "duration_secs": 1.234,
  "mips": 0.776
}
```

**R22.** For distributed execution (coordinator mode), the coordinator SHOULD additionally write a per-round CSV file if `--round-csv <PATH>` is specified. **(SHOULD)**

The CSV schema MUST be:

```
round,duration_ms,local_redexes,border_redexes,interactions,agents_before,agents_after
```

### 3.3 `relativist reduce` Subcommand

**R23.** The `reduce` subcommand MUST perform local reduction using `reduce_all()` from the reduction module (SPEC-03) without any network communication. **(MUST)**

**R24.** The `reduce` subcommand MUST accept the following arguments (cf. SPEC-13, R46). **(MUST)**

```rust
/// Run local reduction on an IC net (no grid, no network).
#[derive(Debug, clap::Args)]
pub struct ReduceArgs {
    /// Path to the input net file (.bin, .ic, or .json).
    #[arg(long)]
    pub input: PathBuf,

    /// Path to write the reduced net (format inferred from extension).
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Override input format detection.
    #[arg(long, value_enum)]
    pub format: Option<NetFormat>,

    /// Path to write metrics JSON.
    #[arg(long)]
    pub metrics: Option<PathBuf>,

    /// Maximum interactions before stopping (safety limit).
    #[arg(long)]
    pub max_interactions: Option<u64>,
}
```

**R25.** The `reduce` subcommand MUST print a Reduction Summary to stdout upon completion (see R35). **(MUST)**

**R26.** If `--max-interactions` is specified and the limit is reached before Normal Form, the subcommand MUST print a warning and write the partially-reduced net (if `--output` is specified). **(MUST)**

### 3.4 `relativist inspect` Subcommand

**R27.** The `inspect` subcommand MUST load a net file and print its statistics without modifying it (cf. SPEC-13, R47). **(MUST)**

**R28.** The `inspect` subcommand MUST accept the following arguments. **(MUST)**

```rust
/// Inspect an IC net file and print summary statistics.
#[derive(Debug, clap::Args)]
pub struct InspectArgs {
    /// Path to the net file (.bin, .ic, or .json).
    pub path: PathBuf,

    /// Output format for the summary.
    #[arg(long, value_enum, default_value = "text")]
    pub format: InspectOutputFormat,
}

/// Output format for the inspect summary.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum InspectOutputFormat {
    /// Human-readable text (default).
    Text,
    /// Machine-readable JSON.
    Json,
}
```

**R29.** The `inspect` subcommand MUST report the following statistics. **(MUST)**

| Statistic | Description | Source |
|-----------|-------------|--------|
| Agent count | Total live agents (`agents.iter().filter(Option::is_some).count()`) | SPEC-02, R7 |
| Wire count | Bidirectional connections / 2 | SPEC-02, R8 |
| Redex count | Current entries in the redex queue (after stale filtering) | SPEC-02, R9, R17 |
| CON count | Agents with `symbol == Symbol::Con` | SPEC-02, R1 |
| DUP count | Agents with `symbol == Symbol::Dup` | SPEC-02, R1 |
| ERA count | Agents with `symbol == Symbol::Era` | SPEC-02, R1 |
| Free port count | Ports connected to `FreePort(_)` in the port array | SPEC-02, R4 |
| Normal form | `redex_count == 0` | SPEC-02, R16 |

**R30.** When `--format json` is specified, the output MUST be a valid JSON object with the statistics as keys. **(MUST)**

```json
{
  "agents": 1000,
  "wires": 1500,
  "redexes": 500,
  "con": 400,
  "dup": 350,
  "era": 250,
  "free_ports": 6,
  "normal_form": false
}
```

**R31.** When `--format text` is specified (default), the output MUST be a human-readable table. **(MUST)**

```
=== Net Summary ===
Agents:      1000  (CON: 400, DUP: 350, ERA: 250)
Wires:       1500
Redexes:     500
Free ports:  6
Normal form: no
```

### 3.5 `relativist generate` Subcommand

**R32.** The `generate` subcommand MUST create pre-built example nets and write them to a file (cf. SPEC-13, R48). **(MUST)**

**R33.** The `generate` subcommand MUST accept the following arguments. **(MUST)**

```rust
/// Generate a pre-built example IC net.
#[derive(Debug, clap::Args)]
pub struct GenerateArgs {
    /// Name of the example to generate.
    #[arg(value_enum)]
    pub example: ExampleNet,

    /// Size parameter (semantics depend on the example).
    #[arg(long, short = 'n')]
    pub size: u32,

    /// Output file path (format inferred from extension, default: .bin).
    #[arg(long, short = 'o')]
    pub output: PathBuf,
}

/// Available pre-built example nets.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ExampleNet {
    /// N ERA-ERA annihilation pairs (Profile A). SPEC-09 R9.
    EpAnnihilation,
    /// N CON-CON annihilation pairs (Profile A). SPEC-09 R10.
    EpAnnihilationCon,
    /// N DUP-DUP annihilation pairs (Profile A). SPEC-09 R11.
    EpAnnihilationDup,
    /// N CON-DUP expansion pairs (Profile B). SPEC-09 R12.
    ConDupExpansion,
    /// Two binary trees of depth N (Profile C). SPEC-09 R13.
    DualTree,
    /// Tree sum with N work units (Profile A/B). SPEC-09 R14.
    TreeSum,
    /// Balanced tree sum with N work units. SPEC-09 R15.
    TreeSumBalanced,
    /// Mixed net with N pairs of each rule type. SPEC-09 R16.
    MixedRules,
    /// ERA propagation chain of length N (Profile C). SPEC-09 R17.
    ErasurePropagation,
}
```

**R34.** Each generator MUST produce a valid `Net` satisfying invariants T1 through T7 from SPEC-01. The generation function MUST validate the output net in debug mode (`#[cfg(debug_assertions)]`). **(MUST)**

**R35.** Generator functions MUST be pure functions with signature `fn generate_<name>(size: u32) -> Net`, reusable by both the `generate` subcommand and the benchmark suite (SPEC-09, `Benchmark::make_net`). **(MUST)**

**R36.** The generators MUST be implemented in the `io` module (SPEC-13, R5) or a shared `examples` submodule, NOT duplicated between the CLI and the benchmark suite. **(MUST)**

**R37.** Generator: `ep_annihilation(n: u32) -> Net`. MUST produce N pairs of ERA agents connected at principal ports. Expected reduction result: 0 agents. **(MUST)**

```rust
/// Generate N ERA-ERA annihilation pairs.
/// Each pair: two ERA agents with principal ports connected.
/// After reduction: empty net (0 agents).
pub fn ep_annihilation(n: u32) -> Net {
    let mut net = Net::new();
    for _ in 0..n {
        let a = net.create_agent(Symbol::Era);
        let b = net.create_agent(Symbol::Era);
        net.connect(
            PortRef::AgentPort(a, 0),
            PortRef::AgentPort(b, 0),
        );
    }
    net
}
```

**R38.** Generator: `ep_annihilation_con(n: u32) -> Net`. MUST produce N pairs of CON agents connected at principal ports, with auxiliary ports connected to free ports. **(MUST)**

```rust
/// Generate N CON-CON annihilation pairs.
/// Each pair: two CON agents, principals connected, auxiliaries to free ports.
/// After reduction: 0 CON agents, free ports cross-reconnected.
pub fn ep_annihilation_con(n: u32) -> Net {
    let mut net = Net::new();
    let mut free_id: u32 = 0;
    for _ in 0..n {
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Con);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        net.connect(PortRef::AgentPort(a, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(a, 2), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 1), PortRef::FreePort(free_id));
        free_id += 1;
        net.connect(PortRef::AgentPort(b, 2), PortRef::FreePort(free_id));
        free_id += 1;
    }
    net
}
```

**R39.** Generator: `dual_tree(depth: u32) -> Net`. MUST produce two perfect binary trees of CON agents with the given depth, connected at the roots via principal ports. **(MUST)**

**R40.** Generator: `con_dup_expansion(n: u32) -> Net`. MUST produce N CON-DUP pairs connected at principal ports, with auxiliary ports connected to free ports. **(MUST)**

**R41.** Generator: `mixed_rules(n: u32) -> Net`. MUST produce a net containing N pairs of each of the 6 interaction rule types (CON-CON, DUP-DUP, ERA-ERA, CON-DUP, CON-ERA, DUP-ERA), for a total of 6N initial redex pairs. **(MUST)**

**R42.** Generators for `tree_sum`, `tree_sum_balanced`, and `erasure_propagation` MUST follow the benchmark definitions in SPEC-09 (R14, R15, R17). **(MUST)**

**R43.** After generation, the `generate` subcommand MUST print a brief confirmation to stdout. **(MUST)**

```
Generated: ep-annihilation (size=1000)
  Agents: 2000
  Redexes: 1000
  Written to: net.bin (23.4 KB)
```

### 3.6 Reduction Summary Format

**R44.** After any reduction (local via `reduce` or distributed via `coordinator`), Relativist MUST print a Reduction Summary to stdout. **(MUST)**

**R45.** The Reduction Summary MUST include at minimum the following fields. **(MUST)**

```
=== Relativist Reduction Complete ===
Input:       1000 agents, 1500 wires, 500 redexes
Output:      42 agents, 63 wires, 0 redexes (normal form)
Interactions: 958
Duration:    1.234s
MIPS:        0.776
```

**R46.** For distributed execution (coordinator mode), the summary MUST additionally include grid-specific metrics. **(MUST)**

```
=== Relativist Reduction Complete ===
Input:       1000 agents, 1500 wires, 500 redexes
Output:      42 agents, 63 wires, 0 redexes (normal form)
Interactions: 958
Duration:    1.234s
MIPS:        0.776
Rounds:      7
Workers:     4
Speedup:     3.2x (vs sequential baseline)
Efficiency:  0.80
Overhead:    22.3%
```

**R47.** If the net did NOT reach Normal Form (e.g., `--max-interactions` or `--max-rounds` limit reached), the summary MUST indicate the reason. **(MUST)**

```
Output:      420 agents, 630 wires, 15 redexes (NOT normal form: max-interactions reached)
```

### 3.7 File Format Detection

**R48.** Relativist MUST auto-detect the input format by file extension. **(MUST)**

| Extension | Format |
|-----------|--------|
| `.bin` | Binary (serde + bincode) |
| `.ic` | Text DSL |
| `.json` | JSON (serde_json) |

**R49.** If the file extension is not recognized, Relativist MUST return an error: `"Unrecognized file extension '{ext}'. Supported: .bin, .ic, .json"`. **(MUST)**

**R50.** The `--format` flag MUST override extension-based detection when provided. **(MUST)**

```rust
/// Supported net file formats.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum NetFormat {
    /// Binary: serde + bincode (fastest, most compact).
    Bin,
    /// Text DSL: human-readable (.ic files).
    Ic,
    /// JSON: serde_json (interop).
    Json,
}
```

### 3.8 I/O Module API

**R51.** The `io` module MUST expose the following public API for loading and saving nets. **(MUST)**

```rust
/// Load a Net from a file, auto-detecting format by extension.
pub fn load_net(path: &Path) -> Result<Net, IoError>;

/// Load a Net from a file with explicit format.
pub fn load_net_with_format(path: &Path, format: NetFormat) -> Result<Net, IoError>;

/// Save a Net to a file, auto-detecting format by extension.
pub fn save_net(net: &Net, path: &Path) -> Result<(), IoError>;

/// Save a Net with explicit format.
pub fn save_net_with_format(net: &Net, path: &Path, format: NetFormat) -> Result<(), IoError>;

/// Parse a Net from a text DSL string.
pub fn parse_ic(input: &str) -> Result<Net, ParseError>;

/// Serialize a Net to a text DSL string.
pub fn format_ic(net: &Net) -> String;
```

**R52.** The `IoError` type MUST be defined with `thiserror` and MUST distinguish between I/O errors, parse errors, and format errors. **(MUST)**

```rust
/// Errors from the I/O subsystem.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },
    #[error("serialization error: {0}")]
    Serialization(String),
    #[error("unrecognized file extension: {0}")]
    UnrecognizedFormat(String),
    #[error("format not supported in this build: {0}")]
    UnsupportedFormat(String),
}
```

---

## 4. Design

### 4.1 Text DSL Parser Architecture

The text DSL parser operates in two passes:

**Pass 1 -- Declaration collection:** Scan all lines, collect `agent` declarations (mapping IDENT -> (AgentId, Symbol)) and `wire` declarations (pairs of port references). Assign `AgentId` values sequentially starting from 0.

**Pass 2 -- Net construction:** Create a `Net`, call `create_agent` for each agent declaration, then call `connect` for each wire declaration. Resolve agent port references (e.g., `a.principal`) to `PortRef::AgentPort(id, port_id)` using the declaration map from Pass 1.

Validation occurs after construction: check T1 (every port connected to exactly one target) and I2 (all AgentPort references valid). Report errors with line numbers.

### 4.2 Text DSL Serializer

The serializer produces a text DSL string from a `Net`:

1. Assign names to agents: `a0`, `a1`, `a2`, ... (by AgentId order).
2. Emit `agent <name> <SYMBOL>` for each live agent.
3. Emit `wire <port_ref> <port_ref>` for each connection (emit each bidirectional connection once, by visiting ports in canonical order: lower AgentId first).
4. Emit `root <port_ref>` if the net has a root port.

### 4.3 Generator Sharing Between CLI and Benchmarks

The generators defined in R37-R42 reside in a shared location (the `io` module or a dedicated `examples` submodule). Both the `generate` CLI subcommand and the `Benchmark` trait implementations (SPEC-09) call the same generator functions. This prevents duplication and ensures that CLI-generated nets and benchmark nets are identical for the same parameters.

```
io/
├── mod.rs          # load_net, save_net, parse_ic, format_ic
├── binary.rs       # bincode serialization/deserialization
├── text.rs         # Text DSL parser and serializer
├── json.rs         # JSON serialization (MAY be stub in v1)
└── examples.rs     # Generator functions: ep_annihilation, dual_tree, etc.
```

### 4.4 Net Summary Computation

```rust
/// Compute summary statistics for a net.
pub fn net_summary(net: &Net) -> NetSummary {
    let mut con = 0u32;
    let mut dup = 0u32;
    let mut era = 0u32;
    let mut agent_count = 0u32;
    for agent in net.agents.iter().flatten() {
        agent_count += 1;
        match agent.symbol {
            Symbol::Con => con += 1,
            Symbol::Dup => dup += 1,
            Symbol::Era => era += 1,
        }
    }
    // ... wire count, redex count, free port count ...
    NetSummary { agent_count, con, dup, era, /* ... */ }
}

/// Summary statistics of a net.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NetSummary {
    pub agents: u32,
    pub wires: u32,
    pub redexes: u32,
    pub con: u32,
    pub dup: u32,
    pub era: u32,
    pub free_ports: u32,
    pub normal_form: bool,
}
```

---

## 5. Rationale

### 5.1 Why three formats

Binary (.bin) is the performance format: zero parsing overhead, compact representation, used by benchmarks where I/O should not be the bottleneck. Text DSL (.ic) is the human format: essential for documentation, hand-crafted test cases, and debugging small nets. JSON (.json) is the interop format: enables visualization tools, web frontends, and integration with analysis pipelines. The Haskell prototype had no text format, making it impossible to hand-craft test nets without writing Haskell code -- a significant usability gap.

### 5.2 Why .ic extension for the text DSL

The `.ic` extension (for Interaction Combinators) avoids confusion with generic `.txt` files and makes the format self-documenting. It also enables IDE syntax highlighting extensions in the future.

### 5.3 Why generators are shared between CLI and benchmarks

The Haskell prototype (AC-005) defines generators inside the benchmark module (`IC.Benchmark.EPAnnihilation`, etc.). Relativist lifts them to a shared module so that `relativist generate` can produce the same nets without depending on the benchmark framework. This makes it possible to generate a net, inspect it, reduce it locally, and compare with distributed reduction -- all without running the full benchmark suite.

### 5.4 Why JSON is MAY and not MUST

JSON serialization of the `Net` struct is straightforward (serde_json) but adds a dependency and is not needed for the core TCC evaluation. Binary and text DSL cover the primary use cases (benchmarks and human inspection). JSON is a convenience for future visualization work and can be deferred without blocking the research goals.

### 5.5 Why --max-interactions on reduce

Interaction Combinator reduction is not guaranteed to terminate for all nets (SPEC-01, T6 applies only to terminating nets). A safety limit prevents the `reduce` subcommand from running forever on pathological inputs.

---

## 6. Haskell Prototype Reference

### 6.1 What the prototype provides

The Haskell prototype defines generators in dedicated modules (`IC.Benchmark.EPAnnihilation`, `IC.Benchmark.DualTree`, `IC.Benchmark.CONDUPExpansion`) and net construction in `IC.Benchmark` (AC-005). The `BenchDef` type bundles `bdMakeNet :: Int -> Net` with the benchmark. There is no standalone CLI for generation, inspection, or local reduction -- all operations go through the benchmark harness or GHCi REPL.

### 6.2 What Relativist changes and why

1. **Text DSL:** The prototype has no text format. Relativist adds `.ic` for hand-crafted nets and documentation examples.
2. **CLI subcommands:** The prototype has no `reduce`, `inspect`, or `generate` commands. Relativist exposes these as first-class CLI subcommands (SPEC-13, R43-R48).
3. **Shared generators:** The prototype's generators are tightly coupled to the benchmark framework. Relativist decouples them into a shared module.
4. **JSON format:** The prototype uses Haskell's `Show` instance for debugging. Relativist adds structured JSON output for machine consumption.
5. **Reduction Summary:** The prototype prints benchmark tables. Relativist prints a standardized summary for every reduction operation.

---

## 7. Test Requirements

**T1.** Binary roundtrip: for each generator, `deserialize(serialize(generate(n))) == generate(n)` for N in {1, 10, 100}. **(MUST)**

**T2.** Text DSL roundtrip: for each generator, `parse_ic(format_ic(generate(n)))` MUST produce a net structurally equivalent to `generate(n)` for N in {1, 5, 10}. **(MUST)**

**T3.** Text DSL parser error handling: MUST reject malformed inputs (missing wire endpoint, unknown agent name, ERA with auxiliary port, duplicate port connection) with descriptive errors. **(MUST)**

**T4.** Each generator MUST produce a valid net (invariants T1-T7 from SPEC-01) for N in {1, 10, 100, 1000}. Validated via `debug_assert` in debug mode and explicit test assertions. **(MUST)**

**T5.** `inspect` correctness: for a known net (e.g., `ep_annihilation(10)`), verify that all reported statistics match expected values (20 agents, 10 redexes, 0 CON, 0 DUP, 20 ERA, 0 free ports, not normal form). **(MUST)**

**T6.** `reduce` correctness: for `ep_annihilation(10)`, verify that the output net has 0 agents and 0 redexes (Normal Form). **(MUST)**

**T7.** `reduce` with `--max-interactions 5` on `ep_annihilation(10)`: verify that reduction stops early and the output is NOT in Normal Form. **(MUST)**

**T8.** File format detection: verify that `.bin`, `.ic`, `.json` extensions are correctly mapped to the corresponding format. Verify that `.xyz` returns `UnrecognizedFormat` error. **(MUST)**

**T9.** Generator consistency: for each `ExampleNet` variant, verify that `generate` CLI produces the same net as the corresponding `Benchmark::make_net` in the benchmark suite (SPEC-09). **(MUST)**

---

## 8. Open Questions

None. All design decisions are resolved.
