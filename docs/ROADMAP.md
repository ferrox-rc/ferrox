# Implementation Roadmap & Phase Status

This document tracks the phased implementation, historical milestones, and ongoing development of the unified `ferrox` operating system across all supported transmitter families.

---

## Current Firmware Footprint (v1.0.0-rc.3)

- **FlySky FS-i6S (Cortex-M0)**: **~104.1 KB Flash** out of 128 KB (~23.9 KB / ~18.7% headroom) and **~6.3 KB static RAM** out of 16 KB.
- **FlySky FS-i6X (Cortex-M0)**: **~99.8 KB Flash** out of 128 KB (~28.2 KB / ~22.1% headroom) and **~6.2 KB static RAM** out of 16 KB.
- **RadioMaster Pocket (Cortex-M4F)**: **~89.8 KB Flash** out of 512 KB (~422 KB headroom) and **~5.7 KB static RAM** out of 192 KB.
- **Jumper T12 (Cortex-M4F)**: **~90.1 KB Flash** out of 512 KB and **~5.7 KB static RAM**.
- **FrSky Taranis X9D / X9E (Cortex-M4F)**: **~90.5 KB Flash** out of 512 KB and **~5.7 KB static RAM** (1696 B 212x64 framebuffer).
- **RadioLink AT9S Pro (Cortex-M4F)**: **~91.8 KB Flash** out of 512 KB and **~5.7 KB static RAM** + 38.4 KB CCM RAM (320x240 4bpp color canvas).
- **Automated Host Tests**: **182 passing unit tests** across all board feature matrices.
- **Authoritative Budget & Breakdown**: For exact section sizes, partition layouts, and stack guarantees, see [ARCHITECTURE.md (Memory Footprint)](ARCHITECTURE.md#4-memory-footprint).

---

## Phase Breakdown

### Phase 1: Board Bring-Up, Power Latch, & Touchscreen (COMPLETED)
- [x] Dual MCU support for both `STM32F072VB` and `APM32F072VB` (UID & DFU mapping).
- [x] Electronic power latch (`PB15`), soft power hold-to-shutdown (`PB14` >= 1.5s), and dual blue LEDs (`PD10`/`PD11`).
- [x] FocalTech FT6236 capacitive touchscreen driver over hardware `I2C1` (`PB8`/`PB9` @ 400 kHz, `PA15` RST, `PC12` INT).
- [x] Parallel 8-bit ST7567 driver for `GPIOE` (ODR write) + control lines with in-tree 1bpp bitmap fonts.
- [x] Zero-disassembly DFU bootloader entry via front buttons (`PA9` + `PA10`), `SYSTEM INFO` menu, and USB CLI (`dfu`).

### Phase 2: Analog & Digital Inputs (COMPLETED)
- [x] Continuous 11/15-channel DMA1 ADC1 scanner (0.23 ms complete scan).
- [x] OpenTX Modified Moving Average (MMA) micro-jitter filter (0 latency on stick movement).
- [x] Exponential moving average (EMA) filter on battery voltage ADC (`PC0`, 10 kΩ / 5.1 kΩ divider).
- [x] Decode switches (`SA..SD`), rotary pots (`VRA`, `VRB`), extended analog (`PC1..PC5`), and J4 tactile buttons (`PA13`/`PA14`).
- [x] Configurable Stick Modes 1, 2, 3, and 4 with decoupled logical controls (`aileron`, `elevator`, `throttle`, `rudder`).

### Phase 3: A7105 SPI Driver & Protocol Timing (COMPLETED)
- [x] Amiccom A7105 hardware SPI1 driver with antenna diversity TR switching (`PE10`/`PE11`/`PE12`).
- [x] Deterministic 16-channel FHSS hopping table generated from 96-bit silicon UID.
- [x] External HSE crystal (48.000 MHz) and calibrated `TIM16` timer (`PSC = 47`, `ARR = 3849`) for exact 3850.0 µs (259.74 Hz) frame sync.

### Phase 4: AFHDS 2A Over-the-Air Link & Telemetry (COMPLETED)
- [x] 18-channel interleaved 38-byte stick frame generation (988..2012 µs).
- [x] 4-phase bidirectional bind sequence with persistent RX ID Flash storage.
- [x] Cancel / Abort binding mode via `[ESC]` and support for one-way receivers (FS-A8S, Fli14).
- [x] Downlink telemetry reception window: live RSSI and RX battery voltage.

### Phase 5: Trims, Audio, & Calibration (COMPLETED)
- [x] Hardware PWM piezo buzzer driver on `PA8` (`TIM1_CH1`, 48 MHz / PSC 47) with `Rich` and `Simple` audio profiles.
- [x] Dual digital trim modes: Stick Modifier Mode (`PA9`/`PA10` + gimbals) and Virtual Touch Perimeter Targets.
- [x] Complete 3-Axis Instant Trim support (Roll, Pitch, Yaw) with neutral-stick reset.
- [x] Throttle trim safety modes (`OFF (Lock)`, `IDLE`, `LINEAR`).
- [x] Interactive adaptive 2-step gimbal & multi-pot endpoint calibration wizard with Flash persistence.

### Phase 6: Settings Menu, Diagnostics, & Backlight Dimming (COMPLETED)
- [x] Hierarchical Settings Menu (`src/ui/menu/`) navigated via touch gestures, tap hitboxes, and front buttons.
- [x] Hardware PWM backlight dimming driver on `PC9` (`TIM3_CH4`, 1 kHz PWM) while keeping stock `PF3` supported.
- [x] 18-channel live pulse width monitor with dynamic channel labels and microsecond readouts.
- [x] Real-time 12-bit Analog Diagnostics (`Diag Anas`) across 4 graphic pages (`ANALOG (1-6)`, `ANALOG (7-11)`, `SWITCHES (E/F/G/H)`, `EXT ANALOG (PC1-5)`).
- [x] System Information screen displaying MCU profile, 96-bit silicon UID, git commit hash, and one-tap DFU reboot.

### Phase 7: 20-Model Memory & Smoothed Throttle Curves (COMPLETED)
- [x] 20 independent model memory slots (`M01`..`M20`), each allocated an exact 128-byte profile.
- [x] Model Match: independent `rx_id` per model profile with dynamic RF switching on model change.
- [x] Per-model digital trims (Roll, Pitch, Throttle, Yaw) and 18-channel reversing bitmask.
- [x] Switchable 5-point and 9-point throttle curves with optional **Catmull-Rom cubic Hermite spline** smoothing.
- [x] Real-time curve graph visualization in the on-screen throttle curve editor.

### Phase 8: Multi-Page Flight Dashboard & Navigation Polish (COMPLETED)
- [x] Pixel-perfect 5-page uniform flight dashboard (Gimbals, CH 1..10 Monitor, CH 11..18 Monitor, Model Dashboard, Custom Telemetry).
- [x] Key auto-repeat for UP and DOWN navigation (300 ms hold threshold, 70 ms repeat interval).
- [x] Non-overlapping adaptive switch and potentiometer bar on Page 1/5 supporting up to 12 switches and 10 pots.

### Phase 9: Safety First Subsystem (COMPLETED)
- [x] Transmitter low battery voltage alarm with configurable threshold in `Radio Setup` (4.0V–5.0V).
- [x] Model-specific pre-flight startup checks across Stick Modes 1–4 (Airplane/Glider/Quad idle, Heli Throttle Hold, General centered throttle).
- [x] Radio inactivity idle alarm after 10 minutes without stick, touch, or key interaction.
- [x] Downlink telemetry RSSI range warnings: audible alerts below 40% (Warning) and 20% (Critical).

### Phase 10: Flight Control & 18-Channel Mixing (COMPLETED)
- [x] Dual Rates & Exponential (D/R & EXPO) on Roll, Pitch, and Yaw with integer cubic curves and switchable high/low rates.
- [x] Auxiliary Channel Source Mapping (`CH5`–`CH18`) across all sticks, pots (`VRA..VRF`), and switches (`SA..SH`, `SWI..SWL`).
- [x] Pre-configured aircraft templates: Elevon / Delta Wing, V-Tail, and Flaperon (with differential throw support).
- [x] EdgeTX / OpenTX-style 18-channel freeform matrix mixer with 8 user-configurable mix lines (Weight, Offset, Switch, ADD/MULT/REPLACE modes).
- [x] Real-time flight control loop decoupled from display flush via hardware Cortex-M SysTick timer (`src/time.rs`).

### Phase 12: Code Review Hardening & Radio Link Safety (COMPLETED)
- [x] Bounded SPI1, I2C1, and A7105 hardware wait loops with loop counters to prevent CPU lockup.
- [x] Gated RF frame broadcast until mixer publishes first valid packet; staleness watchdog fallback (~115 ms) to `PACKET_FAILSAFE`.
- [x] Autonomous failsafe frame broadcast every 1,569 packets (~6.0s).
- [x] In-flight Flash write protection while armed (`!pipeline.prev_armed`).

### Phase 13: USB Subsystem & Protocol Engine (COMPLETED)
- [x] Hardware USB Full-Speed (12 Mbps) peripheral on `PA11` / `PA12` with internal 1.5 kΩ pull-up resistor.
- [x] Native USB Joystick class (HID) at 100 Hz with OpenI6X/EdgeTX mapping (8 axes, 16 buttons).
- [x] Virtual COM Port (CDC-ACM) streaming universal JSON Lines (`ndjson`) telemetry and interactive CLI (`help`, `status`, `channels`, `telem`, `stream`, `dfu`, `reboot`).
- [x] Decoupled stack-safe on-the-fly USB mode switching in `Radio Setup` (`OFF`, `JOYSTICK`, `SERIAL`, `COMPOSITE`).
- [x] Silent RF & PPM standby in Joystick mode (`U:SIM` status indicator).

### Phase 14: CRSF / ExpressLRS & FS-i6S Extension Bay (COMPLETED)
- [x] Full Crossfire (CRSF) & ExpressLRS driver on `USART1` (`PB6` TX, `PB7` RX on `J15`) with 5 selectable baud rates (`115.2k`, `416.6k`, `420k`, `921.6k`, `1.875M`) and full/half-duplex modes (`HDSEL` with self-echo suppression).
- [x] Configurable external module power switch polarity on `PF6` (header `H1`: `HIGH (N)` / `LOW (P)`).
- [x] Native on-radio ExpressLRS/CRSF configurator engine (16-device discovery, Parameter 0 root query, 6-level folder stack, integer & select editing).

### Phase 15: Hardware Watchdog, Append-Only Storage, & Pure PAC Driver (COMPLETED)
- [x] Pure PAC register-level driver layer (`stm32f0xx_hal::pac`).
- [x] Independent Hardware Watchdog (`pac::IWDG`) with 2.0s timeout, LSI clock stabilization, bounded register sync, and `DBGMCU_APB1_FZ` debugger halt freezing.
- [x] In-Flight Watchdog Reset Recovery (`RCC_CSR`) restoring active RF control in < 2 ms.
- [x] 4-page (8 KB, Pages 60–63 at `0x0801_E000`..`0x0802_0000`) log-structured append-only storage engine with `sequential-storage`.

### Phase 16–19: Protocol Compliance, In-Tree Fonts, Flight Timers & Ergonomics (COMPLETED)
- [x] TBS Crossfire Rev 08 11-bit channel scaling formula adherence (`988..2012 µs` <-> `172..1811`).
- [x] Dual-target host unit test harness (`cargo test-host`) with 140 automated unit tests (`docs/TESTING.md`).
- [x] In-tree bare-metal 1bpp glyph lookup tables (`FONT_4X6_DATA` and `FONT_6X10_DATA`), removing runtime `embedded-graphics` font overhead.
- [x] EdgeTX-parity Flight Countdown / Stopwatch timer with multi-trigger modes, arm-switch integration, and Hold-to-Reset HUD progress bar.
- [x] Unipolar throttle mixer source (`Thr+`) and Model Duplicate / Copy utility.

### Phase 20: Trainer Port Subsystem & PPM In/Out (COMPLETED)
- [x] Direct PAC driver for `TIM15` (`src/trainer/`).
- [x] PPM Output on `PF10` (`TIM15_CH2`): Deterministic 22.5 ms 8-channel CPPM stream on `J15`.
- [x] PPM Input on `PF9` (`TIM15_CH1`): 0.5 µs input capture decoder for wired buddy-box training and FPV head-trackers.
- [x] Safety Handover Mixer (`src/trainer/handover.rs`): Student control when `SD` is Down, instant instructor stick-override takeover (> 5% deflection), and 50 ms signal-loss failsafe fallback.

### Phase 22: Universal Input Modes & Extended Hardware Suite (COMPLETED)
- [x] Universal configurable ADC input modes (`DEFAULT`, `2-POS`, `3-POS`, `6-POS`, `POT`, `POT-D`, `INST-TRIM`) for all 11 auxiliary channels (`SA..SD`, `VRA..VRB`, `VRC..VRG`).
- [x] Extended ADC inputs on `PC1`–`PC5` (`ADC_IN11`–`ADC_IN15`) with autonomous 16-channel DMA1 scanning (`0xFFFF`).
- [x] J4 SWD header switch expansion (`PA13` SG / `PA14` SH) and configurable rear tactile button functions (`SWE`/`SWF`, `TIMER RESET`, `INSTANT TRIM`, `TRIMS MOD`).

### Phase 23: Configurable Telemetry Dashboard & RF Power Management (COMPLETED)
- [x] Configurable 4-tile telemetry dashboard on Flight Page 5/5 (`CUSTOM TELEM`) with 2×2 quadrant layout.
- [x] Dedicated `TELEM DASH SETUP` screen under `Radio Setup` supporting 13 selectable sensors (including live `TIMER`).
- [x] Configurable AFHDS 2A transmitter RF output power (`100mW (HI)`, `10mW (MID)`, `1mW (LOW)`, `100uW (TST)`).
- [x] Automatic deep sleep power-down (`0x0F`) for the internal A7105 transceiver when operating in CRSF mode.

### Phase 24: Core Unification & Hardware Abstraction Layer Isolation (COMPLETED)
- [x] Unified core crate name `ferrox` across both FlySky FS-i6X and FS-i6S codebases.
- [x] Complete hardware-agnostic core extraction: 100% byte-for-byte identical core modules outside `src/hw/` (`diff -rq -x hw src/` reports 0 differences).
- [x] Hardware Abstraction Layer (`src/hw/`) isolating board identities, battery voltage divider formulas, setup menu extensions, key scanning, and board peripherals.
- [x] Unified `TIM15` trainer subsystem supporting 8-channel CPPM Output on `PF10` and Student Input Capture on `PF9` with `SD` Down handover.
- [x] Universal `AdcInputMode` supporting 7 modes (`0..=6`: Default, 2-Pos, 3-Pos, 6-Pos, Pot, PotDetent, InstantTrim).
- [x] Standardized 5 ascending CRSF baud rates (`115.2k`..`1.875M`) and per-model half-duplex `HDSEL` routing on `PD5`.
- [x] Single-threaded test harness (`-- --test-threads=1`) with automated host unit tests.

### Phase 25: 32 Matrix Mixers, 7/9-Point Curves, & Active-Model Cache (COMPLETED / BRANCH: `feat/32-mixers-17pt-curves`)
- [x] Expanded freeform matrix mixer from 8 to 32 lines (`M01`..`M32`) per model with 8-byte packed `MixLine`.
- [x] Dynamic weight modulation: select channels/knobs (`weight_src`) to modulate mix line gain (Bipolar `CH1`..`CH18`, Unipolar `CH1+`..`CH18+`, or `Fixed`).
- [x] 5, 7, and 9-point throttle and collective pitch curves with switchable piecewise linear and Catmull-Rom cubic Hermite spline interpolation.
- [x] Active-model SRAM cache architecture: `RadioStorage` holds only `RadioConfig` and active `ModelConfig` in RAM.
- [x] Transparent in-place EEPROM migration: detects 128-byte v1 model slots, verifies CRC, migrates to v2 schema, and re-saves with fresh CRC envelope.
- [x] UI updates: `WING/MIXER` 34-item viewport (`M01`..`M32`), `EDIT MIX 01`..`32` with `Wt Src:` field, and `THR CURVE` 5-PT/7-PT/9-PT mode cycling with `P01:`..`P09:` formatting.

### Phase 26: Variometer Telemetry & Acoustic Audio Feedback (COMPLETED / BRANCH: `feat/vario-telemetry-audio`)
- [x] Telemetry frame decoding: CRSF `0x07` (`CRSF_FRAMETYPE_VARIO`) & `0x09` (`CRSF_FRAMETYPE_BARO_ALTITUDE`), and AFHDS 2A/i-BUS sensor `0x09` (vspeed) and `0x41`/`0x83`/`0xF9` (altitude).
- [x] Core variometer engine (`src/vario.rs`) featuring 4-tap EMA noise filtering, fallback altitude differentiator (Δh / Δt), configurable climb/sink deadbands, pitch scaling (1000..2800 Hz), cadence scaling (550..110 ms), sink modes (`BEEP`, `CONT`, `SILENT`), and automatic motor lockout (> 10% throttle).
- [x] Non-blocking background audio synthesis on hardware PWM piezo (`TIM1_CH1`, `PA8`) with zero-latency preemption by higher-priority alarms (battery, RSSI, timer, clicks) and phase-preserved cadence tracking.
- [x] Model storage integration: persistent `vario_switch`, `vario_climb_deadband`, `vario_sink_deadband`, `vario_sink_mode`, and `vario_motor_mute` in `ModelConfig` with automatic flash sanitization.
- [x] Dedicated UI editors: `VARIO SETUP` under `MODEL SETUP` with switch auto-detection and live acoustic tone preview.
- [x] Multi-tile telemetry dashboard integration: `VSPD` (`+X.Xm/s`) and `ALT` (`XXXXm`) tiles added to custom telemetry dashboard slot assignments.
### Phase 27: Aftermarket Hall Effect Gimbal Calibration (COMPLETED)
- [x] Support for aftermarket Hall effect gimbals alongside stock potentiometers.
- [x] Non-volatile `Gimbal Type` toggle (`POT` / `HALL`) in `Radio Setup` utilizing reserved radio configuration byte (`RadioConfig` remains 148 bytes on FS-i6S).
- [x] Inverted Hall ADC normalization, calibrated center detent deadband, and full stick throw travel without endpoint clipping.
- [x] Dedicated documentation guide: [`docs/HALL_GIMBALS.md`](HALL_GIMBALS.md).

### Phase 28: I2C EEPROM Diagnostics, Status Reporting, & Bus Recovery (COMPLETED)
- [x] Hardware I2C2 EEPROM driver for FT24C128A on `PB10`/`PB11` at ~1000 kHz Fast Mode Plus (Fm+).
- [x] Runtime adaptive bus probing (`0x50`..`0x57`), automatic peripheral APB1 reset & STOP bit recovery on bus lockup.
- [x] Dedicated on-radio `STORAGE (EEPROM)` Diagnostics screen (Page 4 in `Analog Diag`) displaying bus, address, device status, and slot metrics.
- [x] USB Serial interactive CLI command `storage` / `eeprom` reporting live I2C bus counters and slot integrity.

### Phase 29: Global Variables (GV1..GV5) in Mixer & Storage (COMPLETED)
- [x] Dedicated `GV1`..`GV5` variables (range `-1000..+1000`) per model with persistent EEPROM storage.
- [x] Mix line integration: use global variables as direct mix sources or dynamic weight modulation sources.
- [x] Dedicated UI editor under `WING / MIXER -> Global Variables`.
- [x] Zero-loss in-place EEPROM schema migration.

### Phase 30: Spektrum Protocol & Multi-Module Integration (COMPLETED)
- [x] Spektrum Satellite Remote Receiver (DSM2 / DSMX) Wireless Trainer Input on `USART2` (`PA15` RX) decoding standard 10-bit and 11-bit frames.
- [x] Multi-Module serial output protocol for external Spektrum DSM2 / DSMX modules.
- [x] Spektrum Forward Programming engine lifecycle and flight log frame decoding.

### Phase 31: STM32F4 HAL & RadioMaster Pocket Hardware Bring-Up (COMPLETED)
- [x] Dedicated Cortex-M4F HAL crate (`crates/ferrox-stm32f4`) with QX7-family hardware descriptors.
- [x] Automated linker script generation in `boards/ferrox-pocket/build.rs` switching between direct DFU (`0x08000000`, 512 KB) and EdgeTX SD card bootloader (`0x08008000`, 480 KB) via `CARGO_FEATURE_EDGETX_BOOTLOADER`.
- [x] Bare-metal SPI2 microSD card block driver (`crates/ferrox-stm32f4/src/sdcard.rs`) on `PB12` CS, `PB13` SCK, `PB14` MISO, `PB15` MOSI, `PD9` Card Detect (`CMD0`, `CMD8`, `ACMD41`, `CMD58`, `CMD17`, `CMD24`).
- [x] Hybrid `PocketEepromI2cBus`: runtime probing of physical I2C1 EEPROM with automatic fallback to microSD card `/FERROX/` storage and in-RAM defaults.
- [x] Zero-heap 512-byte sector buffered flight telemetry CSV logger (`crates/ferrox-core/src/telemetry_log.rs`).
- [x] Build aliases: `cargo build-pocket` (DFU) and `cargo build-pocket-sd` (EdgeTX SD dual-boot).

### Phase 32: Jumper T12 Board Specification (COMPLETED)
- [x] Board profile `T12Spec` in `crates/ferrox-core/src/board/t12.rs` implementing `Qx7FamilyProfile`.
- [x] Dedicated switch matrix, detented rotary pots, and rotary scroll wheel navigation.
- [x] HAL mapping `T12Hal` in `ferrox-stm32f4::descriptor` and dispatch macro `define_qx7_context!(create_t12_context, T12Hal)`.
- [x] Build alias: `cargo build-t12`.

### Phase 33: Responsive 212x64 Wide Monochrome UI (FrSky X9D / X9E) (COMPLETED)
- [x] Parameterized `DISPLAY_WIDTH` and `DISPLAY_HEIGHT` in `BoardProfile` and `Qx7FamilyProfile`.
- [x] Dynamically scaled ST7567 driver (`1024` bytes for 128x64 vs `1696` bytes for 212x64) and page-stride DMA flushes.
- [x] Responsive UI engine: scaled headers, footers, status bars, splash screen, and multi-column gimbal/switch dashboards across 212-pixel wide displays.
- [x] Board specifications: `X9dSpec` (`crates/ferrox-core/src/board/x9d.rs`) and `X9eSpec` (`crates/ferrox-core/src/board/x9e.rs`).
- [x] Build aliases: `cargo build-x9d` and `cargo build-x9e`.

### Phase 34: Zero-Heap 4-Bit Indexed Palette Color Engine (RadioLink AT9S Pro) (COMPLETED)
- [x] 16-color semantic UI palette (`ColorIndex`) and RGB565 look-up tables (`DARK_PALETTE`, `DAYLIGHT_PALETTE`) in `crates/ferrox-core/src/display/color.rs`.
- [x] Zero-heap `ColorCanvas<W, H, BUF_SIZE>` (consuming only `38,400` bytes for 320x240, fitting within STM32F4 CCM RAM).
- [x] Backward-compatible `BinaryColorAdapter` mapping existing monochrome UI widgets to color indices.
- [x] Extended `DisplayBus` with `flush_indexed_rgb565` hook.
- [x] RadioLink `At9sProSpec` (`crates/ferrox-core/src/board/at9spro.rs`) with 320x240 display, 4 auxiliary analog axes (pots and side sliders), and rotary encoder.
- [x] HAL mapping `At9sProHal` and build alias `cargo build-at9spro` (~91.8 KB release flash footprint, ~5.7 KB RAM).

### Phase 35: FAT32 WAV/PCM Voice Audio, EdgeTX Bootloader Dual-Boot & Feature Gating (COMPLETED)
- [x] Zero-heap read-only FAT16/FAT32 reader (`crates/ferrox-core/src/fat.rs`) with MBR/superfloppy mount, 1-sector FAT cache, 8.3 & VFAT LFN traversal, and sector streaming.
- [x] WAV/PCM parser (`crates/ferrox-core/src/audio/wav.rs`), EdgeTX `/SOUNDS/en/` numeric speech decomposer, and non-blocking `CalloutScheduler` (`crates/ferrox-core/src/audio/mod.rs`).
- [x] STM32F407 `DAC1` (`PA4`) + `TIM6_TRGO` + `DMA1_Stream5` circular ring-buffer audio driver (`crates/ferrox-stm32f4/src/audio.rs`) coordinating `PA4`/`PE12` with `DacToneBuzzer`.
- [x] EdgeTX SD bootloader compatibility (`chip::pre_init()` VTOR relocation + NVIC/SysTick cleanup, PLL reset before reconfig, SPI3 reset & ST7567 init timing fix, `.fwversiondata` header, and `tools/build-sd-bin.*`).
- [x] Compile-time `voice-audio` (STM32F4 only by default) and `soaring` (default-on, opt-out via `--no-default-features`) Cargo feature flags to preserve STM32F072 flash headroom.

