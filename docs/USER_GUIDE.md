# Ferrox User Guide & Operations Manual

Concise operational manual for **Ferrox** across all supported STM32F4 (RadioMaster, Jumper, FrSky, RadioLink) and STM32F0 (FlySky FS-i6X / FS-i6S) transmitters.

---

## 1. Controls & Navigation

Ferrox adapts automatically to each transmitter's physical input hardware:

| Radio Family | Navigation Paradigm | Open Menu | Confirm / Edit | Back / Cancel | Cycle Dashboard |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **STM32F4 Rotary** (Pocket, TX12, Boxer, Zorro, GX12, QX7, T12, T-Pro V2, X9D+, X9E, AT9S Pro) | Scroll wheel + push button (`MDL`/`SYS`/`RTN`) | Press `MDL` or `SYS`, or long-press wheel | Press wheel / `ENT` | `RTN` / `EXIT` | Scroll wheel on dashboard |
| **FlySky FS-i6X** | 4-way tactile keypad (`UP`/`DN`/`OK`/`CANCEL`) | Long-press `OK` | Short-press `OK` | `CANCEL` | `UP` / `DOWN` on dashboard |
| **FlySky FS-i6S** | Capacitive touchscreen + optical jog wheel | Swipe Up or tap center | Tap row / press jog | Swipe Right / `CANCEL` | Swipe Left / Right |

> **Power-On Startup**: Hold the power button until the Ferrox splash screen appears, then release. If throttle is not at minimum or a safety switch is active, the **Pre-Flight Safety Interlock** displays the offending control and holds RF output disabled until cleared (or bypassed with `[OK]`/`[CANCEL]` where permitted).

---

## 2. Multi-Page Flight Dashboard

The live flight screen cycles through 5 standard pages (plus a 6th page when **F5J Soaring** is enabled):

1. **Page 1 — Gimbals & Trims**: Live crosshair plots for both gimbals, digital trim indicators (Ail/Ele/Thr/Rud), active model name, battery voltage, RF link indicator, and flight timer.
2. **Page 2 — Primary Channels (`CH1..CH10`)**: Real-time bipolar bar graphs (`988..2012 µs`) showing post-mixer, post-reverse, post-safety outputs.
3. **Page 3 — Auxiliary Channels (`CH11..CH18`)**: Real-time bar graphs for high auxiliary channels.
4. **Page 4 — Model Summary**: Quick overview of active protocol, RF power/baud, arm state, active logical switches (`L1..L8`), and timer status.
5. **Page 5 — Telemetry & Custom 4-Tile Dashboard**:
   - **Default View**: `RxBat`, `LQ / RSSI`, `TxBat`, and `Current / Link`.
   - **Custom View (`CUSTOM TELEM`)**: 4 user-configurable telemetry tiles selectable in `Model Setup -> Telem Setup`. Stale telemetry (>2 s without fresh packets) is clearly flagged.
6. **Page 6 — F5J Graphic Altitude** *(when Soaring is enabled)*: 10-minute scrolling altitude profile, 30-second motor run countdown, 10-second zoom window, and live launch height / score display.

---

## 3. Digital Trims, Audio, Voice & Variometer

- **Digital Trims**: Pitch-scaled tones rise/fall with trim offset, play a distinct double-chirp at center (`0`), and buzz at travel limits. Trim changes auto-save to non-volatile storage after a brief idle delay.
- **Throttle Trim Modes** (`Radio Setup -> Thr Trim`):
  - `OFF`: Throttle trim locked out.
  - `IDLE`: Affects only the low end (idle stop) without altering full throttle.
  - `LINEAR`: Shifts the entire throttle axis uniformly.
- **Acoustic Variometer** (`Model Setup -> Vario`): Uses native telemetry vertical speed (CRSF/S.Port/AFHDS 2A) or altitude-derived climb rate. Supports configurable deadband, climb cadence, sink tone modes, and automatic throttle lockout.
- **Voice Announcements** *(STM32F4 radios with speaker/DAC)*: Configurable in `Radio Setup` for arming, timer countdowns, and telemetry callouts.

---

## 4. Menu System Reference

| Submenu | Purpose & Key Settings |
| :--- | :--- |
| **Model Select** | Switch between 20 independent CRC-protected model slots (`SCHEMA_VERSION = 2`), rename, copy, or factory-reset a model. |
| **Model Setup** | Model name, RF protocol (`CRSF`, `CC2500`, `MPM`, `DSM`, `AFHDS 2A`), internal/external bay route, Arm switch & motor polarity (unidirectional `988 µs` vs bidirectional `1500 µs`), Timer setup, Telemetry slots, Vario, and Soaring. |
| **Dual Rate / Expo** | Per-axis High/Low Dual Rates (`30..100%`), cubic Expo (`-100..+100%`), and D/R switch assignment. Computed division-free in ±1024 Q10 fixed-point. |
| **Throttle Curve** | 5-, 7-, or 9-point linear or Catmull-Rom spline curve (`ThC`, source ID `58`). |
| **Wing / Mixer** | Predefined mixers (`Normal`, `Delta`/Elevon, `VTail`, `Flaperon`, `DiffThrust`, `Tank` tracked-vehicle mixer, `MTail`, `Crow` butterfly) plus 32 freeform mix lines (`Add` / `Mult` / `Repl`, weight `-125..+125%`, offset, switch & curve gating). |
| **Aux Channels** | Map any of the 18 channels (`CH1..CH18`) to sticks (`Ail/Ele/Thr/Rud`), `ThC`, pots/sliders, switches (`SA..SH`), logical switches (`L1..L8`), or `Off`. |
| **Channel Reverse** | Per-channel inversion bitmask. **Safety-after-reverse** guarantees disarmed throttle always outputs the safe pulse (`988 µs` or `1500 µs`) regardless of CH3 reverse state. |
| **Logical Switches** | 8 programmable logic gates (`L1..L8`) with comparison operators, edge/sticky modes, and millisecond delay/duration timers. |
| **Protocol Setup** | On-radio **CRSF / ELRS Configurator** (packet rate, TX power, bind, Wi-Fi), **CC2500** (13 TI CC2500 protocols), **MPM 4-in-1** (25 protocols including extended IDs `>= 64`), **DSM / Forward Programming**, and **AFHDS 2A** binding/failsafe. |
| **Radio Setup** | Stick mode (Mode 1–4), LCD contrast & backlight timeout/brightness, battery warning voltage, inactivity alarm, audio/voice settings, trainer PPM In/Out, and USB mode. |
| **Stick Calibration** | Interactive 2-step center + full-throw calibration for gimbals and pots/sliders. |
| **Diagnostics** | Live ADC counts, switch/button matrix test, LCD pixel test, and **RF / UART Diagnostics** (`BAY`, `INT`, active `USART`, baud/framing, live `RXB` byte counter, and `TXE` error counter). |
| **System Info** | Firmware version, build feature tags (`CR`, `CC`, `MP`, `DS`, `MS`, `SO`, `LS`), storage health, and MCU UID. |

---

## 5. Flight Timers & Haptics

- **Triggers (`T-Trig`)**: `Off`, `Always`, `Thr > 10%`, `Arm Switch`, or any physical/logical switch.
- **Arm Integration**: When linked to the Arm switch, arming automatically starts/resets the timer and disarming pauses it.
- **Hold-to-Reset**: Long-pressing the timer reset key displays a visual progress bar before zeroing the timer.
- **Audio Cadence**: Speaks/beeps minute marks, 30s/20s/10s callouts, 5-4-3-2-1 countdown, and distinct overtime alerts.
- **Pot Center Detent**: Crossing center on continuous pots triggers a crisp audio/haptic tick so you can feel neutral without looking down.

---

## 6. Gimbal & Potentiometer Calibration

1. Open **`System -> Stick Calibration`**.
2. **Step 1 (Center)**: Place all gimbals and center-detent pots at exact mechanical center and press **`[OK]`**.
3. **Step 2 (Endpoints)**: Move both gimbals through full circular/corner travel and sweep every pot/slider end-to-end. Verify min/max values update smoothly, then press **`[OK]`** to save.

---

## 7. RF Protocols, Binding & External Module Bays

### Protocol Selection & Separation
- **CRSF / ExpressLRS**: Uses `400k`–`1.87M` 8N1. Configure rate, power, bind, and Wi-Fi directly from `Protocol Setup -> ELRS Config` (no Lua scripts required). Optional **CRSF MultiSwitch (`Set4`)** supports up to 4×8-switch banks (`--features multiswitch`).
- **CC2500**: Dedicated 13-protocol catalog (`FrSky D/X/X2`, `SFHSS`, `Corona`, `Hitec`, `Redpine`, `RadioLink`, etc.) over `100k 8E2`.
- **MPM 4-in-1**: Dedicated 25-protocol catalog (`DSM`, `FlySky/AFHDS2A`, `FrSky`, `Bayang`, `Hubsan`, `SymaX`, `HoTT`, `OMP`, `M-Link`, etc.) with automatic 27-byte extended frame support for protocol IDs `>= 64`.
- **AFHDS 2A** *(FlySky FS-i6X / FS-i6S)*: Native internal A7105 driver supporting 18-channel i-BUS, two-way telemetry receivers (`FS-iA6B`/`FS-iA10B`), and one-way receivers (`FS-A8S`/`Fli14`).

### External Bay UART Routing (STM32F4)
Ferrox automatically configures the right STM32F4 USART for both **8-pin Nano** (Pocket, Zorro, GX12, T-Pro V2) and **5-pin JR** (TX12, Boxer, QX7, T12, X9D+, X9E, AT9S Pro) bays:
- **Internal RF**: `USART1` (`PB6`/`PB7`). Use OEM build flags (`internal-elrs`, `internal-cc2500`, `internal-mpm`) if your radio shipped with a CC2500 or 4-in-1 internal module.
- **External CRSF Half-Duplex**: `USART2` (`PD5`/`PD6` + `PD4` direction control) on S.Port / Pin 5.
- **External CRSF Full-Duplex**: `USART6` (`PC6`/`PC7`) on Pins 1/2.
- **External MPM / CC2500 / DSM**: Split `USART6` TX (`PC6`, Pin 1) + `USART2` RX (`PD6`, Pin 5).

> Verify your active bay, USART, baud/framing, and live RX/TX counters anytime in **`System -> Diagnostics -> RF / UART`**.

---

## 8. Firmware Flashing, Backup & Recovery

> [!IMPORTANT]
> **Schema v2 Notice (`v1.0.0-rc12+`)**: `v1.0.0-rc12` introduces a clean `SCHEMA_VERSION = 2` storage layout. Upgrading from `rc11` or earlier resets stored models to factory defaults — note your model settings before flashing and re-calibrate sticks after first boot.

### Option A: Ferrox Web Flasher (Recommended)
Use the browser-based **Ferrox Web Flasher** to select your radio, toggle custom features (e.g., `multiswitch`, `internal-cc2500`, `soaring`), and flash over USB DFU without installing a Rust toolchain.

### Option B: EdgeTX SD Bootloader (STM32F4, No Drivers Needed)
1. Build an EdgeTX-padded `.bin`:
   ```bash
   ./tools/build-sd-bin.sh pocket   # or tx12, boxer, zorro, gx12, t12, qx7, x9d, x9e, at9spro
   ```
2. Copy the generated `.bin` into `/FIRMWARE/` on the radio's microSD card.
3. Boot into the EdgeTX bootloader (hold both horizontal trims inward + Power) and select **Write Firmware**.

### Option C: Direct USB DFU & Stock Flash Backup
1. **Enter DFU Mode**: Power off the radio, hold the bootloader trigger (or use `System -> DFU Mode`), and connect USB.
2. **Always Back Up Stock Flash First**:
   ```bash
   # STM32F4 (512 KB):
   dfu-util -a 0 -s 0x08000000:524288 -U stock_backup.bin
   # FlySky FS-i6X / FS-i6S (128 KB):
   dfu-util -a 0 -s 0x08000000:131072 -U stock_backup.bin
   ```
3. **Flash Ferrox**:
   ```bash
   dfu-util -a 0 -s 0x08000000:leave -D ferrox-pocket.bin
   ```
4. **Restore Anytime**: Flash `stock_backup.bin` back to `0x08000000:leave`.

---

## 9. Safety Systems & Watchdog Recovery

- **Pre-Flight Startup Check**: Blocks RF transmission at boot if throttle is above minimum or a configured switch is out of its safe position.
- **Battery Low-Voltage Alarm**: Configurable warning threshold (`Radio Setup -> VBat Warn`) triggers periodic audio and on-screen alerts.
- **Inactivity Alarm**: Beeps if no stick or button moves for the configured idle window.
- **RSSI / Link-Lost Alarm**: Alerts when telemetry link quality or RSSI drops below threshold or goes stale.
- **Hardware Watchdog (`IWDG`)**: If a fault ever resets the MCU in flight, Ferrox detects the watchdog reset flag, skips the splash screen and pre-flight check, and restores RF control in milliseconds.

---

## 10. USB Simulator, Silent RF & Serial CLI

Configure in **`Radio Setup -> USB Mode`**:
- **Joystick (HID Gamepad)**: 100 Hz USB HID controller compatible with Liftoff, Velocidrone, RealFlight, Uncrashed, and TrypFPV. Entering USB Simulator mode automatically places the RF module in **Silent RF Standby** (zero RF emission).
- **Serial (CDC-ACM)**: Virtual COM port exposing diagnostic commands (`telem`, `stream`, `storage`, `version`) and JSON telemetry streaming.
- **Composite**: Simultaneous HID Joystick + CDC-ACM Serial.

---

## 11. Soaring & Competition Tools (F5J)

Enable in **`Model Setup -> Soaring`**:
- **5-Phase FAI F5J Lifecycle**: `Ready` -> `Motor Run` (30 s auto-cutoff) -> `Zoom` (10 s max-altitude window) -> `Glide` (10-minute working window) -> `Landed` (auto-landing detection ignores ground handling).
- **Scoring & Logging**: Calculates launch height penalty (`0.5 pts/m` up to `200 m`, `3.0 pts/m` above `200 m`), invalidates score on motor restart, and renders the full flight profile on **Dashboard Page 6/6**.
