# Ferrox TODO & Release Roadmap

This roadmap tracks unresolved issues, safety fixes, and feature requests reported during community field testing (notably on the RC Groups official thread through `v1.0.0-rc6`).

---

## 🚨 Priority 0: Safety & Stability Blockers (Must-Have for v1.0.0 / RC7)

These issues directly affect flight safety or hardware operation and must be resolved in a release candidate before declaring a production `v1.0.0`.

- [ ] **Power-Off Warning with Active Telemetry / Connected RX**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* Radios with soft power switches (Zorro, Pocket, TX12v2) allow immediate shutdown without warning while a model is still bound and communicating.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/power/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/power/mod.rs#L136-L171) (`PowerManager::update`): Currently shuts down immediately when `hold_duration_ms >= SHUTDOWN_HOLD_MS` without checking RF link or active telemetry.
    - [`crates/ferrox-core/src/board/qx7_family.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/qx7_family.rs#L107): Location where `power_manager.update()` is called during board loop.
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L270-L335): Telemetry connection status (`telem.connected`). Pass connection state to power manager to trigger an alert/confirmation modal before cutting power.

- [ ] **Audible Telemetry Lost Alarm**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* No beeps, tones, or voice alert when telemetry link is lost.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L1489-L1506): Audio callouts and telemetry polling tier. Add edge detection for `flight_snapshot.telem.connected` transitioning from `true` to `false`.
    - [`crates/ferrox-core/src/buzzer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/buzzer.rs#L18-L80) (`Buzzer::play_tone_pattern`): Trigger distinctive recurring alarm tone sequence.
    - [`crates/ferrox-core/src/audio/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/audio/mod.rs): Enqueue `telem_lost.wav` ("Telemetry Lost") voice callout.

- [ ] **STM32F4 Tone / Buzzer Driver & Variometer Fix**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545)), WimH ([Post #34](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249519))
  - *Issue:* No variometer tones through speaker, no startup chime, no button beeps, and no trim beeps on STM32F4 radios (TX12v2, Zorro), even though WAV voice playback works.
  - *Focus Files & Functions:*
    - [`crates/ferrox-stm32f4/src/buzzer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/buzzer.rs#L18-L106) (`DacToneBuzzer::init`, `on`, `off`): Debug `TIM5_CH1` PWM generation on `PA4`, verify `PE12` (`AUDIO_MUTE`) logic level and timer clock initialization.
    - [`crates/ferrox-stm32f4/src/audio.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/audio.rs#L15-L22), [`lines 185-200`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/audio.rs#L185-L200): Verify atomic flag `AUDIO_ACTIVE` and `PA4` pin mode switching (AF2 for TIM5 vs Analog for DAC1) when WAV playback finishes.
    - [`crates/ferrox-core/src/trim.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/trim.rs#L100-L140): Trim step and center click tone generation.
    - [`crates/ferrox-core/src/vario.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/vario.rs#L120-L170) (`VarioState::update`): Verify vario pitch/pulse modulation feeds into active buzzer.

- [ ] **Audible / Voice Low Battery Warning**
  - *Reported by:* WimH ([Post #34](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249519))
  - *Issue:* Low battery state only flashes on-screen voltage; no beeper tone or voice warning is played.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L860-L920): Battery monitoring loop. Check transmitter voltage against configured threshold and trigger audible warning.
    - [`crates/ferrox-core/src/ui/dashboard/status_bar.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/status_bar.rs#L30-L70): Currently handles visual blinking; pair with `buzzer.play_tone(...)` or audio alert callout.

- [ ] **TX12v2 DFU / Boot Mode Freeze Bug**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* Manually entering DFU/boot mode causes radio to lock up over USB, requiring battery removal.
  - *Focus Files & Functions:*
    - [`crates/ferrox-stm32f4/src/chip.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/chip.rs#L310-L354) (`rcc_deinit`, `enter_dfu_bootloader`): Ensure all GPIOs (especially USB OTG PA11/PA12), interrupts, and clocks are completely reset before jumping to System ROM bootloader at `0x1FFF_0000`.
    - [`crates/ferrox-stm32f4/src/power.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/power.rs#L20-L60): Verify the power latch pin (e.g. `PD0`) is sustained and not dropped during ROM entry.
    - [`crates/ferrox-core/src/boot.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/boot.rs#L170-L202) (`check_dfu_entry`, `reboot_to_dfu`).

---

## 🎛️ Priority 1: Controls, Inputs & Mixing Refinements (WimH List)

- [ ] **Logical Switches as Selectable Input Triggers**
  - *Issue:* Switch selection lists (e.g., Motor Arming, Timers, Special Functions) only display physical hardware switches (SA..SH).
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/mixer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/mixer.rs#L50-L100) (`is_switch_active`): Extend switch condition indexes beyond physical switches (1..28) to include Logical Switches `L1..L8` (and virtual inverted states).
    - [`crates/ferrox-core/src/ui/menu/screens/model.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/model.rs#L788-L825): Timer Start, Stop, and Reset switch selection loops.
    - [`crates/ferrox-core/src/ui/menu/screens/setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L850-L920): Logical switch configuration screen.

- [ ] **Momentary Switch Toggle / Latch Mode (Push-On / Push-Off)**
  - *Issue:* Momentary switches are only active while physically held down.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/input.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/input.rs#L400-L470) (`Switches::update`): Add software latching/toggle state tracking for momentary inputs (e.g. `SH` or `SF`).
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L250-L320) (`AdcInputMode` / switch modes): Allow designating momentary switches as sticky toggles.

- [ ] **Free Receiver Channel Assignment & Stick Mapping Per Model**
  - *Issue:* Channel assignments and inputs are still constrained by legacy FS-i6X architecture; cannot freely assign stick inputs to output channels per model.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/channels.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/channels.rs#L41-L95) (`update_aux_channels`): Remove restriction where channels 1-4 are hardcoded unless `wing_tail_mix == 4` (`is_freeform`).
    - [`crates/ferrox-core/src/board/qx7_family.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/qx7_family.rs#L289), [`crates/ferrox-core/src/board/zorro.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/zorro.rs#L48) (`AUX_SOURCES`): Add primary stick inputs (1..4: Ail, Ele, Thr, Rud) to `AUX_SOURCES` list so any channel can map to any stick.
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L1050-L1085) (`next_available_aux_source` / `prev_available_aux_source`).

- [ ] **Timer Triggers & Latch Behavior**
  - *Issue:* Throttle timer triggers (`Tht` and `Ths`) only evaluate the throttle stick, and the timer "latch" setting does not hold run state properly.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/timer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/timer.rs#L258-L272) (`TimerBank::update`): Bug where `state.is_running = state.latched_running && is_armed;` unconditionally blocks timers on models without an arm switch configured.
    - [`crates/ferrox-core/src/timer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/timer.rs#L223-L243): Decouple `Tht`/`Ths` from raw stick value to support channel throttle assignment.
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L865-L880): Verify `is_armed_or_unassigned` passed to `self.timers.update()`.

- [ ] **Horizontal Roller Inversion Consistency**
  - *Issue:* Roller works well for vertical list navigation, but left/right cursor movement or variable adjustment is inverted in certain edit screens.
  - *Focus Files & Functions:*
    - [`crates/ferrox-stm32f4/src/keys.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/keys.rs#L135-L155) (`Qx7KeyDriver::read_keys`): Rotary encoder transition quadrature accumulator and polarity logic.
    - [`crates/ferrox-core/src/ui/menu/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/mod.rs#L260-L285) & [`lines 411-420`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/mod.rs#L411-L420): Mapping of encoder pulses to `NavKeys` (`up`/`down`).
    - [`crates/ferrox-core/src/ui/menu/screens/setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L858-L915): Inverted increment/decrement direction during parameter editing.
    - [`crates/ferrox-core/src/ui/menu/screens/soaring.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/soaring.rs#L126-L165): `let dir: i16 = if keys.up { 1 } else { -1 };`.

---

## 📡 Priority 2: Protocol Selection & UI Clean-up

- [ ] **Separate Native CC2500 from Multi-Protocol Module (MPM)**
  - *Issue:* Choosing CC2500 lists protocols unsupported by the CC2500 RF chip because internal CC2500 and serial MPM share a menu enum.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/rf/mpm.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/rf/mpm.rs#L26-L100) (`Cc2500Protocol`): Currently contains FlySky (A7105) and DSMX (CYRF6936) which are only supported on 4-in-1 MPM, not discrete CC2500. Split into native CC2500 vs MPM 4-in-1 protocol lists.
    - [`crates/ferrox-core/src/ui/menu/screens/setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L882-L893): Protocol selection and cycling in setup menu.

- [ ] **Telemetry Flight Timer Picker (Timers 1–4)**
  - *Issue:* Telemetry screen only offers the legacy single timer rather than selecting any of the 4 independent flight timers.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs#L270-L275), [`lines 540-565`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs#L540-L565): Add Timer 1, Timer 2, Timer 3, and Timer 4 as selectable tile sensor types.
    - [`crates/ferrox-core/src/ui/dashboard/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/mod.rs#L150-L167): Pass reference to `TimerBank` into `pages::telemetry::render`.
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L650-L670): Telemetry tile configuration enum.

- [ ] **Document & Polish Voice Settings**
  - *Issue:* Unclear purpose of "Voice Intv" in Radio Setup.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L234-L242): Clarify label or add help text ("Voice Int:" periodic telemetry repeat interval).
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L385-L415) (`voice_interval_secs`, `cycle_voice_interval`).
    - [`docs/USER_GUIDE.md`](file:///home/bryansouza/Repos/ferrox/docs/USER_GUIDE.md): Document sound pack and voice interval settings.

---

## 🪂 Priority 3: F5J Soaring Engine & Competition Tooling

- [ ] **F5J Window Time Granularity**
  - *Issue:* Window time step size is 30 seconds; pilots need 10-second steps (e.g., 10:20 or 15:20) to synchronize with competition horn countdowns.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/model.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/model.rs#L772-L781): Change timer duration edit increment from `+= 30` / `-= 30` to `+= 10` / `-= 10` seconds.

- [ ] **Rename 10s Window to "Zoom"**
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/soaring/f5j/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/mod.rs#L28-L46) (`F5jPhase::Window`): Rename to `F5jPhase::Zoom`, update `as_str()` from `"WIN"` to `"ZOOM"` or `"ZM"`.
    - [`crates/ferrox-core/src/ui/dashboard/pages/f5j.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/f5j.rs#L1-L30): Update dashboard phase text rendering.

- [ ] **Dedicated F5J Flight Display Screen**
  - *Issue:* WimH requested a dedicated soaring flight layout:
    - Large font: Remaining Window Time (counting down).
    - Smaller font: Motor run time (counting up to 30s) and Flight time (counting up).
    - Automatic switch to Landing Points input screen upon touchdown detection or window expiration.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/dashboard/pages/f5j.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/f5j.rs#L50-L260): Implement dedicated large-font soaring HUD page.
    - [`crates/ferrox-core/src/soaring/f5j/score.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/score.rs): Score calculation and landing bonus tape input prompt.
    - [`crates/ferrox-core/src/soaring/f5j/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/mod.rs#L30-L34): Touchdown auto-transition hook.

---

## 📖 Priority 4: Documentation & Community Workflow

- [ ] **RadioMaster Zorro User Manual**
  - Create `docs/RADIO_ZORRO.md` covering key mappings (MDL, SYS, TELE, RTN, Roller), CC2500/ELRS setup, and battery calibration.
- [ ] **Direct Testers to GitHub Issues**
  - Add thread banner / note encouraging bug tracking and regression logging directly on GitHub Issues.
