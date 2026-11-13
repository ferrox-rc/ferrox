# Contributing to ferrox

Thank you for your interest in contributing to `ferrox`! This document details our Git workflow, branching conventions, and development standards.

---

## 1. Branching Strategy

Our repository uses a structured Git branching workflow based on folder-style namespacing:

- **`main`**: Production and official stable release tags only.
- **`dev`**: Primary integration branch for active development.
- **Dedicated Task Branches**: All work (features, bug fixes, refactoring, docs) must be developed on a dedicated branch created from `dev`.

### Branch Naming Convention

Branch names must follow the format: `<prefix>/<kebab-case-description>`

| Prefix | Purpose | Example |
| :--- | :--- | :--- |
| `feat/` | New user-facing feature or protocol support | `feat/crsf-v3-telemetry` |
| `fix/` | Bug fixes or hardware quirk corrections | `fix/spi-dma-transfer-stall` |
| `refactor/` | Internal restructuring, concurrency, or code cleanup | `refactor/rtic-concurrency-model` |
| `docs/` | Documentation additions or updates | `docs/architecture-diagrams` |
| `perf/` | Flash size, SRAM reduction, or timing optimizations | `perf/lut-spline-interpolation` |
| `test/` | Adding test cases, harnesses, or CI pipelines | `test/channel-mixer-harness` |
| `chore/` | Toolchain updates, dependency bumps, linker scripts | `chore/update-cortex-m-hal` |

### Starting New Work

Always ensure you are branched from an up-to-date `dev`:

```bash
git checkout dev
git pull origin dev
git checkout -b <prefix>/<short-description>
```

---

## 2. Commit Standards

We adhere to [Conventional Commits](https://www.conventionalcommits.org/):

- `feat: add Catmull-Rom spline throttle curve interpolation`
- `fix: correct EXTI2 flag clear sequence in RF ISR`
- `refactor: adopt lock-free atomic double-buffering for channels`
- `docs: update concurrency model in README`

---

## 3. Firmware Design Principles

- **`no_std` Bare-Metal:** No heap allocations, no dynamic sizing at runtime.
- **Deterministic Latency:** RF packet transmission (`TIM16`) and critical interrupts must not be blocked by non-essential tasks.
- **Resource Footprint:** Maintain awareness of Flash (<128 KB) and SRAM (<16 KB) utilization.
- **Board Isolation:** `ferrox` is a multi-board monorepo. Never run a raw `cargo build --workspace --release` on bare-metal targets because Cargo will unify all optional features across crates, compiling conflicting board definitions simultaneously. Always build each board target crate individually.

---

## 4. Testing & Verification Standards

To guarantee mathematical precision and prevent regressions across protocol state machines, all pull requests and contributions must satisfy the **Dual Verification Requirement**:

### 1. Run Host Unit Tests
Execute automated unit tests locally on your host development environment across target board profiles:
```bash
# Test key board profiles:
cargo test -p ferrox-core --lib --features board-i6x --target x86_64-unknown-linux-gnu -- --test-threads=1
cargo test -p ferrox-core --lib --features board-pocket --target x86_64-unknown-linux-gnu -- --test-threads=1
cargo test -p ferrox-core --lib --features board-t12 --target x86_64-unknown-linux-gnu -- --test-threads=1
cargo test -p ferrox-core --lib --features board-x9d --target x86_64-unknown-linux-gnu -- --test-threads=1
cargo test -p ferrox-core --lib --features board-at9spro --target x86_64-unknown-linux-gnu -- --test-threads=1
```
All unit tests must pass with 0 failures before opening a pull request. When introducing new mathematical formulas, mixing modes, or communication protocols, include corresponding unit tests in the appropriate module.

### 2. Verify Bare-Metal Builds
Verify that the firmware compiles cleanly across both Cortex-M0 and Cortex-M4F targets without errors or compiler warnings:
```bash
# Cortex-M0 Targets (FlySky):
cargo build-i6s
cargo build-i6x

# Cortex-M4F Targets (RadioMaster, Jumper, FrSky, RadioLink):
cargo build-pocket
cargo build-pocket-sd
cargo build-t12
cargo build-x9d
cargo build-x9e
cargo build-at9spro
```

For comprehensive details on our dual-target harness architecture, peripheral mocking, and adversarial verification methodology, refer to the **[Testing Methodology & Verification Guide](docs/TESTING.md)**.


