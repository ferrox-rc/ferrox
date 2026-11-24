# Ferrox TODO & Release Roadmap

This roadmap tracks unresolved issues, safety fixes, and feature requests reported during community field testing (notably on the RC Groups official thread and GitHub Issues through `v1.0.0-rc7`).

## 🏁 Release Checkpoints

1. **`v1.0.0-rc12` $\rightarrow$ `v1.0.0` (Production Gate):** Consolidates the division-free ±1024 Q10 mixer & clean `SCHEMA_VERSION = 2` storage, PR #38 Tank mixer & `AUX_SOURCES` stick parity (#40), PR #37 CRSF MultiSwitch `Set4`, declarative RF/UART board profiles + on-radio `RF / UART` diagnostic page (#45/#47/#48), and the full documentation prune ahead of tagging `v1.0.0`.
2. **`v1.0.1` (Patch Release — Post-Release Polish):** Low-risk bug fixes and community polish items (battery calibration menu, timer latch/trigger polish, horizontal roller edit polarity consistency, and F5J 10s step & `"ZOOM"` label).
3. **`v1.1.0` (Minor Release — Controls, Mixer & UI Capabilities):** Structural input/mixer/UI enhancements (unrestricted `CH1..CH18` channel assignment without requiring `Freeform` template, Logical Switches `L1..L8` as universal triggers, momentary switch software latch/toggle mode, telemetry dashboard Timers 1–4 picker, separate Vario volume & pot-assigned master volume from #43, and dedicated F5J competition HUD).

---

## 🚨 Priority 0: Safety & Stability Blockers (Completed through `v1.0.0-rc10` — Gate for `v1.0.0`)

These issues directly affect flight safety or hardware operation and were resolved ahead of declaring production `v1.0.0`.

- [X] **`v1.0.0-rc10` Gate Fixes & `CC2500` / `MPM 4-in-1` Split *(Completed in `v1.0.0-rc10`)***
  - **False `"LINK ACTIVE!"` Shutdown Warning (#44):** Ignored TX-only `ELRS_STATUS` heartbeats and required `uplink_link_quality > 0` in `LINK_STATISTICS` (`crsf/mod.rs`, `crsf/protocol.rs`).
  - **Vario Switch Assignment & Preview (#43):** Added `[...]` edit brackets on `Switch:` in `VARIO SETUP` and respected `sw_active` during live preview (`ui/menu/screens/model.rs`, `runtime.rs`).
  - **Channel Assignments Resetting from `None` on Reboot:** Removed legacy `aux_channels[0..6] == [0; 6]` overwrite in `ModelConfig::sanitize()` (`storage.rs`).
  - **Mode 1 Trim Mapping, STM32F4 Trim Direction & Zorro Trim Beeps:** Mapped trims per active `StickMode` (`trim.rs`, `runtime.rs`), fixed inverted STM32F4 trim bit polarity (`ferrox-stm32f4/src/keys.rs`), and bumped `trim_step` tone to `65 ms` (`buzzer.rs`).
  - **Dual-Module UART UI Parity & `CC2500` / `MPM 4-in-1` Split:** Added `rf-mpm` Cargo feature and `RfProtocol::Mpm = 4` (`board/mod.rs`, `qx7_family.rs`, `setup.rs`), wired `Module: Internal / External (Bay)` across `CRSF`, `CC2500`, `MPM 4-in-1`, and `DSM`, and derived `DEFAULT_PROTOCOL` from `SUPPORTED_PROTOCOLS[0]`.

- [X] **Dual-Module UART Routing (Internal RF vs. External Module Bay) *(Completed in `v1.0.0-rc9`)***
  - *Issue:* `crates/ferrox-stm32f4/src/uart.rs` previously hardcoded a single `USART1` (`PB6/PB7`) serial port, and `ferrox-core` had a single `RfProtocol` selector. STM32F4 radios have separate internal RF (`USART1`, `PB6/PB7`) and external module bay (`USART6`, `PC6/PC7`) UARTs/power pins.
  - *Implemented:* Added hardware dual-UART routing with DMA streams on STM32F4, `SerialPort::set_port_route()`, `rf_module_target()` model configuration storage, and dynamic UI toggle (`Module: Internal` vs `Module: External (Bay)`) in `SETUP` menu.

- [X] **Power-Off Warning with Active Telemetry / Connected RX**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* Radios with soft power switches (Zorro, Pocket, TX12v2) allow immediate shutdown without warning while a model is still bound and communicating.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/power/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/power/mod.rs#L136-L171) (`PowerManager::update`): Currently shuts down immediately when `hold_duration_ms >= SHUTDOWN_HOLD_MS` without checking RF link or active telemetry.
    - [`crates/ferrox-core/src/board/qx7_family.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/qx7_family.rs#L107): Location where `power_manager.update()` is called during board loop.
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L270-L335): Telemetry connection status (`telem.connected`). Pass connection state to power manager to trigger an alert/confirmation modal before cutting power.

- [X] **Audible Telemetry Lost Alarm**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* No beeps, tones, or voice alert when telemetry link is lost.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L542-L567): Audio callouts and telemetry polling tier. Edge detection for `flight.telem.connected` transitioning from `true` to `false`, plus menu-wide RSSI low/critical alarms.
    - [`crates/ferrox-core/src/buzzer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/buzzer.rs#L494-L506) (`Buzzer::warn_telemetry_lost`): Trigger distinctive recurring alarm tone sequence.
    - [`crates/ferrox-core/src/audio/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/audio/mod.rs#L541-L549): Enqueue `telem_lost.wav` ("Telemetry Lost") voice callout.

- [X] **STM32F4 Tone / Buzzer Driver & Variometer Fix**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545)), WimH ([Post #34](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249519))
  - *Issue:* No variometer tones through speaker, no startup chime, no button beeps, and no trim beeps on STM32F4 radios (TX12v2, Zorro), even though WAV voice playback works.
  - *Focus Files & Functions:*
    - [`crates/ferrox-stm32f4/src/buzzer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/buzzer.rs#L39-L115) (`DacToneBuzzer::init`, `on`, `off`): Replaced invalid `PA4` `AF2` (`TIM5_CH1`) config with `DAC1_OUT` (`PA4` Analog) + `TIM5_TRGO` + `DMA1_Stream5` circular 32-sample sine wave LUT.
    - [`crates/ferrox-stm32f4/src/audio.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/audio.rs#L83-L178): Shared `DAC1` + `DMA1_Stream5` + `PE12` (`AUDIO_MUTE`) hardware helpers between PCM voice driver and tone buzzer.

- [X] **Audible / Voice Low Battery Warning**
  - *Reported by:* WimH ([Post #34](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249519))
  - *Issue:* Low battery state only flashes on-screen voltage; no beeper tone or voice warning is played.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L569-L580): Battery monitoring loop in `BackgroundIdleManager::tick` triggers `buzzer.warn_battery()` and `lowbatt.wav` voice alert across all screens.
    - [`crates/ferrox-core/src/ui/dashboard/status_bar.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/status_bar.rs#L80-L108): Handles visual blinking gauge/voltage display.

- [X] **TX12v2 DFU / Boot Mode Freeze Bug**
  - *Reported by:* Johan Lindell ([Post #35](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100#post55249545))
  - *Issue:* Manually entering DFU/boot mode causes radio to lock up over USB, requiring battery removal.
  - *Focus Files & Functions:*
    - [`crates/ferrox-stm32f4/src/chip.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-stm32f4/src/chip.rs#L249-L302) (`rcc_deinit`, `enter_dfu_bootloader`): Reset `AHB2RSTR` (`USB_OTG_FS`), `PLLCFGR`, and `GPIOA`/`GPIOB`/`GPIOC` while preserving `GPIOD`/`GPIOE` (`PD0` power latch) before jumping to System ROM bootloader at `0x1FFF_0000`.

---

## 🔧 Target `v1.0.1` Patch Candidates: Bug Fixes, Parity & Polish

- [ ] **Full Documentation Review *(High Priority — Post-`v1.0.0`)***
  - *Scope:* Comprehensive audit and update of `README.md`, `docs/USER_GUIDE.md`, `docs/ARCHITECTURE.md`, `docs/ROADMAP.md`, and Web Flasher guides to reflect `v1.0.0` features (dual-module UART routing, `CC2500` vs. `MPM 4-in-1` split, custom build `-sd.bin` vs. DFU flashing, restoring EdgeTX bootloader, Mode 1–4 trims, Vario setup, and F5J soaring).

- [X] **STM32F4 `AUX_SOURCES` Stick Mapping Parity (GitHub Issue #40 — Part 1)**
  - *Reported by:* WimH ([GitHub Issue #40](https://github.com/ferrox-rc/ferrox/issues/40#issuecomment-6970857477))
  - *Issue:* On Zorro (and other STM32F4 `qx7_family` radios), setting `Wing/Tail` to `Freeform` unlocks `CH1..CH18` in the `CHANNELS` screen, but only cycles through switches (`SA..SH`) and pots (`S1`/`S2`)—omitting the primary sticks (`Roll`, `Pitch`, `Thr`, `Yaw`). On `i6x.rs` and `i6s.rs`, `AUX_SOURCES` includes `1, 2, 3, 4` (`Roll`, `Pitch`, `Thr`, `Yaw`), whereas on `zorro.rs` and the other 10 STM32F4 board profiles, `1, 2, 3, 4` were accidentally omitted from `AUX_SOURCES`.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/board/zorro.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/zorro.rs#L48), [`pocket.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/pocket.rs#L45), [`boxer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/boxer.rs#L46), [`tx12.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/tx12.rs#L46), [`qx7.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/qx7.rs#L46), [`tprov2.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/tprov2.rs#L48), [`t12.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/t12.rs#L46), [`x9d.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/x9d.rs#L51), [`x9e.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/x9e.rs#L51), [`at9spro.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/at9spro.rs#L51), [`gx12.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/gx12.rs#L48): Prepend `1, 2, 3, 4` (`Roll`, `Pitch`, `Thr`, `Yaw`) after `0` (`None`) in `AUX_SOURCES` to match [`i6x.rs:126`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/i6x.rs#L126) and [`i6s.rs:132`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/i6s.rs#L132).

- [X] **Rename `"Wing / Mixer"` & `"Wing:"` Menu Labels to Mixer `"Type:"`**
  - *Issue:* The template selector (`NORMAL`, `ELEVON`, `V-TAIL`, `FLAPERON`, `FREEFORM`) is currently labeled `"Wing:"` inside `"WING & MIXER"`, which obscures that it selects the overall mixer template / mode (including `FREEFORM`).
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/mixer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/mixer.rs#L590) (`update_wing_mixer`) & [`line 634`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/mixer.rs#L634) (`render_wing_mixer_row`): Renamed row label to `"Type:"` and header to `"MIXER"`.
    - [`crates/ferrox-core/src/board/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/board/mod.rs#L141) (`MainMenuItem::WingMixer::title`): Renamed main menu item from `"Wing / Mixer"` to `"Mixer"`.

- [X] **Timer Triggers & Latch Behavior**
  - *Issue:* Throttle timer triggers (`Tht` and `Ths`) only evaluate the throttle stick, and the timer "latch" setting does not hold run state properly.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/timer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/timer.rs#L223-L271) (`TimerBank::update`): Gated `THs`/`THt` throttle triggers on `is_armed` and removed unconditional `&& is_armed` blocking from latched and switch-triggered timers.
    - [`crates/ferrox-core/src/runtime.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/runtime.rs#L905-L925): Passes `is_armed_or_unassigned` into `self.timers.update()`.

- [X] **Horizontal Roller Inversion Consistency**
  - *Issue:* Roller works well for vertical list navigation, but left/right cursor movement or variable adjustment is inverted in certain edit screens.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/mod.rs#L421-L435): Swapped `up`/`down` in `NavKeys` when `ActiveBoard::HAS_ROTARY_ENCODER && self.editing` so clockwise/right consistently increments values across all edit screens.

- [X] **Separate Native CC2500 from Multi-Protocol Module (MPM)**
  - *Issue:* Choosing CC2500 lists protocols unsupported by the CC2500 RF chip because internal CC2500 and serial MPM share a menu enum.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/rf/cc2500.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/rf/cc2500.rs) & [`crates/ferrox-core/src/rf/mpm.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/rf/mpm.rs): Split native CC2500 (`Cc2500Protocol`) from 4-in-1 MPM (`MpmProtocol` / `MPM_PROTOCOLS`).

- [X] **F5J Window Time Granularity (10s Steps) & Rename 10s Window to "Zoom"**
  - *Issue:* Window time step size is 30 seconds; pilots need 10-second steps (e.g., 10:20 or 15:20) to synchronize with competition horn countdowns, and the 10s post-cutoff phase is standardly called "Zoom".
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/model.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/model.rs#L787-L795): Changed timer duration edit increment from `+= 30` / `-= 30` to `+= 10` / `-= 10` seconds.
    - [`crates/ferrox-core/src/soaring/f5j/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/mod.rs#L28-L46) (`F5jPhase::Zoom`): Renamed from `F5jPhase::Window` and updated `as_str()` from `"WIN"` to `"ZOOM"`.
    - [`crates/ferrox-core/src/ui/dashboard/pages/f5j.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/f5j.rs#L113): Updated dashboard phase reference.

- [ ] **Documentation & Community Workflow (`v1.0.1`)**
  - **Document & Polish Voice Settings:** Clarify `"Voice Intv"` (periodic telemetry repeat interval) in [`setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L234-L242), [`storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L415-L445), and [`docs/USER_GUIDE.md`](file:///home/bryansouza/Repos/ferrox/docs/USER_GUIDE.md).
  - **RadioMaster Zorro User Manual:** Create `docs/RADIO_ZORRO.md` covering key mappings (`MDL`, `SYS`, `TELE`, `RTN`, Roller), CC2500/ELRS setup, and battery calibration.
  - **Direct Testers to GitHub Issues:** Add thread banner / note encouraging bug tracking and regression logging directly on GitHub Issues.

---

## 🚀 Target `v1.1.0` Minor Release: Controls, Mixer & UI Feature Expansions

- [ ] **Unrestricted Channel Assignment Without Requiring `Freeform` Template (GitHub Issue #40 — Part 2)**
  - *Issue:* Allow reassigning `CH1..CH4` in the `CHANNELS` screen per model without first having to switch `Wing/Tail` to `Freeform`, while keeping wing templates optional.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/menu/screens/channels.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/channels.rs#L41-L95) (`update_aux_channels`): Remove restriction where channels 1–4 are hidden unless `wing_tail_mix == 4` (`is_freeform`).
    - [`crates/ferrox-core/src/mixer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/mixer.rs#L612-L700) (`compute_channels`): Allow `freeform_ch1_4` overrides or per-channel source mapping cleanly alongside wing templates.

- [ ] **Logical Switches as Selectable Input Triggers**
  - *Issue:* Switch selection lists (e.g., Motor Arming, Timers, Special Functions) only display physical hardware switches (`SA..SH`).
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/mixer.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/mixer.rs#L140-L200) (`is_switch_active`): Extend switch condition indexes beyond physical switches (1..28) to include Logical Switches `L1..L8` (`29..=36` normal, `37..=44` inverted) across all UI pickers.
    - [`crates/ferrox-core/src/ui/menu/screens/model.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/model.rs#L788-L825): Timer Start, Stop, and Reset switch selection loops.
    - [`crates/ferrox-core/src/ui/menu/screens/setup.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/menu/screens/setup.rs#L850-L920): Logical switch configuration screen.

- [ ] **Momentary Switch Toggle / Latch Mode (Push-On / Push-Off)**
  - *Issue:* Momentary switches are only active while physically held down.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/input.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/input.rs#L400-L470) (`Switches::update`): Add software latching/toggle state tracking for momentary inputs (e.g. `SH` or `SF`).
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L250-L320) (`AdcInputMode` / switch modes): Allow designating momentary switches as sticky toggles.

- [ ] **Telemetry Flight Timer Picker (Timers 1–4)**
  - *Issue:* Telemetry screen only offers the legacy single timer rather than selecting any of the 4 independent flight timers.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs#L270-L275), [`lines 540-565`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/telemetry.rs#L540-L565): Add Timer 1, Timer 2, Timer 3, and Timer 4 as selectable tile sensor types.
    - [`crates/ferrox-core/src/ui/dashboard/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/mod.rs#L150-L167): Pass reference to `TimerBank` into `pages::telemetry::render`.
    - [`crates/ferrox-core/src/storage.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/storage.rs#L650-L670): Telemetry tile configuration enum.

- [ ] **Dedicated F5J Flight Display Screen**
  - *Issue:* WimH requested a dedicated soaring flight layout:
    - Large font: Remaining Window Time (counting down).
    - Smaller font: Motor run time (counting up to 30s) and Flight time (counting up).
    - Automatic switch to Landing Points input screen upon touchdown detection or window expiration.
  - *Focus Files & Functions:*
    - [`crates/ferrox-core/src/ui/dashboard/pages/f5j.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/ui/dashboard/pages/f5j.rs#L50-L260): Implement dedicated large-font soaring HUD page.
    - [`crates/ferrox-core/src/soaring/f5j/score.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/score.rs): Score calculation and landing bonus tape input prompt.
    - [`crates/ferrox-core/src/soaring/f5j/mod.rs`](file:///home/bryansouza/Repos/ferrox/crates/ferrox-core/src/soaring/f5j/mod.rs#L30-L34): Touchdown auto-transition hook.

- [ ] **Main Screen Timer & Expanded 3-Line Large-Font Custom Telemetry *(Pending further user input)***
  - *Reported by:* Johan Lindell ([Post #54](https://www.rcgroups.com/forums/showthread.php?4966503-Official-Ferrox-Radio-OS-Thread&pp=100))
  - *Issue:* Place the primary timer on the main screen (similar to EdgeTX) with a larger font for quick in-flight glances, and expand `"Custom Telem"` from 2 lines to 3 lines (6 sensors/values) with larger numeric font sizes. *(Note: Pending further user input before finalizing layout changes.)*

- [ ] **Battery Voltage Calibration Menu**
  - *Issue:* Allow users to fine-tune the displayed battery voltage against a multimeter reading via a calibration offset/multiplier in Radio Setup (or Diag -> Analog), persisted in radio storage.


