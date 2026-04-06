# SPEC-14: Arithmetic Encoding

**Status:** Revised v2
**Depends on:** SPEC-00 (Glossary), SPEC-02 (Net Representation), SPEC-03 (Reduction Engine)
**Gray zones resolved:** ---
**References consumed:** REF-002 (Lafont 1997, universality proof, Section 4)
**Discussions consumed:** ---
**Arguments consumed:** ---
**Code analyses consumed:** ---

---

## 1. Purpose

This spec defines the encoding/decoding layer for Relativist: Church numeral representations of natural numbers as IC nets, arithmetic operation combinators (addition, multiplication, exponentiation) as IC net constructions, a decoding (readback) algorithm that extracts numeric results from Normal Form nets, and a `compute` CLI subcommand that combines encoding, reduction, and decoding into a single user-facing workflow. This module bridges the gap between abstract IC net reduction and practical computation, enabling demonstrations where Relativist computes real arithmetic distributedly -- essential for the TCC experimental evaluation (SPEC-09) and defense.

## 2. Definitions

Terms defined in SPEC-00 (Glossary) are used without redefinition. Terms introduced or refined in this spec:

| Term | Definition |
|------|-----------|
| **Church Numeral** | An IC net encoding of a natural number n as the lambda term `lambda f. lambda x. f^n(x)`. Church numeral 0 erases f and returns x directly. Church numeral n (n >= 1) applies f to x exactly n times. The encoding uses CON agents for lambda abstractions and applications, DUP agents for variable sharing (when f is used more than once), and ERA agents for erasure (when f is unused, i.e., n = 0). (Lafont 1997, Section 4: universality via encoding of lambda calculus.) |
| **Encoding** | The process of translating a high-level value (natural number) or expression (arithmetic operation applied to operands) into an IC net. The resulting net contains redexes whose reduction computes the result. **(Relativist)** |
| **Decoding (Readback)** | The process of interpreting a Church numeral IC net in Normal Form as a natural number. Performed by traversing the net structure and counting the application chain length. Inverse of encoding. **(Relativist)** |
| **Arithmetic Net** | An IC net constructed by composing Church numeral sub-nets with an arithmetic combinator (addition, multiplication, or exponentiation). When reduced to Normal Form via `reduce_all` (SPEC-03), the result is a Church numeral encoding the arithmetic result. **(Relativist)** |
| **Combinator** | A closed lambda term (no free variables) that implements an operation. In this spec: `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`, `mul = lambda m. lambda n. lambda f. m (n f)`, `exp = lambda m. lambda n. n m`. Each combinator is encoded as an IC net fragment that connects to the operand sub-nets. |

---

## 3. Requirements

### 3.1 Encoding Module (Core Layer)

**R1.** The encoding module MUST reside in `src/encoding/` within the Core Layer. It MUST be pure synchronous Rust with no `async fn` signatures, no tokio dependency, and no I/O operations. **(MUST)**

**R2.** The encoding module MUST depend only on types from the `net` module (SPEC-02): `Net`, `Symbol`, `PortRef`, `AgentId`, `PortId`. It MUST NOT depend on the `reduction`, `partition`, `merge`, or any Infrastructure Layer module. **(MUST)**

**R3.** The encoding module MUST be organized as follows. **(MUST)**

```
src/encoding/
    mod.rs          # Re-exports: encode_nat, decode_nat, build_add, build_mul, build_exp
    church.rs       # Church numeral encoding and decoding
    arithmetic.rs   # Arithmetic operation combinators (add, mul, exp)
```

### 3.2 Church Numeral Encoding

**R4.** The encoding module MUST expose the function `encode_nat(n: u64) -> Net` that produces a Church numeral IC net for any n in the range [0, 10_000]. For n > 10_000, the function MUST panic with a descriptive message (e.g., `"encode_nat: n = {n} exceeds maximum supported value 10_000"`). **(MUST)**

```rust
/// Encode a natural number as a Church numeral IC net.
///
/// The resulting net is already in Normal Form (zero redexes).
/// To perform computation, compose with arithmetic combinators
/// (build_add, build_mul, build_exp) which introduce redexes.
///
/// # Panics
/// Panics if n > 10_000.
pub fn encode_nat(n: u64) -> Net;
```

**R4b.** The encoding module MUST expose an internal builder variant `encode_church_into(net: &mut Net, n: u64) -> AgentId` that constructs a Church numeral inside an existing net and returns the AgentId of the outer lambda agent (the root of the Church numeral sub-net). This function is used by the arithmetic combinators (`build_add`, `build_mul`, `build_exp`) to compose multiple Church numerals in a single net without ID collisions. `encode_nat(n)` is a convenience wrapper that creates a new `Net`, calls `encode_church_into`, sets the root, and returns the net. **(MUST)**

```rust
/// Encode a natural number as a Church numeral inside an existing net.
///
/// Returns the AgentId of the outer lambda agent (lambda f).
/// Does NOT set net.root -- the caller is responsible for wiring
/// the returned agent's principal port into the surrounding net.
///
/// # Panics
/// Panics if n > 10_000.
pub fn encode_church_into(net: &mut Net, n: u64) -> AgentId;
```

**R5.** Church numeral 0 (`lambda f. lambda x. x`): The net MUST contain exactly 2 CON agents (lambda abstractions for f and x) and 1 ERA agent (erasing the unused f variable). The ERA agent's principal port MUST connect to the outer lambda's left auxiliary port (the f binding). The inner lambda's left auxiliary (x binding) MUST connect to its right auxiliary (body result), representing the identity on x. Total: 3 agents, 0 redexes. **(MUST)**

```
Church(0) = lambda f. lambda x. x

     root (net.root = AgentPort(CON_0, 0))
         |
       [CON_0]  <- outer lambda (lambda f)
       /    \
     p1      p2
      |       |
    [ERA_0]  [CON_1]  <- inner lambda (lambda x)
              /    \
            p1  <-> p2   (x binding wired to body result)
```

Note: The self-loop `CON_1.p1 <-> CON_1.p2` is correct and satisfies invariants T1 and I1 from SPEC-01. Self-loops represent the identity function (`lambda x. x`) in the IC encoding of lambda calculus. `ports[CON_1.p1] = AgentPort(CON_1, 2)` and `ports[CON_1.p2] = AgentPort(CON_1, 1)`, so `ports[ports[p]] = p` holds for both ports. Implementers MUST NOT add assertions that reject self-loops, as they would break Church(0).

**R6.** Church numeral 1 (`lambda f. lambda x. f x`): The net MUST contain exactly 3 CON agents: 2 lambda abstractions (for f and x) and 1 application (f applied to x). No DUP or ERA agents. Total: 3 agents, 0 redexes. **(MUST)**

Port connectivity for Church(1):
- CON_f (lambda f): p0 = root (net.root = AgentPort(CON_f, 0)), p1 = f variable, p2 = body
- CON_x (lambda x): p0 = CON_f.p2, p1 = x variable, p2 = body result
- CON_app (@ f x): p0 = function, p1 = argument, p2 = result

Connections:
- CON_f.p1 = CON_app.p0 (f variable feeds application function port)
- CON_f.p2 = CON_x.p0 (body of lambda f is lambda x)
- CON_x.p1 = CON_app.p1 (x variable feeds application argument port)
- CON_x.p2 = CON_app.p2 (body result is application result)

Zero redexes: all principal ports (p0) connect to non-principal ports of other agents or to FreePort, so no active pairs exist.

**R7.** Church numeral n (n >= 2, `lambda f. lambda x. f^n(x)`): The net MUST contain exactly (n + 2) CON agents (2 lambda abstractions + n applications) and (n - 1) DUP agents (for sharing the variable f across n uses). Total: (2n + 1) agents, 0 redexes. **(MUST)**

The DUP agents MUST form a linear chain: DUP_0.p0 receives f from CON_f.p1; for each DUP_i (i < n-2), DUP_i.p1 provides one copy to application i, and DUP_i.p2 feeds the next DUP; the last DUP provides its two copies to the last two applications.

**R8.** All nets produced by `encode_nat` MUST satisfy invariants T1 through T7 from SPEC-01. In particular: T1 (port linearity -- every port connected to exactly one target), I1 (bidirectionality), and I3 (ID monotonicity). The function MUST validate the output net in debug mode (`#[cfg(debug_assertions)]`). **(MUST)**

**R9.** The net produced by `encode_nat` MUST have `net.root = Some(PortRef::AgentPort(lam_f, 0))`, where `lam_f` is the AgentId of the outer lambda agent (`lambda f`). This follows SPEC-02, Section 4.9, which defines root as `Option<PortRef>` with `AgentPort` as the canonical value. The outer lambda's principal port serves as the external observation point of the Church numeral. **(MUST)**

Note: In the port connection tables (Section 4.2), `CON_0.p0 = root` denotes that CON_0's principal port is the root observation point (i.e., `net.root = Some(AgentPort(CON_0.id, 0))`). This is NOT a `FreePort(0)` stored in the port array. The principal port of the root agent connects to `DISCONNECTED` in the port array (no internal peer), and the `net.root` field provides the external reference.

**R10.** The net produced by `encode_nat(n)` MUST be in Normal Form (zero active pairs in the redex queue). **(MUST)**

### 3.3 Church Numeral Decoding (Readback)

**R11.** The encoding module MUST expose the function `decode_nat(net: &Net) -> Option<u64>`. **(MUST)**

```rust
/// Decode a Church numeral IC net in Normal Form to a natural number.
///
/// Returns Some(n) if the net has the structure of Church numeral n.
/// Returns None if the net is not a recognizable Church numeral
/// (e.g., not in Normal Form, or has an unexpected topology).
///
/// This function does NOT modify the input net.
pub fn decode_nat(net: &Net) -> Option<u64>;
```

**R12.** Decoding MUST work by structural traversal. **(MUST)**

The algorithm:
1. From `net.root`, follow to the outer CON agent (lambda f). Verify symbol is CON.
2. From lambda_f.p2 (body), follow to the inner CON agent (lambda x). Verify symbol is CON.
3. From lambda_x.p2 (body result), walk the application chain: each step follows from the current port to a CON agent (application), then continues from that agent's p2 (result port).
4. Count the number of CON application agents traversed. This count is n.
5. For n = 0: verify that lambda_f.p1 connects to an ERA agent (f is erased) and lambda_x.p1 connects to lambda_x.p2 (identity on x).
6. For n >= 1: verify the DUP chain and application chain are well-formed.

**R13.** Decoding MUST NOT modify the input net. The function takes `&Net` (shared reference). **(MUST)**

**R14.** Decoding MUST return `None` (not panic) for any net that does not match the expected Church numeral structure: wrong number of agents, wrong symbols, unexpected topology, or net not in Normal Form (non-empty redex queue). **(MUST)**

### 3.4 Arithmetic Operations

**R15.** The encoding module MUST expose `build_add(a: u64, b: u64) -> Net`. **(MUST)**

```rust
/// Build an IC net that, when reduced to Normal Form, yields Church numeral (a + b).
///
/// Encodes: add church(a) church(b) where add = lambda m. lambda n. lambda f. lambda x. m f (n f x).
/// The resulting net contains active pairs (redexes) that must be reduced.
pub fn build_add(a: u64, b: u64) -> Net;
```

Mathematical basis: `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)`. When applied to church(a) and church(b), beta-reduction produces church(a + b).

**R16.** The encoding module MUST expose `build_mul(a: u64, b: u64) -> Net`. **(MUST)**

```rust
/// Build an IC net that, when reduced to Normal Form, yields Church numeral (a * b).
///
/// Encodes: mul church(a) church(b) where mul = lambda m. lambda n. lambda f. m (n f).
pub fn build_mul(a: u64, b: u64) -> Net;
```

Mathematical basis: `mul = lambda m. lambda n. lambda f. m (n f)`. When applied to church(a) and church(b), beta-reduction produces church(a * b).

**R17.** The encoding module MUST expose `build_exp(base: u64, exp: u64) -> Net`. **(MUST)**

```rust
/// Build an IC net that, when reduced to Normal Form, yields Church numeral (base ^ exp).
///
/// Encodes: exp church(base) church(exp) where exp = lambda m. lambda n. n m.
/// WARNING: The reduction of exp(base, exp) requires O(base^exp) interactions.
/// Use small values to avoid excessive computation.
pub fn build_exp(base: u64, exp: u64) -> Net;
```

Mathematical basis: `exp = lambda m. lambda n. n m`. Church exponentiation is the most natural Church arithmetic operation -- it is simply application.

**R18.** All arithmetic nets (addition, multiplication, exponentiation) MUST reduce to a valid Church numeral Normal Form when processed by `reduce_all` (SPEC-03). Formally: for all valid operands, the following MUST hold: **(MUST)**

```rust
let mut net = build_op(a, b);
reduce_all(&mut net);   // SPEC-03, R13: mutates net in place, returns ReductionStats
assert_eq!(decode_nat(&net), Some(expected_result));
```

**R19.** Factorial encoding MAY be implemented as a stretch goal. It requires encoding the Y-combinator (fixed-point combinator) as an IC net, which introduces significant complexity. If not implemented, the module MUST NOT expose a `build_factorial` function. **(MAY)**

### 3.5 Complexity Bounds

**R20.** The following complexity bounds MUST hold for encoding functions. **(MUST)**

| Function | Agent Count | Construction Time | Reduction Interactions |
|----------|------------|-------------------|----------------------|
| `encode_nat(n)` | 2n + 1 (n >= 2); 3 (n in {0, 1}) | O(n) | 0 (already Normal Form) |
| `build_add(a, b)` | O(a + b) | O(a + b) | O(a + b) |
| `build_mul(a, b)` | O(a + b) | O(a + b) | O(a * b) |
| `build_exp(a, b)` | O(a + b) | O(a + b) | O(a^b) |

Note: `build_mul` produces a compact net (O(a + b) agents) but reduction generates O(a * b) interactions because DUP-CON commutations expand the net before annihilations collapse it. This is Profile B behavior (SPEC-09).

**Complexity justification:** The interaction counts are asymptotic estimates derived from the Church encoding structure. For addition (`add(a, b)`): the add combinator applies `m` to `f`, producing `a` applications of `f`, then applies `n` to `(f, x)`, producing `b` applications of `f`. The beta-reductions for the combinator application and the resulting annihilations total O(a + b) interactions. For multiplication (`mul(a, b)`): `m` applies the composed function `(n f)` a total of `a` times, and each application of `n f` involves `b` interactions, yielding O(a * b). For exponentiation (`exp(a, b)`): `n` applies `m` b times, and each level involves the previous level's result, yielding O(a^b). These are order-of-magnitude estimates; exact interaction counts depend on the encoding strategy (direct construction vs. combinator composition) and will be determined empirically during benchmarking (SPEC-09). The benchmark table in Section 8.2 provides approximate values for specific inputs.

**R21.** `decode_nat` MUST run in O(n) time where n is the decoded value (proportional to the application chain length). **(MUST)**

### 3.6 CLI Integration -- `compute` Subcommand

**R22.** A `compute` subcommand MUST be added to the Relativist CLI (SPEC-13, R43). **(MUST)**

```rust
/// Encode an arithmetic expression as an IC net, reduce it, and decode the result.
#[derive(Debug, clap::Args)]
pub struct ComputeArgs {
    /// Arithmetic operation to perform.
    #[arg(value_enum)]
    pub operation: ArithmeticOp,

    /// First operand.
    pub a: u64,

    /// Second operand.
    pub b: u64,

    /// Number of workers for distributed reduction.
    /// If omitted, reduces locally via reduce_all.
    #[arg(long)]
    pub workers: Option<u32>,

    /// Path to write the reduced net file.
    #[arg(long)]
    pub output: Option<PathBuf>,

    /// Path to write metrics JSON.
    #[arg(long)]
    pub metrics: Option<PathBuf>,
}

/// Supported arithmetic operations for the compute subcommand.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum ArithmeticOp {
    /// Addition: a + b
    Add,
    /// Multiplication: a * b
    Mul,
    /// Exponentiation: a ^ b
    Exp,
}
```

**R23.** Without `--workers`: the `compute` subcommand MUST use local `reduce_all` (SPEC-03). With `--workers N`: it MUST use the distributed grid with N workers (SPEC-05, SPEC-13). **(MUST)**

**R24.** After reduction, the `compute` subcommand MUST decode the result using `decode_nat` and print a human-readable summary to stdout. **(MUST)**

Local mode output format:

```
=== Relativist Compute ===
Expression:  add(500, 500)
Encoding:    2006 agents, 1001 redexes
Reduction:   958 interactions in 0.12s (7.98 MIPS)
Result:      1000
```

Distributed mode output format (includes grid-specific metrics, cf. SPEC-12, R46):

```
=== Relativist Compute ===
Expression:  add(500, 500)
Encoding:    2006 agents, 1001 redexes
Reduction:   958 interactions in 0.04s (23.95 MIPS)
Workers:     4
Rounds:      5
Speedup:     3.17x
Result:      1000
```

**R25.** If `decode_nat` returns `None` after reduction, the `compute` subcommand MUST print a warning: `"Warning: result is not a recognizable Church numeral. The net may not have reached Normal Form or the encoding may be incorrect."` and MUST still print the Reduction Summary and net statistics. **(MUST)**

### 3.7 Generator Integration

**R26.** The following `ExampleNet` variants MUST be added to the existing enum (SPEC-12, R33). **(MUST)**

```rust
/// Church numeral encoding of N.
ChurchNat,
/// Addition: church(N/2) + church(N/2) (or church(N/2) + church(N/2 + 1) if N is odd).
ChurchAdd,
/// Multiplication: church(sqrt(N)) * church(sqrt(N)) (approximated).
ChurchMul,
```

The `size` parameter (N) controls the magnitude of the operands, which in turn determines the number of agents and redexes in the generated net.

**R27.** These generators MUST be usable from both the `generate` subcommand (SPEC-12) and the benchmark suite (SPEC-09, `Benchmark::make_net`). They MUST reside in the shared generator location alongside existing generators (SPEC-12, R35-R36). **(MUST)**

---

## 4. Design

### 4.1 Church Numeral Construction Algorithm

The core construction logic resides in `encode_church_into`, which builds a Church numeral inside an existing net. `encode_nat` is a convenience wrapper.

```rust
pub fn encode_nat(n: u64) -> Net {
    assert!(n <= 10_000, "encode_nat: n = {n} exceeds maximum supported value 10_000");
    let mut net = Net::new();
    let lam_f = encode_church_into(&mut net, n);
    net.root = Some(PortRef::AgentPort(lam_f, 0));
    debug_assert!(
        net.redex_queue.is_empty(),
        "Church numeral must be in Normal Form"
    );
    net
}

pub fn encode_church_into(net: &mut Net, n: u64) -> AgentId {
    assert!(n <= 10_000, "encode_church_into: n = {n} exceeds maximum supported value 10_000");

    // Step 1: Create the two lambda abstractions
    let lam_f = net.create_agent(Symbol::Con);  // outer lambda f
    let lam_x = net.create_agent(Symbol::Con);  // inner lambda x
    // Connect lambda_f body to lambda_x interface
    net.connect(
        PortRef::AgentPort(lam_f, 2),
        PortRef::AgentPort(lam_x, 0),
    );

    match n {
        0 => {
            // lambda f. lambda x. x  -- f is erased, x is identity
            let era = net.create_agent(Symbol::Era);
            net.connect(
                PortRef::AgentPort(lam_f, 1),
                PortRef::AgentPort(era, 0),
            );
            // x binding -> body result (identity)
            net.connect(
                PortRef::AgentPort(lam_x, 1),
                PortRef::AgentPort(lam_x, 2),  // self-loop on auxiliaries
            );
        }
        1 => {
            // lambda f. lambda x. f x  -- single application, no DUP
            let app = net.create_agent(Symbol::Con);
            net.connect(
                PortRef::AgentPort(lam_f, 1),   // f binding
                PortRef::AgentPort(app, 0),      // -> app function port
            );
            net.connect(
                PortRef::AgentPort(lam_x, 1),   // x binding
                PortRef::AgentPort(app, 1),      // -> app argument port
            );
            net.connect(
                PortRef::AgentPort(lam_x, 2),   // body result
                PortRef::AgentPort(app, 2),      // -> app result port
            );
        }
        n => {
            // lambda f. lambda x. f^n(x)  -- n applications, (n-1) DUPs for sharing f
            let n = n as usize;

            // Create n application agents
            let apps: Vec<AgentId> = (0..n)
                .map(|_| net.create_agent(Symbol::Con))
                .collect();

            // Create (n-1) DUP agents for sharing f
            let dups: Vec<AgentId> = (0..n - 1)
                .map(|_| net.create_agent(Symbol::Dup))
                .collect();

            // Wire f variable to DUP chain
            net.connect(
                PortRef::AgentPort(lam_f, 1),
                PortRef::AgentPort(dups[0], 0),
            );

            // Wire DUP chain: each DUP feeds one copy to an app
            // and passes the rest to the next DUP
            for i in 0..dups.len() {
                // Left output -> application i's function port
                net.connect(
                    PortRef::AgentPort(dups[i], 1),
                    PortRef::AgentPort(apps[i], 0),
                );
                if i + 1 < dups.len() {
                    // Right output -> next DUP
                    net.connect(
                        PortRef::AgentPort(dups[i], 2),
                        PortRef::AgentPort(dups[i + 1], 0),
                    );
                } else {
                    // Last DUP: right output -> last application
                    net.connect(
                        PortRef::AgentPort(dups[i], 2),
                        PortRef::AgentPort(apps[n - 1], 0),
                    );
                }
            }

            // Wire x to innermost application argument
            net.connect(
                PortRef::AgentPort(lam_x, 1),
                PortRef::AgentPort(apps[n - 1], 1),
            );

            // Chain application results: app[i].p2 -> app[i-1].p1
            for i in (1..n).rev() {
                net.connect(
                    PortRef::AgentPort(apps[i], 2),
                    PortRef::AgentPort(apps[i - 1], 1),
                );
            }

            // Outermost application result -> body result
            net.connect(
                PortRef::AgentPort(apps[0], 2),
                PortRef::AgentPort(lam_x, 2),
            );
        }
    }

    lam_f
}
```

### 4.2 Port Connection Tables

These tables define the exact wiring of each Church numeral encoding. All connections are bidirectional (SPEC-01, I1). The tables serve as the ground truth for both construction (R4-R7) and decoding (R11-R12).

**Church(0) -- 3 agents (2 CON + 1 ERA):**

| Agent | Symbol | p0 connects to | p1 connects to | p2 connects to |
|-------|--------|----------------|----------------|----------------|
| 0 (lambda f) | CON | root (DISCONNECTED in port array) | ERA_0.p0 | CON_1.p0 |
| 1 (lambda x) | CON | CON_0.p2 | CON_1.p2 (self-loop) | CON_1.p1 (self-loop) |
| 2 (ERA) | ERA | CON_0.p1 | -- | -- |

`net.root = Some(AgentPort(0, 0))`.

Note on CON_0.p0: The principal port of the root agent has no internal peer in the port array (its slot contains `DISCONNECTED`). The external reference is provided by `net.root`. This is consistent with SPEC-02, Section 4.9.

Note on self-loop: CON_1.p1 <-> CON_1.p2 is a valid self-loop representing the identity function (`lambda x. x`). This satisfies T1 and I1 from SPEC-01 (see R5 note above).

Redex verification: No principal-to-principal connections exist. CON_0.p0 = DISCONNECTED (root, no redex), CON_1.p0 = CON_0.p2 (auxiliary), ERA_0.p0 = CON_0.p1 (auxiliary). Zero redexes.

**Church(1) -- 3 agents (3 CON):**

| Agent | Symbol | p0 connects to | p1 connects to | p2 connects to |
|-------|--------|----------------|----------------|----------------|
| 0 (lambda f) | CON | root (DISCONNECTED in port array) | CON_2.p0 | CON_1.p0 |
| 1 (lambda x) | CON | CON_0.p2 | CON_2.p1 | CON_2.p2 |
| 2 (@ app) | CON | CON_0.p1 | CON_1.p1 | CON_1.p2 |

`net.root = Some(AgentPort(0, 0))`.

Redex verification: CON_0.p0 = DISCONNECTED (root, no redex), CON_1.p0 = CON_0.p2 (auxiliary, no redex), CON_2.p0 = CON_0.p1 (auxiliary, no redex). Zero redexes.

**Church(2) -- 5 agents (4 CON + 1 DUP):**

| Agent | Symbol | p0 connects to | p1 connects to | p2 connects to |
|-------|--------|----------------|----------------|----------------|
| 0 (lambda f) | CON | root (DISCONNECTED in port array) | DUP_0.p0 | CON_1.p0 |
| 1 (lambda x) | CON | CON_0.p2 | CON_3.p1 | CON_2.p2 |
| 2 (@_1, outer app) | CON | DUP_0.p1 | CON_3.p2 | CON_1.p2 |
| 3 (@_2, inner app) | CON | DUP_0.p2 | CON_1.p1 | CON_2.p1 |
| 4 (DUP_0) | DUP | CON_0.p1 | CON_2.p0 | CON_3.p0 |

`net.root = Some(AgentPort(0, 0))`.

Redex verification: active pairs require principal-to-principal (p0 <-> p0) connections between agents. Checking all p0 connections:
- CON_0.p0 = DISCONNECTED (root) -- no redex
- CON_1.p0 = CON_0.p2 (auxiliary) -- no redex
- CON_2.p0 = DUP_0.p1 (auxiliary) -- no redex
- CON_3.p0 = DUP_0.p2 (auxiliary) -- no redex
- DUP_0.p0 = CON_0.p1 (auxiliary) -- no redex

Zero redexes confirmed.

**Church(n) for n >= 2 -- (2n + 1) agents ((n + 2) CON + (n - 1) DUP):**

General structure:
- Agents 0-1: lambda_f (CON), lambda_x (CON)
- Agents 2 to (n+1): app_0 through app_(n-1) (CON), where app_0 is outermost
- Agents (n+2) to (2n): dup_0 through dup_(n-2) (DUP)

General wiring:
- lambda_f.p0 = root (DISCONNECTED in port array; `net.root = Some(AgentPort(lambda_f, 0))`)
- lambda_f.p1 = dup_0.p0
- lambda_f.p2 = lambda_x.p0
- lambda_x.p1 = app_(n-1).p1 (x variable feeds innermost application argument)
- lambda_x.p2 = app_0.p2 (body result is outermost application result)
- dup_i.p1 = app_i.p0 (each DUP feeds one f copy to application i)
- dup_i.p2 = dup_(i+1).p0 (chain to next DUP), except last DUP
- dup_(n-2).p2 = app_(n-1).p0 (last DUP feeds last application)
- app_i.p1 = app_(i+1).p2 for i < n-1 (each app's argument is the next app's result)
- app_i.p2 = app_(i-1).p1 for i > 0 (each app's result feeds the previous app's argument)
- app_0.p2 = lambda_x.p2 (outermost result is body result)
- app_(n-1).p1 = lambda_x.p1 (innermost argument is x)

### 4.3 Arithmetic Net Construction

#### 4.3.1 Addition: build_add(a, b)

The addition combinator `add = lambda m. lambda n. lambda f. lambda x. m f (n f x)` is constructed as a separate IC net fragment and connected to the Church numeral sub-nets for a and b via two application nodes.

**ID composition:** All arithmetic construction functions (`build_add`, `build_mul`, `build_exp`) MUST construct everything in a single `Net` by calling `encode_church_into` (R4b) sequentially. Because `encode_church_into` calls `net.create_agent` on the shared net, all agent IDs are assigned monotonically from the same `next_id` counter, satisfying invariant I3 (SPEC-01) without any ID remapping.

The complete net for `add(a, b)`:
1. Create a single `Net`
2. Call `encode_church_into(&mut net, a)` to build church(a) in the net -- returns `m_root: AgentId`
3. Call `encode_church_into(&mut net, b)` to build church(b) in the net -- returns `n_root: AgentId`
4. Build the add combinator IC net fragment (4 lambda abstractions as CON agents)
5. Create two application CON agents: `@_1` applies add to church(a), `@_2` applies the result to church(b)
6. Connect: `@_1.p0 = add_root, @_1.p1 = AgentPort(m_root, 0), @_1.p2 = @_2.p0, @_2.p1 = AgentPort(n_root, 0), @_2.p2 = result_root`

After connecting the sub-nets, new active pairs emerge at the application boundaries -- these are the redexes that drive the computation.

**Alternative (direct construction):** Instead of building the full `add` combinator, `build_add` MAY directly construct the expanded term `(lambda f. lambda x. church_a f (church_b f x))` to avoid unnecessary beta-reduction steps. This optimization reduces the number of reduction interactions by eliminating the outer beta-reductions of the add combinator. The implementer SHOULD choose the approach that produces fewer total interactions. **(SHOULD)**

**Worked example: build_add(1, 1) using direct construction**

Direct construction builds `lambda f. lambda x. (f (f x))` via `(lambda f. lambda x. church(1) f (church(1) f x))`.

Sub-nets built in a single net via `encode_church_into`:
- church(1) for m (agents 0-2): CON_m_f (id=0), CON_m_x (id=1), CON_m_app (id=2)
- church(1) for n (agents 3-5): CON_n_f (id=3), CON_n_x (id=4), CON_n_app (id=5)

Combinator agents:
- CON_lam_f (id=6): outer lambda for f
- CON_lam_x (id=7): inner lambda for x
- CON_app_m (id=8): application `church(1) f ...` (applies m to f)
- CON_app_n (id=9): application `church(1) f x` (applies n to f)

Wiring:
- CON_lam_f.p0 = root (net.root = Some(AgentPort(6, 0)))
- CON_lam_f.p2 = CON_lam_x.p0 (body of outer lambda is inner lambda)
- DUP_f (id=10): duplicates f for two uses. CON_lam_f.p1 = DUP_f.p0
- DUP_f.p1 = CON_app_m.p1 (first copy of f -> argument to m)
- DUP_f.p2 = CON_app_n.p1 (second copy of f -> argument to n)
- CON_app_m.p0 = AgentPort(0, 0) [m_root] -- forms redex (principal <-> principal)
- CON_app_m.p2 = CON_lam_x.p2 (result of m(...) is body result)
- CON_app_n.p0 = AgentPort(3, 0) [n_root] -- forms redex (principal <-> principal)
- CON_app_n.p2 = CON_app_m.p1 would be wrong; instead:
- CON_lam_x.p1 = CON_app_n.p1b ... (x feeds innermost argument)

Note: The exact wiring for the direct construction approach is non-trivial because it involves threading `f` and `x` through two Church numeral sub-nets. The worked example above shows the general structure; the implementer MUST verify that the port connections produce exactly `church(2)` after `reduce_all`, using the roundtrip test `decode_nat(&net) == Some(2)`.

Initial redexes:
- CON_app_m.p0 <-> CON_m_f.p0 (beta-reduction: apply church(1) to f)
- CON_app_n.p0 <-> CON_n_f.p0 (beta-reduction: apply church(1) to f)

After reducing these two redexes (CON-CON annihilations), the net collapses to `lambda f. lambda x. f (f x)` = church(2). Total interactions: 2 (one per Church numeral application).

#### 4.3.2 Multiplication: build_mul(a, b)

The multiplication combinator `mul = lambda m. lambda n. lambda f. m (n f)` computes function composition: applying m to the composed function `(n f)`.

Construction steps for `build_mul(a, b)`:

1. Create a single `Net` and use `encode_church_into` to build church(a) and church(b) as sub-nets (avoiding ID collisions per R4b).
2. Create the mul combinator structure:
   - CON_lam_f: outer lambda abstraction for `f` (the composed function parameter)
   - CON_app_nf: application node for `(n f)` -- applies church(b) to f
   - CON_app_m: application node for `m (n f)` -- applies church(a) to the result of `(n f)`
3. Wire the combinator:
   - CON_lam_f.p1 (f binding) connects to CON_app_nf.p1 (f is the argument to n)
   - CON_lam_f.p2 (body) connects to CON_app_m.p2 (result of mul is result of m applied to (n f))
   - CON_app_nf.p0 connects to the principal port of church(b)'s root agent (applying n to f -- forms a redex)
   - CON_app_nf.p2 (result of (n f)) connects to CON_app_m.p1 (argument to m)
   - CON_app_m.p0 connects to the principal port of church(a)'s root agent (applying m to (n f) -- forms a redex)
4. Set `net.root = Some(AgentPort(CON_lam_f, 0))`

Key difference from addition: multiplication involves function composition (`m (n f)`) rather than sequential application. The DUP agents in church(a) will duplicate the entire `(n f)` sub-expression during reduction, producing O(a * b) intermediate agents (Profile B expansion phase).

**Worked example: build_mul(2, 2)**

Sub-nets:
- church(2) for m: agents m_lam_f (CON), m_lam_x (CON), m_app_0 (CON), m_app_1 (CON), m_dup_0 (DUP) -- 5 agents
- church(2) for n: agents n_lam_f (CON), n_lam_x (CON), n_app_0 (CON), n_app_1 (CON), n_dup_0 (DUP) -- 5 agents

Combinator: CON_lam_f, CON_app_nf, CON_app_m -- 3 agents

Total initial agents: 13. After reduction via `reduce_all`, result is church(4) with 9 agents.

Initial redexes (principal-to-principal connections):
- CON_app_nf.p0 <-> n_lam_f.p0 (beta-reduction: apply n to f)
- CON_app_m.p0 <-> m_lam_f.p0 (beta-reduction: apply m to (n f))

These initial redexes trigger a cascade of CON-DUP commutations (expansion) followed by CON-CON annihilations (collapse).

#### 4.3.3 Exponentiation: build_exp(base, exp)

The exponentiation combinator `exp = lambda m. lambda n. n m` is the simplest: it is just application of church(exp) to church(base).

Construction steps for `build_exp(base, exp)`:

1. Create a single `Net` and use `encode_church_into` to build church(base) and church(exp) as sub-nets.
2. Create one application CON agent: CON_app (applies n to m).
3. Wire:
   - CON_app.p0 connects to the principal port of church(exp)'s root agent (forms a redex)
   - CON_app.p1 connects to the principal port of church(base)'s root agent
   - CON_app.p2 = root observation point
4. Set `net.root = Some(AgentPort(CON_app, 2))` -- or wrap in an additional lambda if the result needs the standard Church numeral interface.

Note: The exact wiring depends on whether `build_exp` produces `n m` (a partially applied Church numeral) or wraps it in `lambda f. lambda x. (n m) f x`. The latter produces more redexes but outputs a standard Church numeral structure; the former is simpler. The implementer SHOULD use the approach that correctly produces `church(base^exp)` after `reduce_all`.

Initial redex: CON_app.p0 <-> church(exp)_root.p0 (single beta-reduction).

Exponentiation has deep sequential dependency chains: church(exp) applies church(base) exp times, and each application must complete before the next can proceed. This limits parallelism and approaches Profile C behavior for large exponents.

### 4.4 Decoding Algorithm

The decode algorithm uses only SPEC-02's public API: `net.agents[id as usize]` for agent lookup and `net.get_target(port) -> PortRef` for port traversal. Per SPEC-02, `get_target` returns `PortRef` (not `Option<PortRef>`), returning the sentinel `DISCONNECTED` (= `PortRef::FreePort(u32::MAX)`) for invalid or out-of-range ports.

Helper function used in pseudocode below:

```rust
/// Helper: look up an agent by ID. Returns None if the slot is empty or out of range.
fn get_agent(net: &Net, id: AgentId) -> Option<&Agent> {
    net.agents.get(id as usize).and_then(|slot| slot.as_ref())
}
```

Note: This helper is NOT part of SPEC-02's public API. It is a local utility for the encoding module. The implementer MAY inline it or add it as a private method.

```rust
use crate::net::{DISCONNECTED};

pub fn decode_nat(net: &Net) -> Option<u64> {
    // Must be in Normal Form
    if !net.redex_queue.is_empty() {
        return None;
    }

    // Step 1: Find outer lambda (lambda f) from root
    let root = net.root?;
    let lam_f = match root {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    let lam_f_agent = get_agent(net, lam_f)?;
    if lam_f_agent.symbol != Symbol::Con {
        return None;
    }

    // Step 2: Find inner lambda (lambda x) from lambda_f.p2
    let lam_f_p2_target = net.get_target(PortRef::AgentPort(lam_f, 2));
    if lam_f_p2_target == DISCONNECTED {
        return None;
    }
    let lam_x = match lam_f_p2_target {
        PortRef::AgentPort(id, 0) => id,
        _ => return None,
    };
    let lam_x_agent = get_agent(net, lam_x)?;
    if lam_x_agent.symbol != Symbol::Con {
        return None;
    }

    // Step 3: Check for n = 0 case
    // lambda_x.p1 connects to lambda_x.p2 (self-loop on auxiliaries)
    // and lambda_f.p1 connects to an ERA agent
    let f_target = net.get_target(PortRef::AgentPort(lam_f, 1));
    let x_bind = net.get_target(PortRef::AgentPort(lam_x, 1));
    let x_body = net.get_target(PortRef::AgentPort(lam_x, 2));

    if f_target == DISCONNECTED || x_bind == DISCONNECTED || x_body == DISCONNECTED {
        return None;
    }

    // Check self-loop: x_bind == lambda_x.p2 and x_body == lambda_x.p1
    if x_bind == PortRef::AgentPort(lam_x, 2)
        && x_body == PortRef::AgentPort(lam_x, 1)
    {
        // Verify ERA on f
        if let PortRef::AgentPort(era_id, 0) = f_target {
            let era_agent = get_agent(net, era_id)?;
            if era_agent.symbol == Symbol::Era {
                return Some(0);
            }
        }
        return None;
    }

    // Step 4: Walk application chain from lambda_x.p2
    // lambda_x.p2 connects to the outermost application's p2
    let mut count: u64 = 0;
    let mut current = PortRef::AgentPort(lam_x, 2);

    loop {
        let target = net.get_target(current);
        if target == DISCONNECTED {
            return None;
        }
        match target {
            PortRef::AgentPort(app_id, 2) => {
                let agent = get_agent(net, app_id)?;
                if agent.symbol != Symbol::Con {
                    return None;
                }
                count += 1;
                // Follow the application chain: next is app.p1
                // which connects to the previous application's p2
                // (or to lambda_x.p1 for the innermost application)
                current = PortRef::AgentPort(app_id, 1);
            }
            PortRef::AgentPort(id, port) if id == lam_x && port == 1 => {
                // Reached the x variable binding -- end of chain
                break;
            }
            _ => return None,
        }
    }

    Some(count)
}
```

Note: The exact traversal logic depends on the Normal Form topology. The implementer MUST verify the decoding algorithm against the port connection tables in Section 4.2. The key insight is that in a Church numeral Normal Form, the body of lambda_x (port p2 of CON_1) connects to the outermost application's result port (p2), and following each application's argument port (p1) leads to the next inner application's result port (p2), forming a chain of length n.

### 4.5 Overhead Profile Analysis

Church arithmetic nets exhibit **Profile B** behavior (SPEC-09, DISC-006 v2):

1. **Expansion phase:** When beta-reduction applies a Church numeral to its arguments, CON-DUP commutations create new agents. For `mul(a, b)`, the DUP agents in church(a) duplicate the entire church(b) sub-net, creating O(a * b) intermediate agents.

2. **Collapse phase:** After expansion, the duplicated sub-nets contain annihilation redexes (CON-CON) that reduce the net toward the Normal Form result.

3. **BSP implications:** The expansion phase creates many independent redexes (good for parallelism). The collapse phase may create sequential dependencies if borders cut through the collapsing chains. Larger operands produce more redexes and better distribution potential.

| Operation | Initial Agents | Peak Agents (approx.) | Final Agents | Interactions | Profile |
|-----------|---------------|----------------------|--------------|-------------|---------|
| add(n, n) | O(n) | O(n) | O(n) | O(n) | A/B |
| mul(n, n) | O(n) | O(n^2) | O(n^2) | O(n^2) | B |
| exp(2, n) | O(n) | O(2^n) | O(2^n) | O(2^n) | B/C |

---

## 5. Rationale

### 5.1 Why Church Encoding

Church encoding is the canonical encoding of natural numbers in lambda calculus (Church 1936) and maps directly to IC nets via the standard lambda-to-inet translation (Lafont 1997, Section 4). It uses only the three IC symbols (CON, DUP, ERA) without requiring additional agent types or extensions. Alternative encodings (Scott, Parigot, Bohm-Berarducci) would require either: additional symbols (beyond IC's 3), more complex reduction strategies, or loss of the direct correspondence with Lafont's universality proof. Church encoding is the simplest choice that demonstrates Relativist's ability to perform real computation while remaining theoretically grounded.

### 5.2 Why a Separate `compute` Subcommand

The `compute` subcommand combines three operations (encode, reduce, decode) into a single user-facing command. An alternative would be to extend `generate` (to create arithmetic nets) and `reduce` (to decode results). However, the combined subcommand provides a better demonstration experience: the user types one command and sees the complete pipeline from arithmetic expression to numeric result. This is essential for the TCC defense where the professor needs to see practical computation, not just abstract graph reduction.

### 5.3 Why Factorial is a Stretch Goal

Computing factorial(n) = n * (n-1) * ... * 1 requires recursion, which in lambda calculus requires the Y-combinator (or a similar fixed-point combinator). Encoding the Y-combinator as an IC net introduces self-referential structures that complicate the encoding significantly and risk non-termination if the encoding is incorrect. Addition, multiplication, and exponentiation are sufficient to demonstrate practical distributed computation for the TCC. Factorial MAY be added later if time permits.

### 5.4 Why Not Scott Encoding

Scott encoding represents natural numbers as case-discrimination functions: `scott(0) = lambda z. lambda s. z` and `scott(n+1) = lambda z. lambda s. s n`. While Scott numerals enable O(1) predecessor (unlike Church numerals, where predecessor is notoriously complex), they require pattern matching that does not map as cleanly to the 3-symbol IC system. Church encoding's advantage is simplicity: arithmetic operations are simple compositions with well-understood interaction profiles.

---

## 6. Haskell Prototype Reference

### 6.1 What the prototype provides

The Haskell prototype does NOT have any encoding layer. All benchmark nets are hand-constructed IC nets (ERA-ERA pairs, CON-DUP expansion pairs, dual trees) without semantic meaning as computations. There is no mechanism to encode numbers, arithmetic, or lambda terms.

### 6.2 What Relativist adds and why

The encoding module is entirely new functionality that transforms Relativist from an abstract graph reducer into a demonstrable computation engine. This addresses a critical gap for the TCC: the ability to show that distributed IC net reduction computes something meaningful (e.g., "500 + 500 = 1000 computed across 4 workers with 3.17x speedup").

---

## 7. Test Requirements

Test labels use the prefix `ET-` (Encoding Tests) to avoid collision with SPEC-01 invariant labels T1-T7.

**ET-1.** Structure test: `encode_nat(0)` MUST produce exactly 2 CON + 1 ERA agents with the port connections specified in Section 4.2 (Church(0) table). **(MUST)**

**ET-2.** Structure test: `encode_nat(1)` MUST produce exactly 3 CON + 0 DUP + 0 ERA agents with the port connections specified in Section 4.2 (Church(1) table). **(MUST)**

**ET-3.** Structure test: `encode_nat(2)` MUST produce exactly 4 CON + 1 DUP agents with the port connections specified in Section 4.2 (Church(2) table). **(MUST)**

**ET-4.** Normal Form test: for n in {0, 1, 2, 5, 10, 100}, `encode_nat(n)` MUST produce a net with zero redexes. **(MUST)**

**ET-5.** Roundtrip test: for n in {0, 1, 2, 3, 5, 10, 50, 100}, `decode_nat(&encode_nat(n))` MUST return `Some(n)`. **(MUST)**

**ET-6.** Addition correctness: for (a, b) in {(0,0), (0,1), (1,0), (1,1), (2,3), (10,20), (50,50), (100,100)}: **(MUST)**

```rust
let mut net = build_add(a, b);
reduce_all(&mut net);  // SPEC-03, R13: reduce_all(net: &mut Net) -> ReductionStats
assert_eq!(decode_nat(&net), Some(a + b));
```

**ET-7.** Multiplication correctness: for (a, b) in {(0,1), (1,0), (1,1), (2,3), (5,5), (10,10)}: **(MUST)**

```rust
let mut net = build_mul(a, b);
reduce_all(&mut net);
assert_eq!(decode_nat(&net), Some(a * b));
```

**ET-8.** Exponentiation correctness: for (a, b) in {(2,0), (2,1), (2,3), (2,8), (3,3)}: **(MUST)**

```rust
let mut net = build_exp(a, b);
reduce_all(&mut net);
assert_eq!(decode_nat(&net), Some(a.pow(b as u32)));
```

**ET-9.** Invariant preservation: for all encodings and arithmetic operations in ET-1 through ET-8, the generated nets MUST satisfy invariants T1-T7 from SPEC-01. Verification via `net.assert_all_invariants()` (SPEC-02, R20) in debug mode. **(MUST)**

**ET-10.** Property test (proptest): for random a, b in [0, 100]: **(SHOULD)**

```rust
let mut net = build_add(a, b);
reduce_all(&mut net);
assert_eq!(decode_nat(&net), Some(a + b));
```

**ET-11.** Distributed correctness (Fundamental Property): for (a, b) = (50, 50) and k in {1, 2, 4}: **(MUST)**

```rust
// Local reduction
let mut local_net = build_add(50, 50);
reduce_all(&mut local_net);
let local_result = decode_nat(&local_net);

// Distributed reduction
let distributed_net = run_grid(build_add(50, 50), k);
let distributed_result = decode_nat(&distributed_net);

assert_eq!(local_result, distributed_result);
```

**ET-12.** Decode rejection: `decode_nat` MUST return `None` for nets that are not Church numerals -- e.g., `ep_annihilation(5)`, an empty net, and a net with non-zero redexes. **(MUST)**

---

## 8. Arithmetic Benchmark Scenarios for Distributed Evaluation

This section defines the specific arithmetic problems to be tested in distributed mode, informed by the overhead profile analysis (SPEC-09, DISC-006 v2, DISC-009) and the batch nature of the current system.

### 8.1 Design Rationale

Church arithmetic nets exhibit Profile B behavior (Section 4.5): expansion via CON-DUP followed by collapse via annihilation. The key factor determining distribution benefit is whether total interactions exceed the break-even threshold (estimated at 5K-10K redexes per worker for optimized Rust, per DISC-006 v2 and ARG-004).

Since the Relativist v1 operates in batch mode (DISC-009, Section 4.3), streaming optimizations from Mackie and Sato (REF-015) are not available. All encoding uses standard Church combinators where operations process all data before returning results.

### 8.2 Benchmark Table

| Benchmark ID | Expression | Interactions (approx.) | Profile | Workers | Purpose |
|---|---|---|---|---|---|
| ARITH-ADD-S | add(50, 50) | ~100 | A/B | 1,2 | Baseline: too small for distribution benefit |
| ARITH-ADD-M | add(500, 500) | ~1,000 | A/B | 1,2,4 | Near break-even threshold |
| ARITH-ADD-L | add(5000, 5000) | ~10,000 | A/B | 1,2,4,8 | Above break-even: should show speedup |
| ARITH-ADD-XL | add(10000, 10000) | ~20,000 | A/B | 1,2,4,8 | Large addition: strong distribution candidate |
| ARITH-MUL-S | mul(5, 5) | ~25 | B | 1,2 | Baseline: minimal expansion |
| ARITH-MUL-M | mul(30, 30) | ~900 | B | 1,2,4 | Near break-even with expansion phase |
| ARITH-MUL-L | mul(100, 100) | ~10,000 | B | 1,2,4,8 | Significant expansion/collapse cycle |
| ARITH-MUL-XL | mul(200, 200) | ~40,000 | B | 1,2,4,8 | Large multiplication: tests Profile B limits |
| ARITH-EXP-S | exp(2, 3) | ~8 | B/C | 1,2 | Baseline: tiny exponentiation |
| ARITH-EXP-M | exp(2, 8) | ~256 | B/C | 1,2,4 | Moderate: exponential growth begins |
| ARITH-EXP-L | exp(2, 12) | ~4,096 | B/C | 1,2,4,8 | Near threshold: tests deep dependency chains |
| ARITH-EXP-XL | exp(2, 16) | ~65,536 | B/C | 1,2,4,8 | Large exponentiation: high interaction count but deep dependencies |

### 8.3 Verification Property

For every benchmark point, the fundamental property MUST hold:

```rust
// Local baseline
let mut local_net = build_op(a, b);
reduce_all(&mut local_net);
let local_result = decode_nat(&local_net);

// Distributed
let distributed_net = run_grid(build_op(a, b), k);
let distributed_result = decode_nat(&distributed_net);

assert_eq!(local_result, distributed_result);
```

Zero correctness failures across all arithmetic benchmarks constitutes the success criterion. This extends SPEC-09's verification to arithmetic workloads.

### 8.4 Metrics Collected

Per benchmark point (same as SPEC-09, Section 3.4):
- **Correctness:** Sequential result == distributed result (via decode_nat comparison)
- **Speedup:** T_sequential / T_distributed
- **MIPS:** Total interactions / total time
- **Overhead ratio:** T_overhead / T_total
- **Rounds:** Number of BSP rounds required
- **Peak agents:** Maximum agent count during reduction (captures expansion phase)

### 8.5 Expected Behavior by Operation Type

**Addition (ARITH-ADD):** O(a+b) interactions, mostly annihilation (CON-CON) after initial beta-reductions. Expansion is minimal. Approaches Profile A behavior for large operands. Expected: speedup above break-even for ADD-L and ADD-XL.

**Multiplication (ARITH-MUL):** O(a*b) interactions with significant CON-DUP expansion phase (DUP agents in church(a) duplicate the entire church(b) sub-net). This is the canonical Profile B benchmark for arithmetic. Expected: speedup depends on topology of emergent border redexes; multiple BSP rounds likely.

**Exponentiation (ARITH-EXP):** O(base^exp) interactions with deep dependency chains. Even though the interaction count is large for exp(2,16), the reduction has inherent sequential structure from nested function composition. Expected: limited or negative speedup due to depth dependencies, approaching Profile C behavior.

### 8.6 Limitations of Church Arithmetic Benchmarks

1. **Church encoding is unary:** church(n) uses O(n) agents. This limits practical operand sizes (n > 10,000 consumes significant memory).
2. **Batch encoding only:** The current encoding does not use streaming patterns from Mackie and Sato (REF-015). Streaming would enable more parallelism within rounds but requires fundamental protocol changes (see DISC-009, Section 6).
3. **Not representative of HVM2-class workloads:** HVM2 uses native numeric types (NUM, OPE, SWI) that avoid Church encoding overhead entirely. The arithmetic benchmarks validate correctness and distribution mechanics, not raw performance.
4. **Interaction counts are approximate:** The exact interaction count depends on the encoding strategy chosen (direct construction vs. combinator composition, per Section 4.3.1). Values in the table assume standard combinator composition.

---

## 9. Open Questions

0. **Streaming encoding.** Mackie and Sato (REF-015) demonstrated that streaming operations (which release partial results incrementally) enable significantly more parallelism than batch operations (which process all data before returning). The current Church arithmetic encoding is strictly batch. A streaming variant (e.g., `add(S(x), y) = S(add(x, y))` instead of `add(S(x), y) = add(x, S(y))`) would create new redexes incrementally, potentially enabling better distribution. However, exploiting this in the Relativist would also require protocol changes (removing BSP barrier, supporting incremental merge). See DISC-009 for full analysis. **(Does NOT block implementation. Future work.)**

1. **Factorial and recursion.** Encoding the Y-combinator as an IC net enables recursive computations (factorial, Fibonacci, etc.). This is well-studied in the literature (Lamping 1990, Asperti & Guerrini 1998) but adds significant complexity. Defer to post-v1 if time does not permit. **(Does NOT block implementation.)**

2. **Boolean encoding.** Church booleans (`true = lambda x. lambda y. x`, `false = lambda x. lambda y. y`) and conditionals would enable predicate-based computations (e.g., primality testing). This is straightforward but expands scope beyond arithmetic. **(Does NOT block implementation.)**

3. **Maximum practical operand size.** Church encoding is inherently unary -- church(n) has O(n) agents. For n > 10,000, the net may consume significant memory. The practical limit depends on available RAM and the target reduction time. Empirical measurement during benchmarking will establish the useful range. **(Does NOT block implementation.)**

4. **Predecessor and subtraction.** Church predecessor is notoriously complex (Kleene's trick). If needed, consider switching to Parigot numerals for operations that require predecessor, while keeping Church numerals for addition, multiplication, and exponentiation. **(Does NOT block implementation.)**

5. **Direct construction vs. combinator composition.** Section 4.3.1 notes that `build_add` MAY construct the expanded term directly instead of composing the full add combinator with Church sub-nets. The direct approach reduces unnecessary beta-reductions but increases code complexity. The implementer should benchmark both approaches and choose the one with fewer total interactions. **(Does NOT block implementation.)**

---

## 10. Cross-References (Specs Affected)

This section documents minimal updates needed in other specs. These updates are NOT part of SPEC-14 itself but are tracked here for the implementer.

1. **SPEC-00 (Glossary):** Add entries for Church Numeral, Encoding, Decoding (Readback), Arithmetic Net, Combinator.
2. **SPEC-12 (User I/O):** Add `ChurchNat`, `ChurchAdd`, `ChurchMul` to `ExampleNet` enum (R33). Add `Compute` to CLI subcommand list.
3. **SPEC-13 (System Architecture):** Add `encoding/` to module structure (R5). Add `Compute(ComputeArgs)` to `Cli` enum (R43). Note: `encoding/` is Core Layer -- it MUST NOT depend on tokio or I/O.
4. **SPEC-09 (Benchmarks):** Church arithmetic nets provide new Profile B benchmark scenarios. Addition of `ChurchAdd` and `ChurchMul` benchmarks SHOULD be considered for the scaling curve analysis.
