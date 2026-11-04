# Implementation Plan: Ferrox HAL & Multi-Transmitter Abstraction

This document defines the roadmap and architecture to transition `ferrox` from FlySky-specific firmware into a multi-transmitter, memory-safe Rust radio operating system.

---

## 1. Architectural Strategy

### A. The 3-Layer System
```
┌────────────────────────────────────────────────────────┐
│                      ferrox-core                       │
│  - 100% Platform-Agnostic, no MCU PACs, tests on host │
│  - Mathematical pipelines: Matrix mixer, curves, trims│
│  - High-rate FlightPipeline & 30Hz BackgroundIdle      │
│  - Protocol engines: CRSF framing, AFHDS 2A, Vario     │
│  - UI: MonoFramebuffer (128x64 fast primitives)        │
├────────────────────────────────────────────────────────┤
│                       HAL Traits                       │
│  ToneOutput, StorageBackend, DisplayDriver, RadioHal   │
├───────────────────────────┬────────────────────────────┤
│      ferrox-stm32f0       │       ferrox-stm32f4       │
│  (F072: i6X, i6S)         │  (F407: Pocket, TX12, etc.)│
├───────────────────────────┼────────────────────────────┤
│      boards/ferrox-i6x    │    boards/ferrox-pocket    │
│      boards/ferrox-i6s    │    boards/ferrox-tx12      │
└───────────────────────────┴────────────────────────────┘
```

### B. Key Architectural Decisions
1. **Unify Runtime First**: Eliminate the 1,100 lines of duplicated flight loop and background idle state machine between `ferrox-i6x` and `ferrox-i6s` by moving them into `ferrox-core::runtime`.
2. **Shared MCU Crates**: Do not duplicate F0 peripheral drivers into `boards/ferrox-i6x` and `boards/ferrox-i6s`. Both share an STM32F072 hardware crate.
3. **Storage Abstraction**: Abstract storage at the slot level (`load_radio_slot`, `save_radio_slot`, `load_model_slot`, `save_model_slot`) so EEPROM targets (i6X/i6S) can use 64B page writes while STM32F4 targets (Pocket, TX12) can use Flash sectors or SD card files.
4. **Fast Display Buffer**: Retain the `1024`-byte `MonoFramebuffer` in `ferrox-core` with page-aligned character and horizontal line drawing routines, decoupling only the bus flush (8-bit parallel vs SPI).

---

## 2. Execution Phases

### Phase 1: Unify Runtime (`FlightPipeline` & `BackgroundIdleManager`)
- Unify `FlightPipeline`, `BackgroundIdleManager`, and `run_preflight_check` into `ferrox-core::runtime`.
- Parameterize switch counts, pot center detection, and idle overlay rendering using `BoardProfile` and board hooks.
- Shrink `boards/ferrox-i6x/src/main.rs` and `boards/ferrox-i6s/src/main.rs` down to clean hardware initialization and loop entry.

### Phase 2: Decouple Buzzer, Display, and Storage Hardware Trait Boundaries
- **Buzzer**: Introduce `ToneOutput` trait (`set_freq(u16)`, `off()`). Move `TIM1` PWM setup out of core.
- **Display**: Split `St7567` into `MonoFramebuffer` (pure graphics) and `DisplayFlush` trait (bus execution).
- **Storage**: Move I2C peripheral bus operations behind a slot/block `StorageBackend` trait.

### Phase 3: Extract `crates/ferrox-stm32f0`
- Move shared F072 drivers (clocks, watchdog, time, USB PMA, A7105 SPI/TIM16, TIM15 trainer) into `crates/ferrox-stm32f0`.
- Remove `stm32f0xx-hal` from `ferrox-core/Cargo.toml`.
- Enable building `ferrox-core` natively on `x86_64-unknown-linux-gnu` for unit and UI regression tests.

### Phase 4: Target Radios Integration (e.g. RadioMaster Pocket)
- Implement `crates/ferrox-stm32f4` for STM32F407.
- Implement SPI driver for the Pocket's ST7567 display.
- Map gimbals, rotary navigation wheel, buttons, and internal ExpressLRS CRSF UART.
