# SOFTWARE ARCHITECTURE & TIMING MODEL

`ferrox` uses a bare-metal, `no_std` reactive architecture powered by the unified **`ferrox-core`** flight engine and board targets (`boards/ferrox-i6s` and `boards/ferrox-i6x`), designed for deterministic RF packet timing and sub-millisecond control latency on STM32F072 / APM32F072 Cortex-M0 microcontrollers.

---

## 1. Clock Tree & Unified `ferrox` Workspace Architecture

### 1.1 Clock Tree Configuration

| Component | Setting | Notes |
| :--- | :--- | :--- |
| **Primary Oscillator** | **HSE Crystal @ 8.000 MHz** | Clean external quartz crystal on pins `PD0`/`PD1` |
| **PLL Multiplier** | **PLLMUL = 6** | 8.000 MHz * 6 = **48.000 MHz** core clock |
| **Flash Latency** | **1 Wait State (`LATENCY = 1`)** | Mandatory for Cortex-M0 operation above 24 MHz |
| **System Busses** | **AHB = 48 MHz, APB = 48 MHz** | Prescalers set to 1 for maximum peripheral throughput |
| **USB Physical Clock** | **USBSW = 1 (PLLCLK)** | Routes 48.000 MHz PLL directly to USB peripheral (`RCC_CFGR3` bit 7) |

Implemented in [`crates/ferrox-core/src/chip/mod.rs`](../crates/ferrox-core/src/chip/mod.rs) using pure PAC direct register access (`pac::RCC`).

### 1.2 Unified `ferrox-core` & Multi-Architecture Workspace Architecture

The repository is organized as a unified Cargo workspace supporting multiple processor architectures with zero runtime branch penalty between targets:

| Layer | Module Path | Responsibility |
| :--- | :--- | :--- |
| **Board Spec Trait** | [`crates/ferrox-core/src/board/mod.rs`](../crates/ferrox-core/src/board/mod.rs) | `BoardProfile` const trait specifying `NAME`, `DISPLAY_WIDTH`, `DISPLAY_HEIGHT`, `HAS_TOUCH`, `HAS_POWER_LATCH`, `NUM_ANALOG_CHANNELS`, `HAS_EXT_ADC`, `HAS_ROTARY_ENCODER`, and EEPROM CRC parameters |
| **Unified Core (`ferrox-core`)** | `crates/ferrox-core/src/{lib.rs, input.rs, mixer.rs, storage.rs, safety.rs, calib.rs, curve.rs, trim.rs, buzzer.rs, watchdog.rs, time.rs, telemetry_log.rs}` | Real-time flight pipeline, 18-channel 32-line matrix mixer, Catmull-Rom curves, sequential Flash/EEPROM storage, pre-flight safety, calibration wizard, audio engine, and SD telemetry logging |
| **Protocol & Display** | `crates/ferrox-core/src/{rf/, crsf/, trainer/, usb/, display/, ui/}` | AFHDS 2A / A7105 driver, CRSF / ExpressLRS configurator, `TIM15` CPPM Out/In & buddy-box handover, direct PAC USB 2.0 driver, responsive ST7567 display (128x64 & 212x64), and 4-bit indexed color canvas |
| **STM32F0 HAL Driver** | `crates/ferrox-stm32f0/` | Shared peripheral drivers for STM32F072 / APM32F072 (Cortex-M0 @ 48 MHz) implementing `ToneOutput`, `DisplayBus`, and `StorageBus` |
| **STM32F4 HAL Driver** | `crates/ferrox-stm32f4/` | Shared peripheral drivers for STM32F407 / STM32F405 (Cortex-M4F @ 168 MHz) featuring SPI2 microSD block driver, DMA display flushes, and QX7-family descriptors |
| **Cortex-M0 Board Crates** | `boards/ferrox-i6s/`, `boards/ferrox-i6x/` | Target crates for FlySky FS-i6S (touch, power latch) and FS-i6X (matrix keypad, optical jog wheel) |
| **Cortex-M4F Board Crate** | `boards/ferrox-f4/` | Target crate for all STM32F407/F405 targets (Pocket, Boxer, Zorro, TX12, QX7, T-Pro V2, T12, X9D/X9E, AT9S Pro, GX12) |

---

## 2. Real-Time Concurrency & Safety Model

```mermaid
flowchart TD
    subgraph Hardware_Timers ["Deterministic Interrupts & Peripherals"]
        USART1["USART1 ISR (0x40, PB6/PB7)<br>128B Lock-Free CRSF RX Ring Buffer"]
        TIM16["TIM16 ISR @ 259.74 Hz (3.850 ms, 0x80)<br>Pulls fresh CHANNEL_BUFFER slot<br>Transmits 18-CH AFHDS 2A Packet via SPI"]
        EXTI["EXTI2_3 ISR (A7105 GIO2, 0x80)<br>Handles TX Finished / RX Telemetry Available"]
        TIM15["TIM15 ISR (0xC0, PF10 Out / PF9 In)<br>22.5ms 8-CH CPPM Out & 0.5µs Trainer Capture"]
        DMA["DMA1 Channel 1 (Autonomous)<br>Scans 11/15 ADC channels in 0.23-0.31 ms"]
        USB_IRQ["USB Full-Speed Interrupt<br>Low priority (0xC0), handles host bus events"]
        PWM_Audio["TIM1 Channel 1 (Hardware PWM @ PA8)<br>Drives Piezo Buzzer Audio Frequencies"]
        PWM_BL["TIM3 Channel 4 (Hardware PWM @ PC9)<br>1 kHz Backlight Dimming Mod"]
        IWDG["Hardware Watchdog (pac::IWDG @ 40 kHz LSI)<br>2.0s Hard Timeout, APB1 Freeze in Debug"]
    end

    subgraph Main_Thread ["Main Execution Loop (~500 Hz)"]
        WDT_Feed["0. Pet Watchdog (watchdog::feed)<br>Refreshes 2.0s hardware timer"]
        Key_Poll["1. hw::poll_keys<br>FT6236 I2C1 touch on PC12 + front/J4 keys + modifier trims"]
        Flight_Tick["2. FlightPipeline::tick (multi-kHz)<br>ADC scan, matrix mixer, trainer handover, spline curves, RF publish"]
        Idle_Tick["3. BackgroundIdleManager::tick (30 Hz)<br>PowerManager (PB14), UI screens, ST7567 LCD flush, buzzer"]
    end

    DMA -.-> Flight_Tick
    WDT_Feed --> Key_Poll --> Flight_Tick --> Idle_Tick
    Flight_Tick -. Triple Buffer .-> TIM16
    Flight_Tick -. 8-CH Pulses .-> TIM15
    TIM15 -. Student Channels .-> Flight_Tick
    WDT_Feed -. Reload .-> IWDG
```

### Interrupt Priorities & Safety Systems
- **Hardware Watchdog (`pac::IWDG`)**: Independent 2.0-second hardware watchdog running off the autonomous 40 kHz internal low-speed oscillator (LSI) with prescaler `/64` (625 Hz tick rate, 1.6 ms/tick) and reload count `1250`. Prior to watchdog key registration, LSI clock stabilization is confirmed via `pac::RCC.csr.lsirdy`. To support non-intrusive SWD debugging via ST-Link or probe-rs, `DBGMCU_APB1_FZ.DBG_IWDG_STOP` is configured (with DBGMCU clock gated via `RCC_APB2ENR`) so the watchdog timer freezes when the core is halted. The watchdog is refreshed (`watchdog::feed()`) at the start of the main execution loop, during flash compaction routines, and across soft reset vectors.
- **Highest Peripheral Priority (`USART1` CRSF / ELRS RX on i6S, `USART2` on i6X)**: IRQ 27 assigned priority `0x40` in [`crates/ferrox-core/src/hw/i6s/uart.rs`](../crates/ferrox-core/src/hw/i6s/uart.rs). At 420,000–1,875,000 baud, bytes arrive every 5.3–23.8 µs. With no hardware FIFO on the STM32F072 USART, this priority ensures the ISR preempts main thread execution (e.g. LCD flushing) to buffer incoming bytes into a 128-byte atomic ring buffer (`RX_RING`) with zero overrun errors (`USART_ISR_ORE`).
- **High Priority (RF Transmission)**: `TIM16` fires strictly every **3.850 ms** (259.74 Hz). It pulls the latest pre-computed channel microsecond pulses from the lock-free SPSC triple buffer (`CHANNEL_BUFFER`) with zero critical sections, zero interrupt latency, and zero data races, initiating A7105 SPI transmission. Priority = `0x80`.
- **Medium Priority (Radio Event)**: `EXTI2_3` fires on A7105 GIO2 line transitions (packet transmission complete or downlink telemetry packet received). Priority = `0x80`.
- **Autonomous DMA**: `DMA1_CH1` transfers all 11 (or 15 when `Ext ADC (PC)` is enabled) ADC channels directly into circular SRAM buffers with zero CPU intervention.
- **Hardware Timers**: `TIM1` generates non-blocking audio frequencies on `PA8`; `TIM3` generates 1 kHz PWM brightness control on `PC9`; `TIM15` (`0xC0`) drives 22.5 ms CPPM output on `PF10` (`TIM15_CH2`) and 0.5 µs input capture on `PF9` (`TIM15_CH1`).
- **Low Priority (USB Physical Layer)**: The USB interrupt is assigned priority `0xC0`. Because RF interrupts have higher priority (`0x80`), USB transactions or host bus stalls can never preempt or delay an over-the-air packet.
- **Background / Main Loop**: Decoupled control loop architecture; the real-time flight control pipeline executes in under 30 µs at multi-kHz pass rates, updating the lock-free SPSC triple buffer for RF transmission, while ST7567 LCD frame rendering is throttled to a smooth 30 Hz (~33 ms).

---

### 2.1 Hardware Watchdog & Safety-Critical Recovery Architecture

In flight-critical avionics and remote control systems, a hardware watchdog must do more than simply reset a hung processor: it must guarantee safe, predictable, and instantaneous return-to-service without entrapping the pilot in power-on interlocks.

```mermaid
flowchart TD
    subgraph Boot_Decision ["Reset Source Evaluation (RCC_CSR)"]
        Boot["MCU Boot / Reset Vector (0x0800_0000)"]
        PowerInit["hw::pre_init() -> power::init()<br>Latch PB15 HIGH immediately"]
        ReadCSR["chip::check_and_clear_reset_flags()<br>Read RCC_CSR: IWDGRSTF / WWDGRSTF"]
        ClearCSR["Clear Reset Flags (RCC_CSR.RMVF = 1)"]
        IsWDT{"Watchdog Reset?"}
        
        Boot --> PowerInit --> ReadCSR --> ClearCSR --> IsWDT
    end

    subgraph Cold_Boot ["Normal Cold Power-On Boot"]
        DFU["hw::keys::check_dfu_entry (PA9+PA10 or 0xDEADBEEF)<br>~6.25 ms contact settling delay"]
        Clocks["Init 48 MHz HSE+PLL & SysTick"]
        TouchInit["hw::init_touch() -> FT6236 I2C1"]
        Storage["Load Storage & Tone Preferences"]
        Splash["Ferrox-RC Splash Screen (1200 ms)<br>Concurrent 4-Note Welcome Fanfare (buzzer::chime_welcome)"]
        Periphs["Init ADC+DMA, RF, USB, CRSF, Trainer, Trims"]
        SafetyCheck["Pre-Flight Safety Check Loop<br>Mode 1..4 Throttle & Switches Safe<br>Modal Trap with 1000 µs Failsafe"]
        
        IsWDT -- "No (POR/PDR/Pin Reset)" --> DFU --> Clocks --> TouchInit --> Storage --> Splash --> Periphs --> SafetyCheck
    end

    subgraph Warm_Recovery ["In-Flight Watchdog Recovery (< 2 ms)"]
        FastInit["Bypass DFU Check (hw::keys::init_keys Directly)<br>Inhibit Bind-on-Boot & Calib Wizard"]
        ClocksWarm["Init 48 MHz HSE+PLL & SysTick"]
        TouchWarm["hw::init_touch() -> FT6236 Touchscreen"]
        PeriphsWarm["Init LCD, ADC+DMA, RF, Trainer, Trims"]
        AlertChirp["Urgent Warning Chirp (buzzer::play_tone_pattern)<br>Non-blocking 2.6 kHz acoustic alert"]
        BypassCheck["BYPASS Pre-Flight Safety Interlocks<br>Direct into Flight Pipeline"]
        
        IsWDT -- "Yes (IWDG/WWDG Timeout)" --> FastInit --> ClocksWarm --> TouchWarm --> PeriphsWarm --> AlertChirp --> BypassCheck
    end

    subgraph Flight_Loop ["Active Real-Time Flight Pipeline"]
        ActiveRF["Multi-kHz Flight Control Loop<br>Real-time ADC -> Mixer -> RF Transmission"]
    end

    SafetyCheck -- "Sticks Idle / ESC Pressed" --> Flight_Loop
    BypassCheck --> Flight_Loop
```

#### 1. Clock Domain Independence & Fail-Safe Hardware Configuration
The Independent Watchdog (`IWDG`) runs off the dedicated 40 kHz Low-Speed Internal (`LSI`) RC oscillator. This oscillator is physically decoupled from the primary high-speed crystal oscillator (`HSE`), internal high-speed oscillator (`HSI`), and Phase-Locked Loop (`PLL`).
- **Oscillator Failure Immunity**: If the 8 MHz external crystal fractures due to physical shock or vibration, or if the PLL loses lock, the watchdog continues counting down unhindered and forces a hardware MCU reset.
- **Deterministic Timeout**: With prescaler `/64` (PR = 4) and reload count `1250`, each counter tick is:
  - tick duration = 64 / 40,000 Hz = 1.6 ms
  - timeout = 1250 * 1.6 ms = 2.000 seconds
- **Zero-Cost Standalone Feed**: The `watchdog::feed()` function is marked `#[inline(always)]` and directly writes the reload key `0xAAAA` into `IWDG_KR`. It compiles down to a single Cortex-M0 assembly instruction:
  ```armasm
  str r0, [r1]
  ```
  This is completely lock-free, reentrant, and incurs zero runtime overhead (~21 ns execution at 48 MHz).

#### 2. Hardware Anti-Lock & Bounded Synchronization
On Cortex-M0 microcontrollers, register updates across asynchronous clock domains (48 MHz APB to 40 kHz LSI) require synchronization cycles reflected in `IWDG_SR` (`PVU` and `RVU` status bits).
- Rather than using unbounded `while` loops that could hang firmware if the LSI oscillator degrades, `WatchdogManager::init` implements an explicit bounded iteration cap (`sync_timeout = 10_000u32`).
- Clock gating for debug peripherals is initialized in the correct hardware sequence: `RCC_APB2ENR.DBGMCUEN` is enabled before reading or writing `DBGMCU`, preventing immediate bus error `HardFault` exceptions.
- `DBGMCU_APB1_FZ.DBG_IWDG_STOP` is asserted, halting the IWDG counter automatically during SWD debugger halts (`probe-rs`, GDB, ST-Link) so breakpoints do not cause spurious resets.

#### 3. Flash Wear-Leveling Compaction Guarding
During multi-page log-structured compaction cycles within `sequential-storage` (e.g. updating model trims, mixer settings, or calibration data across Flash pages 60–63), synchronous flash controller erase cycles can block execution for several milliseconds. The storage subsystem explicitly feeds the watchdog between page erase cycles, guaranteeing that background flash maintenance never starves the watchdog.

#### 4. The In-Flight Reset Trap & Instant Recovery Pipeline
A catastrophic hazard in conventional open-source and commercial RC firmware is the **watchdog in-flight trap**:
1. An electrostatic discharge (ESD) event, RF power amplifier surge, or voltage sag triggers an unexpected MCU watchdog reset mid-flight.
2. The MCU reboots in under 2 ms.
3. The firmware enters standard cold-boot safety checks: it tests whether the throttle stick is at idle (<= 2% / 20‰ on unipolar models, centered within ±2% on General surface models, or Throttle Hold engaged on Heli) across the configured stick mode (Mode 1..4), and all switches are in the UP position.
4. Because the aircraft is actively flying, the pilot's throttle is elevated, and flight switches (e.g., Arm, Flight Mode) are engaged.
5. The transmitter flags a safety violation, traps the pilot on a warning screen, forces all over-the-air channels to 1000 µs failsafe, and refuses to send flight control packets until the throttle is lowered.
6. The aircraft crashes before the pilot can diagnose or clear the screen.

#### Solution & DO-178C Deterministic Recovery:
In `ferrox`, reset recovery is treated as a safety-critical state machine:
- **RCC_CSR Reset Cause Latching**: At the absolute top of `main()`, `chip::check_and_clear_reset_flags()` reads `RCC_CSR`. It detects if the reset was initiated by `IWDGRSTF` (Independent Watchdog) or `WWDGRSTF` (Window Watchdog) and immediately clears all reset flags via `RMVF`.
- **DFU Settling Delay Elimination**: On cold boots, `hw::keys::check_dfu_entry` inserts a ~6.25 ms settling delay to debounce buttons. On watchdog recovery, this check is completely bypassed, calling `hw::keys::init_keys()` directly. This removes unnecessary latency and eliminates any possibility of accidentally entering the DFU bootloader mid-flight.
- **Inhibition of Modal Wizards**: Bind-on-boot and direct calibration wizard entry (`initial_keys & (1 << 10)`) are inhibited during watchdog recovery, ensuring active model binding and calibration states are untouched.
- **Non-Blocking Acoustic Pilot Alert**: The standard multi-note startup melodic chime is replaced with an urgent, non-blocking 3-beep alarm pattern (`buzzer.play_tone_pattern(2600, 60, 40, 3)`). This alerts the pilot through acoustic feedback that a reset occurred while consuming zero CPU wait cycles.
- **Pre-Flight Safety Check Bypass**: When `was_watchdog_reset` is true, the pre-flight safety check loop is bypassed entirely. The runtime immediately instantiates the `FlightPipeline` and enters the real-time flight loop.
- **Sub-2 Millisecond Recovery**: Total elapsed time from reset vector execution to the first active, calibrated over-the-air RF packet is measured at **< 2.0 ms**: well within the typical 50–100 ms receiver failsafe timeout window, preventing any loss of aircraft altitude or attitude control.

---

## 3. Channel Latency & Data Freshness

1. **Continuous ADC Scan**: All 11 standard channels (or 15 channels when `Ext ADC (PC)` is enabled) are digitized continuously by ADC1 via DMA1 Channel 1 in **0.23–0.31 ms**.
2. **Lock-Free SPSC Triple Buffer (`CHANNEL_BUFFER`)**: The flight loop publishes 18-channel frames via atomic index swapping with zero interrupt disabling, while `TripleBuffer::read_local()` allows background consumers (LCD monitors, USB HID/CDC) to read without racing the `TIM16` RF ISR.
3. **RF Gating & Staleness Watchdog**: Over-the-air transmission is gated until the flight loop publishes its first valid frame (`has_published`), and if the flight pipeline ever stalls for >30 ticks (~115 ms), the RF ISR automatically transmits pre-configured failsafe pulses (`PACKET_FAILSAFE`).
4. **Guaranteed Fresh Packets**: Because the decoupled flight control loop runs at multi-kHz rates while the RF transmitter transmits at ~260 Hz, **every over-the-air packet carries fresh, up-to-date stick data** with end-to-end latency under **2.0 ms**.

---

## 4. Memory Footprint

Measured on release builds across architectures (`opt-level = "z"`, LTO enabled):

### A. Cortex-M0 Targets (`thumbv6m-none-eabi`, 128 KB Flash, 16 KB SRAM)
- **FlySky FS-i6S**: **~104.1 KB Flash** (`.text` ~92.9 KB + `.rodata` ~12.6 KB + `.data` 764 B), leaving **~23.9 KB headroom (18.7%)**. Static RAM: **~6.3 KB** (`.bss` ~5.5 KB + `.data` ~770 B), leaving **> 9.7 KB stack safety margin**.
- **FlySky FS-i6X**: **~99.8 KB Flash** (`.text` ~89.1 KB + `.rodata` ~12.6 KB + `.data` 772 B), leaving **~28.2 KB headroom (22.0%)**. Static RAM: **~6.2 KB** (`.bss` ~5.4 KB + `.data` ~770 B), leaving **> 9.8 KB stack safety margin**.

### B. Cortex-M4F Targets (`thumbv7em-none-eabihf`, 512 KB Flash, 128 KB SRAM + 64 KB CCM)
- **RadioMaster Pocket**: **~89.8 KB Flash** (`.text` ~78.2 KB + `.rodata` ~11.5 KB), leaving **> 420 KB headroom**. Static RAM: **~5.7 KB** (`.bss` ~4.5 KB + `.data` ~1.1 KB).
- **Jumper T12**: **~90.1 KB Flash**, leaving **> 420 KB headroom**. Static RAM: **~5.7 KB**.
- **FrSky Taranis X9D / X9E**: **~90.5 KB Flash**, leaving **> 420 KB headroom**. Static RAM: **~5.7 KB** (includes 1696-byte wide 212x64 framebuffer).
- **RadioLink AT9S Pro**: **~91.8 KB Flash**, leaving **> 420 KB headroom**. Static RAM: **~5.7 KB** + 38.4 KB CCM RAM (320x240 4-bit indexed color canvas).

### Zero Heap & In-Place Loading Guarantees
- Strictly `#![no_std]` without any dynamic allocator (`alloc` crate is not linked).
- All display framebuffers, DMA circular queues, and triple buffers are statically allocated at link time.
- External I2C EEPROM wear-leveled A/B ping-pong storage for `RadioConfig` and 20 dedicated `ModelConfig` slots completely isolates model data from firmware flashing.
- Optional SPI2 microSD card block driver and telemetry logging buffers 512-byte raw sectors directly to card.

---

## 5. Throttle Curve Engine & Catmull-Rom Spline Math (`src/curve.rs`)

The curve engine transforms normalized stick inputs (0..1000) into tailored output curves:
- **Modes**: 5-point (0%, 25%, 50%, 75%, 100%) and 9-point (0%, 12.5%, 25%, ..., 100%).
- **Linear Interpolation**:
  ```text
  val = y0 + ((y1 - y0) * delta_x) / x_span
  ```
- **Catmull-Rom Cubic Spline Smoothing**:
  To achieve smooth, C1-continuous throttle response without flat inflection points or aggressive step changes, the engine evaluates standard Catmull-Rom cubic Hermite splines:
  ```text
  P(t) = 0.5 * (2*P1 + (-P0 + P2)*t + (2*P0 - 5*P1 + 4*P2 - P3)*t^2 + (-P0 + 3*P1 - 3*P2 + P3)*t^3)
  ```
- **Deterministic Fixed-Point Execution**:
  Implemented using integer-only fixed-point arithmetic (`t` scaled by 1024). Spline interpolation executes in **< 60 CPU clock cycles** (< 1.25 µs at 48 MHz), meaning spline smoothing imposes zero perceptible latency on the control loop.

---

## 6. Voice Audio & FAT32 Streaming (STM32F4, `voice-audio` feature)

- **Zero-Heap FAT16/FAT32 Reader (`crates/ferrox-core/src/fat.rs`)**: Mounts MBR or superfloppy microSD cards with a 1-sector FAT cache, traverses `/SOUNDS/<lang>/` paths (8.3 + VFAT LFN) using the Radio Setup voice language setting, and streams WAV files one 512-byte sector at a time.
- **Non-Blocking Background Streaming (`crates/ferrox-core/src/audio/`)**: `AudioPlayer` and `CalloutScheduler` decode 8-bit/16-bit mono/stereo PCM into 12-bit unsigned DAC samples (`0..=4095`) in the background idle loop (up to 4 sectors per tick), supporting configurable volume, repeat interval, and F5J/timer/telemetry cues without blocking the 4 ms RF flight loop.
- **STM32F407 DAC + Circular DMA (`crates/ferrox-stm32f4/src/audio.rs`)**: Uses `PA4` (`DAC_OUT1`), `TIM6_TRGO`, and `DMA1_Stream5` (Channel 7) over a 2048-sample (`4 KiB`) circular ring buffer, clearing consumed slots to DAC midpoint (`2048`) and coordinating `PA4`/`PE12` (`AUDIO_MUTE`) with `DacToneBuzzer`.

---

## 7. Multi-Protocol Module (MPM), Logical Switches & SD Telemetry Logging

- **100k 8E2 MPM Serial Link (`crates/ferrox-core/src/rf/mpm.rs`)**: Drives external/internal Multi-Protocol Modules over 100,000 baud 8E2 UART (`baud_idx = 5` in STM32F0/F4 UART drivers) with FrSky D8/D16/LBT, SFHSS, and Graupner HoTT protocol selection, RX number, frequency fine-tuning, range-check, and FrSky S.Port / HoTT telemetry decoding.
- **Programmable Logical Switches (`crates/ferrox-core/src/mixer.rs`, `logical-switches` feature)**: Evaluates 8 configurable logical switches (`L1..L8`) supporting comparator (`a>x`, `a<x`), edge/sticky, timer/pulse, and boolean (`AND`, `OR`, `XOR`) operations as mixer/timer/audio sources.
- **SD Telemetry CSV Logger (`crates/ferrox-core/src/telemetry_log.rs`, STM32F4)**: Buffers 512-byte CSV flight telemetry sectors to `/FERROX/` on microSD cards during background idle ticks.
