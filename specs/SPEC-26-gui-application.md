# SPEC-26: GUI Desktop Application

**Status:** Draft
**Depends on:** SPEC-07 (Configuration and CLI), SPEC-09 (Benchmark Suite), SPEC-13 (System Architecture)
**ROADMAP items:** 2.39 (GUI Desktop Application)
**Briefings consumed:** BRIEF-20260415-v2-tier5-codebase (Section 2.39: workspace restructure, Tauri command layer, screen map)

---

## 1. Purpose

This spec defines the Relativist GUI — a Tauri v2 desktop application that wraps the existing CLI functionality in a graphical interface. The GUI enables users to generate, inspect, reduce, and benchmark interaction combinator nets without memorizing CLI flags, and to manage coordinator/worker grid operations visually.

The GUI requires a Cargo workspace restructure: the current single crate becomes `relativist-core` (library), `relativist-cli` (binary), and `relativist-gui` (Tauri app). This restructure is the architectural prerequisite and is specified first.

**No IC-theoretic invariants are affected.** The GUI calls the same library functions as the CLI. This spec focuses entirely on architecture, user interface, and integration.

---

## 2. Definitions

Terms defined in SPEC-00, SPEC-07, SPEC-09, and SPEC-13 are used without redefinition. Terms introduced in this spec:

| Term | Definition |
|------|-----------|
| **Cargo Workspace** | A Rust project organization where multiple crates share a root `Cargo.toml` with `[workspace]` configuration. Crates build independently but share a target directory and lockfile. |
| **relativist-core** | The library crate containing all existing functionality: `net/`, `reduction/`, `partition/`, `merge/`, `protocol/`, `security/`, `observability/`, `io/`, `encoding/`, `bench/`, `config`, `commands`, `coordinator`, `worker`. |
| **relativist-cli** | A thin binary crate that imports `relativist-core` and provides the CLI entry point (current `main.rs` functionality). |
| **relativist-gui** | A Tauri v2 application crate containing the Rust backend (`src-tauri/`) and the Svelte web frontend (`ui/`). |
| **Tauri Command** | A Rust function annotated with `#[tauri::command]` that the web frontend can invoke via Tauri's IPC bridge. Each command wraps a `relativist-core` function. |
| **Tauri Event** | A message emitted from the Rust backend to the web frontend via `tauri::Emitter::emit()`. Used for real-time progress updates (round completion, worker status, benchmark progress). |
| **Screen** | A distinct view in the GUI, corresponding to one or more CLI subcommands. Each screen is a Svelte component with its own route. |

---

## 3. Requirements

### 3.1 Workspace Restructure

**R1.** The Relativist repository MUST be restructured as a Cargo workspace with the following layout:
```
relativist/                  (workspace root)
├── Cargo.toml               ([workspace] manifest)
├── relativist-core/
│   ├── Cargo.toml
│   └── src/                 (all current src/ files)
├── relativist-cli/
│   ├── Cargo.toml
│   └── src/main.rs
└── relativist-gui/
    ├── Cargo.toml
    ├── src-tauri/src/
    │   ├── main.rs
    │   └── commands.rs
    ├── ui/
    │   ├── package.json
    │   ├── src/
    │   └── vite.config.ts
    └── tauri.conf.json
```
**(MUST)**

**R2.** The workspace root `Cargo.toml` MUST define:
```toml
[workspace]
members = ["relativist-core", "relativist-cli", "relativist-gui"]
resolver = "2"
```
**(MUST)**

**R3.** `relativist-core` MUST contain all existing source files from `src/` with no semantic modifications. All `use crate::*` imports MUST remain valid within the core crate. **(MUST)**

**R4.** `relativist-core`'s `Cargo.toml` MUST inherit all current dependencies from the root `Cargo.toml`. Feature flags (`tls`, `metrics`, `otel`, `full`) MUST be preserved. **(MUST)**

**R5.** `relativist-cli`'s `src/main.rs` MUST delegate to `relativist_core::commands::run()` (or equivalent entry point). The CLI binary MUST behave identically to the pre-restructure binary. **(MUST)**

**R6.** All 690 tests MUST be in `relativist-core` and MUST pass after restructure. The `[[bench]]` section MUST move to `relativist-core/Cargo.toml`. **(MUST)**

**R7.** The workspace MUST support `cargo test --workspace` to run all tests across all crates. **(MUST)**

### 3.2 Tauri Application Scaffold

**R8.** `relativist-gui` MUST use Tauri v2 (stable release) with Svelte as the frontend framework. **(MUST)**

**R9.** The Tauri configuration (`tauri.conf.json`) MUST specify:
- App name: "Relativist"
- Window title: "Relativist — Distributed IC Reducer"
- Default window size: 1200×800
- Minimum window size: 800×600
**(MUST)**

**R10.** The Tauri backend MUST depend on `relativist-core` as a path dependency:
```toml
[dependencies]
relativist-core = { path = "../relativist-core" }
```
**(MUST)**

### 3.3 Tauri Commands

**R11.** Each CLI subcommand MUST have a corresponding Tauri command in `src-tauri/src/commands.rs`. Commands MUST wrap `relativist-core` functions and return JSON-serializable results. **(MUST)**

**R12.** The following Tauri commands MUST be implemented:

| Command | Core Function | Input | Output |
|---------|--------------|-------|--------|
| `generate_net` | `io::generators::generate()` | `{ example: String, size: u32, output: String }` | `{ agents: u32, redexes: u32, path: String }` |
| `inspect_net` | Net deserialization + stats | `{ path: String }` | `{ agents: u32, live_agents: u32, redexes: u32, symbols: {con: u32, dup: u32, era: u32}, is_normal_form: bool }` |
| `reduce_net` | `reduction::reduce_all()` | `{ path: String, output: String }` | `{ interactions: u64, by_rule: [u64; 6], is_normal_form: bool }` |
| `run_local_grid` | `merge::run_grid()` | `{ path: String, workers: u32, strict_bsp: bool }` | `{ rounds: u32, interactions: u64, speedup: f64, ... }` |
| `start_coordinator` | `coordinator::run()` | `{ bind: String, workers: u32, token: Option<String> }` | `{ status: String }` (event-based updates) |
| `start_worker` | `worker::run()` | `{ coordinator: String, token: Option<String> }` | `{ status: String }` (event-based updates) |
| `compute_arithmetic` | `encoding::*` + `reduction::reduce_all()` | `{ op: String, a: u64, b: u64 }` | `{ result: u64, interactions: u64 }` |
| `run_benchmarks` | `bench::run_suite()` | `{ benchmarks: Vec<String>, sizes: Vec<u32>, workers: Vec<u32> }` | Stream of progress events + final results |

**(MUST)**

**R13.** Long-running Tauri commands (`run_local_grid`, `start_coordinator`, `start_worker`, `run_benchmarks`) MUST emit progress events via `tauri::Emitter::emit()`. Events MUST include:
- `round-complete`: round number, interactions, timing.
- `worker-status`: worker ID, state (connected/reducing/done).
- `benchmark-progress`: benchmark name, size, worker count, status (running/complete), result.
**(MUST)**

**R14.** Tauri commands MUST run on a separate Tokio runtime from the Tauri event loop, to prevent blocking the UI. The `#[tauri::command]` functions MUST be `async` and use `tokio::spawn` for long-running operations. **(MUST)**

### 3.4 GUI Screens

**R15.** The GUI MUST implement the following screens, each as a Svelte route component:

| Screen | Route | CLI Equivalent | Key UI Elements |
|--------|-------|----------------|-----------------|
| Dashboard | `/` | — | Version card, recent runs list, quick action buttons |
| Generate | `/generate` | `generate` | Net type dropdown, size input, output picker, stats preview |
| Inspect | `/inspect` | `inspect` | File picker (drag-and-drop), stats cards, symbol breakdown |
| Reduce | `/reduce` | `reduce` | Input picker, progress bar, before/after stats |
| Local Grid | `/grid/local` | `local` | Worker count slider, input picker, round table, speedup chart |
| Coordinator | `/grid/coordinator` | `coordinator` | Bind address, worker count, token display, connected workers panel |
| Worker | `/grid/worker` | `worker` | Coordinator address, connection status, reduction progress |
| Calculator | `/calculator` | `compute` | Operation picker, number inputs, result display |
| Benchmarks | `/benchmarks` | `bench` | Config panel, progress bars, results table, chart, CSV export |
| Network | `/network` | — | Tailscale status (if 2.37), coordinator discovery |
| Settings | `/settings` | — | Version, update check, paths, log level |

**(MUST)**

**R16.** The Dashboard screen MUST show:
- Current Relativist version.
- Quick actions: Generate, Reduce, Start Grid.
- Last 5 runs with type (generate/reduce/grid/bench), timestamp, and status.
**(MUST)**

**R17.** The Benchmarks screen MUST support:
- Selecting which benchmarks to run (checkboxes).
- Configuring sizes and worker counts.
- Live progress bars during execution.
- Results table with columns: benchmark, size, workers, mode, interactions, time, speedup.
- CSV export button.
**(MUST)**

**R18.** The Local Grid screen MUST display a round-by-round progress table during execution:
- Columns: round, interactions, time, border redexes, merge time.
- A speedup chart (line graph) showing speedup vs. worker count.
**(MUST)**

### 3.5 Frontend Technology

**R19.** The frontend MUST use Svelte (version 4+) with Vite as the build tool. **(MUST)**

**R20.** The frontend bundle size MUST be under 500 KB (gzipped) for initial load. **(MUST)**

**R21.** The frontend MUST NOT bundle a chart library larger than 50 KB. Recommended: Chart.js (~60 KB but tree-shakeable to <30 KB) or uPlot (~30 KB). **(SHOULD)**

**R22.** The frontend MUST support dark and light themes. The default MUST follow the OS preference. **(MUST)**

### 3.6 Installer and Distribution

**R23.** The Tauri build MUST generate platform-specific installers:
- Windows: `.msi` installer (via WiX).
- macOS: `.dmg` disk image.
- Linux: `.deb` package and AppImage.
**(MUST)**

**R24.** The Windows installer MUST add Relativist to the Start Menu and optionally to PATH. **(MUST)**

**R25.** The Tauri build SHOULD support code signing for Windows (Authenticode) and macOS (notarization). This eliminates SmartScreen warnings (ROADMAP 2.38). **(SHOULD)**

**R26.** The `relativist-cli` binary MUST continue to be distributed independently (via `cargo install`, GitHub releases, and platform package managers). The GUI is an addition, not a replacement. **(MUST)**

---

## 4. Non-Goals

**NG1.** Replacing the CLI. The CLI remains the primary interface for scripting, CI/CD, and headless environments. The GUI is complementary.

**NG2.** Web-based dashboard. The Tauri app uses a local webview, not a hosted web server. A separate web dashboard for remote monitoring (via `observability/http.rs`) is a different feature.

**NG3.** Mobile support. Tauri v2 supports mobile targets, but Relativist's compute requirements make mobile impractical. Desktop only.

**NG4.** IC net visualization. Graphviz-style graph rendering of nets is a separate feature (ROADMAP 2.13, Tier 6 FUTURE). The GUI shows statistics, not graph layouts.

---

## 5. Implementation Phases

| Phase | Duration | Deliverable | Screen Count |
|-------|----------|-------------|--------------|
| **1. Skeleton** | 1-2 weeks | Workspace restructure + Tauri scaffold + Dashboard + Generate + Inspect + Reduce | 4 |
| **2. Grid** | 2-3 weeks | Local Grid + Coordinator + Worker + real-time events | 3 |
| **3. Benchmarks** | 1-2 weeks | Benchmarks screen with charts + CSV export | 1 |
| **4. Network** | 1 week | Network screen (Tailscale status), Calculator | 2 |
| **5. Polish** | 2-3 weeks | Installer generation, code signing, themes, Settings | 1 |

**MVP (Phases 1-2):** 7 screens, ~3-5 weeks. Proves architecture and covers core functionality.

---

## 6. Test Strategy

### 6.1 Workspace Tests

**T1. All 690 tests pass after restructure.**
- `cargo test -p relativist-core` MUST return 690+ passing tests.

**T2. CLI binary equivalence.**
- Run `relativist-cli generate --example ep-annihilation --size 100 --output test.bin`.
  Verify output matches pre-restructure binary.

**T3. Workspace-wide test.**
- `cargo test --workspace` MUST pass (core + cli + gui backend tests).

### 6.2 Tauri Command Tests

**T4. Generate command round-trip.**
- Call `generate_net("ep_annihilation", 100, "test.bin")`.
  Verify file exists, agents=200, redexes=100.

**T5. Reduce command.**
- Generate net, then call `reduce_net("test.bin", "reduced.bin")`.
  Verify `is_normal_form = true`, interactions > 0.

**T6. Local grid command.**
- Call `run_local_grid("test.bin", 2, false)`.
  Verify rounds > 0, result matches sequential reduction.

**T7. Compute arithmetic.**
- Call `compute_arithmetic("add", 3, 5)`.
  Verify result = 8.

### 6.3 Frontend Tests

**T8. Screen navigation.**
- Verify all 11 routes render without JavaScript errors.

**T9. Dark/light theme toggle.**
- Toggle theme, verify CSS variables update.

**T10. CSV export.**
- Run benchmark, click export. Verify CSV file has correct headers and data.

---

## 7. Open Questions

**Q1. Svelte vs. React.** The ROADMAP recommends Svelte for smaller bundle size. If the developer has more React experience, React is acceptable. Both have official Tauri templates.

**Q2. System tray.** Should the coordinator/worker run as background processes with a system tray icon? Tauri v2 supports system tray. Deferred to Phase 5 (polish).

**Q3. Auto-updater.** Tauri v2 has a built-in updater. Should the GUI check for updates automatically? If yes, it needs a release server (GitHub Releases works). Deferred to Phase 5.

**Q4. Benchmark result persistence.** Should the GUI store benchmark results locally for comparison across runs? A simple SQLite database or JSON file in `~/.relativist/results/` would suffice. Not required for MVP.

**Q5. Accessibility.** The web frontend should follow WCAG 2.1 AA guidelines (keyboard navigation, ARIA labels, color contrast). This adds complexity to each screen. Should be addressed in Phase 5 (polish).
