//! Platform-agnostic flight runtime execution pipeline and background idle manager.
//!
//! Orchestrates:
//! - Multi-kHz Flight Pipeline: stick reading, mixing, curves, trims, RF publication, and USB polling.
//! - 30 Hz Background Idle Manager: inactivity alarms, backlight auto-dim, timer engine,
//!   pot center haptic/audio clicks, bind gestures, debounced flash saves, and display frame dispatch.
//! - Startup Pre-flight Safety Interlock: throttle at idle and switch safety verification.

use crate::board::{ActiveBoard, BoardIdleHook, BoardProfile};
use crate::buzzer::Buzzer;
use crate::calib::CalibWizard;
use crate::crsf;
use crate::curve;
use crate::display::St7567;
#[cfg(feature = "rf-dsm")]
use crate::dsm;
use crate::input::{self, InputState, SwitchPos};
use crate::mixer::{self, SAFE_IDLE_CHANNELS};
use crate::rf::{self, afhds2a::TelemetryData};
use crate::safety::{self, StickMode};
use crate::storage::{self, AdcInputMode, RadioStorage};
use crate::time;
use crate::trainer;
use crate::trim::{ActiveTrim, TrimController};
use crate::ui::dashboard::DashboardController;
use crate::ui::format;
use crate::ui::menu::MenuController;
use crate::usb;
use crate::vario::Vario;
use crate::watchdog;

pub const MAX_POTS: usize = 7;

/// High-rate flight pipeline state and outputs.
pub struct FlightSnapshot {
    pub state: InputState,
    pub rf_chs: [u16; mixer::NUM_CHANNELS],
    pub telem: TelemetryData,
    pub is_binding: bool,
    pub is_armed: bool,
    pub just_armed: bool,
    pub just_disarmed: bool,
}

/// Flight pipeline context: manages stick sampling, mixing, and RF/USB publication.
pub struct FlightPipeline {
    pub prev_armed: bool,
    pub prev_active_model: u8,
    pub telem_reading_idx: u8,
    pub vario: Vario,
    pub logical_bank: mixer::LogicalSwitchBank,
    pub last_tick_ms: u32,
}

impl FlightPipeline {
    pub fn new(storage: &RadioStorage, init_state: &InputState) -> Self {
        let active = storage.active_model();
        let prev_armed = mixer::eval_arm_switch(active.arm_switch, &init_state.switches);
        Self {
            prev_armed,
            prev_active_model: storage.radio.active_model,
            telem_reading_idx: 1,
            vario: Vario::new(),
            logical_bank: mixer::LogicalSwitchBank::new(),
            last_tick_ms: 0,
        }
    }

    /// High-rate flight pipeline execution tick (multi-kHz execution speed).
    /// Polls physical sticks, evaluates throttle curves, computes matrix mixer,
    /// checks arm status, and publishes channels to RF (AFHDS 2A / CRSF) and USB.
    #[inline(never)]
    pub fn tick(
        &mut self,
        now: u32,
        storage: &RadioStorage,
        trims: &TrimController,
        menu_active: bool,
        vario_preview: bool,
        buzzer: &mut Buzzer,
    ) -> FlightSnapshot {
        // 1. Poll continuous DMA analog and digital inputs (sub-microsecond)
        let mut state = input::read();
        ActiveBoard::update_flight_switches(&mut state, storage, menu_active);

        let active_model = storage.active_model();
        let stick_mode = StickMode::from_u8(storage.radio.stick_mode);
        let controls = state.flight_controls(stick_mode);

        // 1b. Update Logical Switches L1..L8 with millisecond delay & duration timers
        let dt_ms = if self.last_tick_ms == 0 {
            0
        } else {
            now.wrapping_sub(self.last_tick_ms).min(100) as u16
        };
        if dt_ms > 0 || self.last_tick_ms == 0 {
            self.last_tick_ms = now.max(1);
        }
        let raw_sticks = [
            controls.aileron,
            controls.elevator,
            controls.throttle,
            controls.rudder,
        ];
        let dummy_chs = [0i32; mixer::NUM_CHANNELS];
        self.logical_bank.update(
            dt_ms,
            &raw_sticks,
            &state.aux_pots[4..],
            &mut state.switches,
            &dummy_chs,
            active_model,
        );

        // 2. Resynchronize arm state when active model changes or when exiting settings menu
        if storage.radio.active_model != self.prev_active_model {
            self.prev_active_model = storage.radio.active_model;
            self.logical_bank.reset();
            self.prev_armed = mixer::eval_arm_switch(active_model.arm_switch, &state.switches);
            self.vario.reset();
        }

        // 3. Check configured Arm Switch condition and play Armed/Disarmed chimes
        let mut just_armed = false;
        let mut just_disarmed = false;
        let is_armed = mixer::eval_arm_switch(active_model.arm_switch, &state.switches);
        if active_model.arm_switch > 0
            && active_model.arm_switch <= 44
            && is_armed != self.prev_armed
        {
            if is_armed {
                just_armed = true;
                if !menu_active {
                    buzzer.chime_armed();
                }
            } else {
                just_disarmed = true;
                if !menu_active {
                    buzzer.chime_disarmed();
                }
            }
            self.prev_armed = is_armed;
        }

        // 4. Evaluate active model throttle curve (normalized 0..MIXER_MAX)
        let thr_input =
            ((controls.throttle + mixer::MIXER_MAX) / 2).clamp(0, mixer::MIXER_MAX) as u16;
        let thr_curved = curve::evaluate_curve(
            thr_input,
            active_model.thr_curve_pts,
            active_model.thr_curve_smooth != 0,
            &active_model.thr_curve,
        );

        // 5. Compute all 14 channels via 4-stage pipeline (D/R, Expo, Matrix Mixer, Trims, Reversing)
        let mut rf_chs = mixer::compute_channels(
            controls.aileron,
            controls.elevator,
            thr_curved,
            controls.rudder,
            &state.aux_pots[4..],
            &state.switches,
            active_model,
            trims,
            storage.radio.throttle_trim,
        );

        // 5b. Safety Handover: Process student PPM input if Trainer In is enabled
        if storage.radio.is_ppm_in_enabled() {
            let trainer_switch_active = state.switches.sd == SwitchPos::Down;
            let mut handover_chs = rf_chs;
            trainer::process_handover(trainer_switch_active, &rf_chs, &mut handover_chs);
            rf_chs = handover_chs;
        }

        let sim_mode = usb::is_sim_mode();

        // 5c. PPM Output: Update 22.5ms CPPM stream on PF10 if PPM Out is enabled
        if !sim_mode && storage.radio.is_ppm_out_enabled() {
            trainer::update_out_channels(&rf_chs);
        }

        // 6. Publish channels to active RF subsystem
        let proto = ActiveBoard::resolve_protocol(active_model.rf_protocol);
        let is_crsf = proto == crate::board::RfProtocol::Crsf;

        match proto {
            crate::board::RfProtocol::Crsf => {
                #[cfg(feature = "rf-dsm")]
                dsm::set_enabled(false);
                rf::mpm::set_enabled(false);
                let crsf_active = !sim_mode;
                crsf::set_enabled(
                    crsf_active,
                    active_model.crsf_baud,
                    active_model.crsf_half_duplex != 0,
                );
                if crsf_active {
                    let arm_mode_switch = active_model.crsf_arm_mode != 0;
                    crsf::update_channels(now, &rf_chs, arm_mode_switch, is_armed);
                    crsf::poll_telemetry(now);
                }
                rf::set_silenced(true);
            }
            #[cfg(feature = "rf-dsm")]
            crate::board::RfProtocol::Dsm => {
                crsf::set_enabled(false, 0, false);
                rf::mpm::set_enabled(false);
                let dsm_active = !sim_mode;
                dsm::set_enabled(dsm_active);
                dsm::set_bind_mode(rf::is_binding());
                rf::set_silenced(true);
                if dsm_active {
                    let sub_proto =
                        crate::board::DsmSubProtocol::from_u8(active_model.dsm_sub_proto);
                    dsm::update_link(now, &rf_chs, sub_proto);
                    dsm::poll_telemetry(now);
                    rf::set_channels(&rf_chs);
                }
            }
            crate::board::RfProtocol::Cc2500 => {
                crsf::set_enabled(false, 0, false);
                #[cfg(feature = "rf-dsm")]
                dsm::set_enabled(false);
                rf::set_silenced(true);
                let mpm_active = !sim_mode;
                rf::mpm::set_enabled(mpm_active);
                if mpm_active {
                    let cc_proto = rf::mpm::Cc2500Protocol::from_u8(active_model.rx_serial_proto);
                    let rx_num = active_model.mpm_rx_num();
                    let freq_tune = active_model.mpm_freq_tune();
                    let range_check = active_model.mpm_range_check();
                    rf::mpm::update_link(
                        now,
                        &rf_chs,
                        cc_proto,
                        rx_num,
                        freq_tune,
                        rf::is_binding(),
                        range_check,
                    );
                    rf::mpm::poll_telemetry(now);
                    rf::set_channels(&rf_chs);
                }
            }
            crate::board::RfProtocol::Afhds2a => {
                crsf::set_enabled(false, 0, false);
                #[cfg(feature = "rf-dsm")]
                dsm::set_enabled(false);
                rf::mpm::set_enabled(false);
                rf::set_silenced(sim_mode);
                if !sim_mode {
                    rf::set_channels(&rf_chs);
                }
            }
        }

        // 7. Map telemetry data based on active protocol
        let telem = match proto {
            crate::board::RfProtocol::Crsf => {
                let ct = crsf::get_telemetry();
                rf::afhds2a::TelemetryData {
                    connected: ct.connected,
                    rssi: ct.uplink_link_quality,
                    rx_voltage_mv: ct.rx_battery_mv,
                    alt_m: ct.baro_alt_m,
                    alt_dm: ct.baro_alt_dm,
                    vspeed_cms: ct.vspeed_cms,
                    has_vspeed: ct.has_vspeed,
                    rpm: ct.rpm,
                    temp_c_x10: ct.temp_c_x10,
                    last_rx_voltage_ms: ct.last_vbat_ms,
                    last_alt_ms: ct.last_alt_ms,
                    last_vspd_ms: ct.last_vspd_ms,
                    last_rpm_ms: ct.last_rpm_ms,
                    last_temp_ms: ct.last_temp_ms,
                    packets_sent: 0,
                    packets_received: 0,
                }
            }
            #[cfg(feature = "rf-dsm")]
            crate::board::RfProtocol::Dsm => {
                let dt = dsm::get_telemetry();
                rf::afhds2a::TelemetryData {
                    connected: dt.connected,
                    rssi: dt.rssi_pct,
                    rx_voltage_mv: dt.rx_voltage_mv,
                    alt_m: 0,
                    alt_dm: 0,
                    vspeed_cms: 0,
                    has_vspeed: false,
                    rpm: 0,
                    temp_c_x10: 0,
                    last_rx_voltage_ms: if dt.connected { now } else { 0 },
                    last_alt_ms: 0,
                    last_vspd_ms: 0,
                    last_rpm_ms: 0,
                    last_temp_ms: 0,
                    packets_sent: 0,
                    packets_received: 0,
                }
            }
            crate::board::RfProtocol::Cc2500 => {
                let mt = rf::mpm::get_telemetry();
                rf::afhds2a::TelemetryData {
                    connected: mt.connected,
                    rssi: mt.rssi,
                    rx_voltage_mv: mt.rx_batt_mv,
                    alt_m: mt.alt_m,
                    alt_dm: mt.alt_dm,
                    vspeed_cms: mt.vspeed_cms,
                    has_vspeed: mt.has_vspeed,
                    rpm: mt.rpm as u32,
                    temp_c_x10: mt.temp_c_x10,
                    last_rx_voltage_ms: mt.last_rx_batt_ms,
                    last_alt_ms: mt.last_alt_ms,
                    last_vspd_ms: mt.last_vspd_ms,
                    last_rpm_ms: mt.last_rpm_ms,
                    last_temp_ms: mt.last_temp_ms,
                    packets_sent: mt.packets_sent as u32,
                    packets_received: mt.packets_received as u32,
                }
            }
            _ => rf::get_telemetry_local(&mut self.telem_reading_idx),
        };
        let is_binding = !is_crsf && rf::is_binding();

        // 8. Poll USB subsystem (Joystick HID @ 100Hz, Serial CLI / Telemetry)
        usb::poll(now, &rf_chs, &state.switches, &telem, state.battery_mv);

        // 9. Variometer Acoustic Evaluation & Background Audio Generation
        if !menu_active || vario_preview {
            let vario_cfg = storage.vario_config();
            let sw_active = match vario_cfg.switch {
                0 => false,
                1 => true,
                sw if sw <= (ActiveBoard::MAX_MIX_SWITCH + 1) => {
                    mixer::eval_arm_switch(sw - 1, &state.switches)
                }
                _ => false,
            };
            let thr_ch = rf_chs[2]; // CH3 Throttle in µs (1000..2000)
            let tone = self.vario.update(
                now,
                telem.alt_m,
                telem.alt_dm,
                telem.vspeed_cms,
                telem.has_vspeed,
                telem.connected,
                sw_active || vario_preview,
                thr_ch,
                &vario_cfg,
            );
            buzzer.set_vario(tone.freq_hz, tone.pulse_ms, tone.pause_ms);
        } else {
            buzzer.set_vario(0, 0, 0);
        }

        FlightSnapshot {
            state,
            rf_chs,
            telem,
            is_binding,
            is_armed,
            just_armed,
            just_disarmed,
        }
    }
}

/// Background idle and UI context: tracks inactivity, backlight timers, user input, and screen rendering.
pub struct BackgroundIdleManager {
    pub prev_stick_samples: [u16; 6],
    pub prev_pots: [i16; MAX_POTS],
    pub pot_center_armed: [bool; MAX_POTS],
    pub prev_switches: input::Switches,
    pub prev_bind_key: bool,
    pub bind_hold_ms: u32,
    pub bind_was_held: bool,
    pub ok_hold_ms: u16,
    pub cancel_hold_ms: u16,
    pub timers: crate::timer::TimerBank,
    #[cfg(any(feature = "soaring", test))]
    pub f5j: crate::soaring::f5j::F5jEngine,
    pub timer_remaining_secs: u16,
    pub timer_elapsed_secs: u16,
    pub timer_ms_acc: u16,
    pub timer_latched: bool,
    pub timer_reset_toast_ms: u16,
    pub timer_reset_cooldown_ms: u16,
    pub cancel_waiting_release: bool,
    pub prev_timer_model: u8,
    pub prev_up_key: bool,
    pub prev_down_key: bool,
    pub bl_timer_ms: u32,
    pub inactivity_timer_ms: u32,
    pub inactivity_beep_timer: u32,
    pub last_display_ms: u32,
    pub menu_was_active: bool,
    pub rx_id_dirty: bool,
    pub trim_dirty: bool,
    pub trim_save_cooldown_ms: u16,
    pub prev_instant_trim_active: bool,
    pub telemetry_logger: crate::telemetry_log::TelemetryLogger,
    pub last_telem_log_ms: u32,
    #[cfg(feature = "voice-audio")]
    pub audio_callouts: crate::audio::CalloutScheduler,
    pub board_idle: <ActiveBoard as BoardProfile>::IdleState,
}

impl BackgroundIdleManager {
    pub fn new(init_state: &InputState, bind_on_boot: bool) -> Self {
        let pot_count = init_state.aux_pots.len().min(MAX_POTS);
        let mut prev_pots = [0i16; MAX_POTS];
        let mut pot_center_armed = [false; MAX_POTS];
        for i in 0..pot_count {
            if i + 4 < init_state.aux_pots.len() {
                let val = init_state.aux_pots[i + 4];
                prev_pots[i] = val;
                pot_center_armed[i] = val.abs() >= 40;
            }
        }

        Self {
            prev_stick_samples: [
                init_state.raw[0],
                init_state.raw[1],
                init_state.raw[2],
                init_state.raw[3],
                init_state.raw[6],
                init_state.raw[7],
            ],
            prev_pots,
            pot_center_armed,
            prev_switches: init_state.switches,
            prev_bind_key: bind_on_boot,
            bind_hold_ms: 0,
            bind_was_held: false,
            ok_hold_ms: 0,
            cancel_hold_ms: 0,
            timers: crate::timer::TimerBank::new(),
            #[cfg(any(feature = "soaring", test))]
            f5j: crate::soaring::f5j::F5jEngine::new(),
            timer_remaining_secs: 0,
            timer_elapsed_secs: 0,
            timer_ms_acc: 0,
            timer_latched: false,
            timer_reset_toast_ms: 0,
            timer_reset_cooldown_ms: 0,
            cancel_waiting_release: false,
            prev_timer_model: 0xFF,
            prev_up_key: false,
            prev_down_key: false,
            bl_timer_ms: 30_000,
            inactivity_timer_ms: 0,
            inactivity_beep_timer: 0,
            last_display_ms: 0,
            menu_was_active: false,
            rx_id_dirty: false,
            trim_dirty: false,
            trim_save_cooldown_ms: 0,
            prev_instant_trim_active: false,
            telemetry_logger: crate::telemetry_log::TelemetryLogger::new(),
            last_telem_log_ms: 0,
            #[cfg(feature = "voice-audio")]
            audio_callouts: crate::audio::CalloutScheduler::new(),
            board_idle: ActiveBoard::create_idle_state(),
        }
    }

    /// Background idle execution tick: manages timers, power save, bind key gestures, Flash saves, and display frames.
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        now: u32,
        dt_ms: u16,
        keys: u16,
        flight: &FlightSnapshot,
        pipeline: &mut FlightPipeline,
        storage: &mut RadioStorage,
        trims: &mut TrimController,
        lcd: &mut St7567,
        buzzer: &mut Buzzer,
        dashboard: &mut DashboardController,
        menu_controller: &mut MenuController,
        calib_wizard: &mut CalibWizard,
        rf_ok: bool,
    ) {
        let menu_active = menu_controller.is_active() || calib_wizard.is_active();

        let (ok_held, timer_reset_clicked, instant_trim_clicked) =
            self.board_idle
                .update(now, dt_ms, keys, menu_active, storage, buzzer);

        // 1. Resynchronize arm tracking when exiting settings menu
        if self.menu_was_active && !menu_active {
            let active = storage.active_model();
            pipeline.prev_armed = mixer::eval_arm_switch(active.arm_switch, &flight.state.switches);
        }
        self.menu_was_active = menu_active;

        // Pot Center Crossing Haptic/Audio Feedback (VRA..VRG)
        let max_pot = if storage.radio.ext_adc != 0 {
            ActiveBoard::EXT_ADC_COUNT + 2
        } else {
            2
        }
        .min(MAX_POTS);

        for idx in 0..max_pot {
            let curr = flight.state.aux_pots[idx + 4];
            let mode = storage.radio.get_adc_mode(idx + 4);

            if ActiveBoard::HAS_JOG_WHEELS {
                // FS-i6S: Only continuous pots beep on center, detented pots and spring wheels do not.
                // Uses hysteresis deadband: arms when outside ±40, beeps once upon entering ±20.
                if mode == AdcInputMode::Pot {
                    if curr.abs() >= 40 {
                        self.pot_center_armed[idx] = true;
                    } else if self.pot_center_armed[idx] && curr.abs() <= 20 {
                        buzzer.pot_center_click();
                        self.pot_center_armed[idx] = false;
                    }
                } else {
                    self.pot_center_armed[idx] = false;
                }
            } else {
                // Standard FS-i6X: Detect transitions across deadband ±25 from outside (|prev| >= 25 && |curr| < 25)
                let prev = &mut self.prev_pots[idx];
                if prev.abs() >= 25 && curr.abs() < 25 {
                    buzzer.pot_center_click();
                }
            }
            self.prev_pots[idx] = curr;
        }

        // 2. Physical activity & inactivity tracking
        let stick_moved = (flight.state.raw[0] as i32 - self.prev_stick_samples[0] as i32).abs()
            > 30
            || (flight.state.raw[1] as i32 - self.prev_stick_samples[1] as i32).abs() > 30
            || (flight.state.raw[2] as i32 - self.prev_stick_samples[2] as i32).abs() > 30
            || (flight.state.raw[3] as i32 - self.prev_stick_samples[3] as i32).abs() > 30
            || (flight.state.raw[6] as i32 - self.prev_stick_samples[4] as i32).abs() > 40
            || (flight.state.raw[7] as i32 - self.prev_stick_samples[5] as i32).abs() > 40;
        self.prev_stick_samples[0] = flight.state.raw[0];
        self.prev_stick_samples[1] = flight.state.raw[1];
        self.prev_stick_samples[2] = flight.state.raw[2];
        self.prev_stick_samples[3] = flight.state.raw[3];
        self.prev_stick_samples[4] = flight.state.raw[6];
        self.prev_stick_samples[5] = flight.state.raw[7];

        let sw_changed = flight.state.switches != self.prev_switches;
        if sw_changed && storage.radio.switch_beep != 0 && !menu_active {
            buzzer.play_tone(2200, 20);
        }
        self.prev_switches = flight.state.switches;

        let user_active = keys != 0 || stick_moved || sw_changed;

        // 3. Backlight auto-dim timeout tracking
        if user_active {
            let timeout_ms: u32 = match storage.radio.backlight_timeout {
                1 => 15_000,
                2 => 30_000,
                3 => 60_000,
                _ => 0,
            };
            self.bl_timer_ms = timeout_ms;
            lcd.set_backlight_level(storage.radio.backlight_brightness * 10);
        } else if storage.radio.backlight_timeout != 0 && dt_ms > 0 {
            if self.bl_timer_ms > dt_ms as u32 {
                self.bl_timer_ms -= dt_ms as u32;
            } else {
                self.bl_timer_ms = 0;
                lcd.set_backlight_level(0);
            }
        }

        // 4. Radio Inactivity Alarm (10 minutes without physical control activity)
        if user_active {
            self.inactivity_timer_ms = 0;
            self.inactivity_beep_timer = 0;
        } else if dt_ms > 0 {
            self.inactivity_timer_ms = self.inactivity_timer_ms.saturating_add(dt_ms as u32);
            if self.inactivity_timer_ms >= 600_000 {
                if self.inactivity_beep_timer >= 30_000 {
                    self.inactivity_beep_timer = 0;
                    buzzer.warn_inactivity();
                } else {
                    self.inactivity_beep_timer += dt_ms as u32;
                }
            }
        }

        // 5. Keys handling (Bind, Cancel, Up, Down)
        let bind_key_raw = (keys & (1 << 12)) != 0;
        let bind_pressed = bind_key_raw && !self.prev_bind_key;
        self.prev_bind_key = bind_key_raw;

        let up_key_raw = (keys & (1 << 9)) != 0;
        let up_pressed = up_key_raw && !self.prev_up_key;
        self.prev_up_key = up_key_raw;

        let down_key_raw = (keys & (1 << 8)) != 0;
        let down_pressed = down_key_raw && !self.prev_down_key;
        self.prev_down_key = down_key_raw;

        let cancel_key = (keys & (1 << 11)) != 0;

        if flight.is_binding {
            if cancel_key || bind_pressed {
                rf::set_bind_mode(false);
                buzzer.click();
            }
        } else if self.board_idle.handle_board_keys(
            dt_ms,
            keys,
            crate::boot::scan_edgetx_keys(),
            menu_active,
            dashboard,
            menu_controller,
            buzzer,
        ) || menu_active
        {
            self.bind_hold_ms = 0;
            self.bind_was_held = false;
        } else {
            // Dashboard page cycling via UP/DOWN keys
            if up_pressed {
                dashboard.prev_page(buzzer);
            } else if down_pressed {
                dashboard.next_page(buzzer);
            }

            if bind_key_raw {
                self.bind_hold_ms = self.bind_hold_ms.saturating_add(dt_ms as u32);
                if self.bind_hold_ms >= 1000 && !self.bind_was_held {
                    rf::set_bind_mode(true);
                    buzzer.play_tone(2400, 150);
                    self.bind_was_held = true;
                }
            } else {
                if !self.bind_was_held && self.bind_hold_ms >= 40 {
                    dashboard.next_page(buzzer);
                }
                self.bind_hold_ms = 0;
                self.bind_was_held = false;
            }
        }

        // 6. Persist newly bound RX ID safely to Flash outside ISR (inhibit while armed)
        if let Some(new_rx_id) = rf::take_pending_rx_save() {
            if new_rx_id != 0
                && new_rx_id != 0xFFFF_FFFF
                && storage.active_model().rx_id != new_rx_id
            {
                storage.active_model_mut().rx_id = new_rx_id;
                self.rx_id_dirty = true;
            }
        }
        if self.rx_id_dirty && !pipeline.prev_armed && storage::save_active_model(storage) {
            self.rx_id_dirty = false;
            buzzer.play_tone_pattern(2400, 70, 50, 2);
        }

        // 7. Instant Trim Evaluation: Check if any auxiliary channel configured as InstantTrim went active
        let mut instant_trim_active = false;
        let max_adc = if storage.radio.ext_adc != 0 {
            6 + ActiveBoard::EXT_ADC_COUNT
        } else {
            6
        };
        for ch in 0..max_adc {
            let mode = AdcInputMode::resolve(ch, storage.radio.adc_modes[ch]);
            if matches!(mode, AdcInputMode::InstantTrim) {
                // Active when switch is toggled down (raw reading > 2048)
                let raw_val = flight.state.raw[input::aux_index_to_raw_adc(ch)];
                if raw_val >= 2048 {
                    instant_trim_active = true;
                    break;
                }
            }
        }

        let instant_trim_triggered = instant_trim_active && !self.prev_instant_trim_active;
        self.prev_instant_trim_active = instant_trim_active;

        if instant_trim_clicked || (!menu_active && instant_trim_triggered) {
            let stick_mode = StickMode::from_u8(storage.radio.stick_mode);
            let controls = flight.state.flight_controls(stick_mode);
            // Apply symmetric rounding to nearest trim step (40 counts per trim step: -1000..+1000 -> -25..+25)
            let round_step = |val: i16| -> i8 {
                if val >= 0 {
                    ((val + 20) / 40) as i8
                } else {
                    ((val - 20) / 40) as i8
                }
            };
            let delta_roll = round_step(controls.aileron);
            let delta_pitch = round_step(controls.elevator);
            let delta_yaw = round_step(controls.rudder);

            if delta_roll != 0 || delta_pitch != 0 || delta_yaw != 0 {
                // Sticks deflected: adjust trims toward the stick deflection
                trims.values.roll = (trims.values.roll + delta_roll).clamp(-25, 25);
                trims.values.pitch = (trims.values.pitch + delta_pitch).clamp(-25, 25);
                trims.values.yaw = (trims.values.yaw + delta_yaw).clamp(-25, 25);

                // Set active trim indicator so the dashboard footer immediately shows the updated trim value
                if delta_roll != 0 {
                    trims.last_active = ActiveTrim::Roll;
                } else if delta_pitch != 0 {
                    trims.last_active = ActiveTrim::Pitch;
                } else {
                    trims.last_active = ActiveTrim::Yaw;
                }
                trims.active_timer_ms = 1500;

                self.trim_dirty = true;
                self.trim_save_cooldown_ms = 1000;
                buzzer.play_tone_pattern(2400, 60, 40, 2);
            } else if trims.values.roll != 0 || trims.values.pitch != 0 || trims.values.yaw != 0 {
                // Sticks centered and trims were non-zero: reset all primary flight trims back to neutral
                trims.values.roll = 0;
                trims.values.pitch = 0;
                trims.values.yaw = 0;

                trims.last_active = ActiveTrim::Roll;
                trims.active_timer_ms = 1500;

                self.trim_dirty = true;
                self.trim_save_cooldown_ms = 1000;
                buzzer.trim_center();
            }
        }

        // 8. Debounced auto-save of active model trims to Flash (inhibit while armed)
        let active_trims = [
            trims.values.roll,
            trims.values.pitch,
            trims.values.throttle,
            trims.values.yaw,
        ];
        if storage.active_model().trims != active_trims {
            storage.active_model_mut().trims = active_trims;
            self.trim_dirty = true;
            self.trim_save_cooldown_ms = 2000;
        }

        if self.trim_save_cooldown_ms > 0 {
            self.trim_save_cooldown_ms = self.trim_save_cooldown_ms.saturating_sub(dt_ms);
        }

        if self.trim_dirty
            && self.trim_save_cooldown_ms == 0
            && !pipeline.prev_armed
            && storage::save_active_model(storage)
        {
            self.trim_dirty = false;
        }

        // 8b. Long-press OK (1.2s) from flight dashboard opens Settings Menu;
        //     Short-press OK on F5J dashboard when landed cycles landing bonus points
        if !menu_active {
            if ok_held {
                self.ok_hold_ms = self.ok_hold_ms.saturating_add(dt_ms);
                if self.ok_hold_ms >= 1200 {
                    menu_controller.open(buzzer);
                    self.ok_hold_ms = 0;
                    self.board_idle.clear_menu_open_state();
                }
            } else {
                #[cfg(any(feature = "soaring", test))]
                if self.ok_hold_ms > 0
                    && self.ok_hold_ms < 500
                    && dashboard.page == 5
                    && self.f5j.landed
                {
                    self.f5j.cycle_landing_bonus();
                    buzzer.click();
                }
                self.ok_hold_ms = 0;
                self.board_idle.clear_menu_open_state();
            }
        }

        // 8c. Flight Timer Engine (4 Independent Timers)
        let active_idx = storage.radio.active_model;
        let model = storage.active_model();
        let timer_configs = model.all_timer_configs();

        if self.prev_timer_model != active_idx {
            self.prev_timer_model = active_idx;
            self.timers.reset_all(&timer_configs);
            #[cfg(any(feature = "soaring", test))]
            self.f5j.reset_to_ready(flight.telem.alt_dm);
        }

        // Cooldown timer, toast duration, and key release guard for timer reset
        if self.timer_reset_toast_ms > 0 {
            self.timer_reset_toast_ms = self.timer_reset_toast_ms.saturating_sub(dt_ms);
        }
        if self.timer_reset_cooldown_ms > 0 {
            self.timer_reset_cooldown_ms = self.timer_reset_cooldown_ms.saturating_sub(dt_ms);
        }
        if !cancel_key {
            self.cancel_waiting_release = false;
        }

        // Check for manual reset via assigned button or [CANCEL] held for >= 1.0s on flight dashboard
        if timer_reset_clicked && self.timer_reset_cooldown_ms == 0 {
            self.timers.reset_all(&timer_configs);
            #[cfg(any(feature = "soaring", test))]
            self.f5j.reset_to_ready(flight.telem.alt_dm);
            self.cancel_hold_ms = 0;
            self.cancel_waiting_release = true;
            self.timer_reset_toast_ms = 800;
            self.timer_reset_cooldown_ms = 1500;
            buzzer.play_tone(2400, 100);
        } else if !menu_active
            && cancel_key
            && !self.cancel_waiting_release
            && self.timer_reset_cooldown_ms == 0
        {
            self.cancel_hold_ms = self.cancel_hold_ms.saturating_add(dt_ms);
            if self.cancel_hold_ms >= 1000 {
                self.timers.reset_all(&timer_configs);
                #[cfg(any(feature = "soaring", test))]
                self.f5j.reset_to_ready(flight.telem.alt_dm);
                self.cancel_hold_ms = 0;
                self.cancel_waiting_release = true;
                self.timer_reset_toast_ms = 800;
                self.timer_reset_cooldown_ms = 1500;
                buzzer.play_tone(2400, 100);
            }
        } else {
            self.cancel_hold_ms = 0;
            if menu_active && cancel_key {
                self.cancel_waiting_release = true;
            }
        }

        // Evaluate timer triggers and update all 4 timers
        let is_armed_or_unassigned = if model.arm_switch > 0
            && (model.arm_switch <= ActiveBoard::MAX_MIX_SWITCH
                || (29..=44).contains(&model.arm_switch))
        {
            flight.is_armed
        } else {
            true
        };
        let stick_mode = StickMode::from_u8(storage.radio.stick_mode);
        let controls = flight.state.flight_controls(stick_mode);

        self.timers.update(
            dt_ms,
            &timer_configs,
            controls.throttle,
            &flight.state.switches,
            is_armed_or_unassigned,
            flight.just_armed,
            flight.just_disarmed,
            buzzer,
        );

        #[cfg(any(feature = "soaring", test))]
        let was_landed = self.f5j.landed;
        #[cfg(any(feature = "soaring", test))]
        self.f5j
            .update(now, &flight.rf_chs, &flight.telem, &model.soaring, buzzer);

        // 8d. Periodic 512-byte sector CSV telemetry logging to SD card
        if flight.just_armed {
            self.telemetry_logger.reset();
            self.last_telem_log_ms = now;
        } else if flight.just_disarmed {
            let logger = &mut self.telemetry_logger;
            let _ = crate::storage::with_storage_bus(|bus| logger.flush(bus));
        } else {
            #[cfg(any(feature = "soaring", test))]
            if !was_landed && self.f5j.landed && model.soaring.auto_log_enabled() {
                let logger = &mut self.telemetry_logger;
                let _ = crate::storage::with_storage_bus(|bus| logger.flush(bus));
            }
            if is_armed_or_unassigned
                && flight.telem.connected
                && model.soaring.auto_log_enabled()
                && now.wrapping_sub(self.last_telem_log_ms) >= 500
            {
                self.last_telem_log_ms = now;
                let frame = crate::telemetry_log::FlightLogFrame {
                    time_ms: now,
                    rqly: flight.telem.rssi,
                    rssi_dbm: 0,
                    snr_db: 0,
                    tx_pwr: model.rf_power,
                    rx_vbat_mv: flight.telem.rx_voltage_mv,
                    curr_ma: 0,
                    mah: 0,
                    lat: 0,
                    lon: 0,
                    alt_m: flight.telem.alt_m,
                    spd_kt: 0,
                    sats: 0,
                    vspd_cms: flight.telem.vspeed_cms,
                    ch1: flight.rf_chs[0],
                    ch2: flight.rf_chs[1],
                    ch3: flight.rf_chs[2],
                    ch4: flight.rf_chs[3],
                };
                let logger = &mut self.telemetry_logger;
                let _ = crate::storage::with_storage_bus(|bus| logger.log_frame(&frame, bus));
            }
        }

        // Keep legacy Timer 1 scalar fields in sync for telemetry/logging
        self.timer_remaining_secs = self.timers.timers[0].remaining_secs;
        self.timer_elapsed_secs = self.timers.timers[0].elapsed_secs;
        self.timer_ms_acc = self.timers.timers[0].ms_acc;
        self.timer_latched = self.timers.timers[0].latched_running;

        // 9. Display Frame Rendering (~30 Hz)
        let run_display = now.wrapping_sub(self.last_display_ms) >= 33;
        if !run_display {
            return;
        }
        self.last_display_ms = now;

        let frame_keys = self.board_idle.take_frame_keys(keys, menu_active);

        self.render_display_frame(
            now,
            lcd,
            frame_keys,
            dt_ms,
            flight,
            storage,
            trims,
            buzzer,
            dashboard,
            menu_controller,
            calib_wizard,
            rf_ok,
        );
    }

    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn render_display_frame(
        &mut self,
        now: u32,
        lcd: &mut St7567,
        keys: u16,
        dt_ms: u16,
        flight: &FlightSnapshot,
        storage: &mut RadioStorage,
        trims: &mut TrimController,
        buzzer: &mut Buzzer,
        dashboard: &mut DashboardController,
        menu_controller: &mut MenuController,
        calib_wizard: &mut CalibWizard,
        rf_ok: bool,
    ) {
        if menu_controller.is_active() {
            menu_controller.update(
                lcd,
                keys,
                &flight.state.switches,
                &flight.state.pots,
                storage,
                trims,
                &flight.state.raw,
                &flight.rf_chs,
                buzzer,
            );

            if menu_controller.request_calibration {
                calib_wizard.start(buzzer);
                menu_controller.request_calibration = false;
            }

            if menu_controller.request_bind {
                rf::set_bind_mode(true);
                menu_controller.request_bind = false;
                buzzer.play_tone(2400, 150);
            }
        } else if calib_wizard.is_active() {
            calib_wizard.update(lcd, storage, &flight.state.raw, keys, dt_ms.max(20), buzzer);
        } else {
            let model = storage.active_model();
            let mut timer_buf = [0u8; 8];
            let timer_display = if model.timer_source != 0 {
                let (disp_secs, expired) = if model.timer_secs > 0 {
                    if self.timer_remaining_secs > 0 {
                        (self.timer_remaining_secs, false)
                    } else {
                        (self.timer_elapsed_secs, true)
                    }
                } else {
                    (self.timer_elapsed_secs, false)
                };
                let formatted = format::format_timer(disp_secs, expired, &mut timer_buf);
                Some((formatted, expired))
            } else {
                None
            };

            let (timer_str, timer_expired) = match timer_display {
                Some((s, exp)) => (Some(s), exp),
                None => (None, false),
            };

            dashboard.render(
                lcd,
                &flight.state,
                storage,
                trims,
                &flight.rf_chs,
                rf_ok,
                flight.is_binding,
                &flight.telem,
                #[cfg(any(feature = "soaring", test))]
                &self.f5j,
                timer_str,
                timer_expired,
                buzzer,
                now,
            );

            if self.timer_reset_toast_ms > 0 {
                draw_timer_reset_modal(lcd, 100, true);
            } else if self.cancel_hold_ms > 80 {
                let pct = ((self.cancel_hold_ms as u32 * 100) / 1000).min(100) as u8;
                draw_timer_reset_modal(lcd, pct, false);
            }
        }

        self.board_idle.render_overlay(lcd);
        lcd.flush();
    }
}

/// Pre-flight safety check: verifies throttle at idle and switches in safe UP positions.
#[inline(never)]
pub fn run_preflight_check(
    storage: &RadioStorage,
    buzzer: &mut Buzzer,
    lcd: &mut St7567,
    calib_wizard: &CalibWizard,
    was_watchdog_reset: bool,
) {
    if calib_wizard.is_active() || was_watchdog_reset {
        return;
    }

    let mut preflight_beep_timer: u32 = 0;
    let mut preflight_last_render: u32 = 0;
    let mut warned = false;

    loop {
        watchdog::feed();

        let now = time::millis();
        let state = input::read();
        let keys = crate::boot::scan_keys();

        let raw_adc = [state.raw[0], state.raw[1], state.raw[2], state.raw[3]];
        let model_type = storage.active_model().model_type();
        let stick_mode = StickMode::from_u8(storage.radio.stick_mode);
        let cal = [
            safety::ChannelCalibration::from_channel_calib(&storage.radio.sticks[0]),
            safety::ChannelCalibration::from_channel_calib(&storage.radio.sticks[1]),
            safety::ChannelCalibration::from_channel_calib(&storage.radio.sticks[2]),
            safety::ChannelCalibration::from_channel_calib(&storage.radio.sticks[3]),
        ];
        let thr_hold = state.switches.sa == SwitchPos::Down;

        let thr_result = safety::check_preflight_throttle(
            &raw_adc,
            Some(&cal),
            stick_mode,
            model_type,
            thr_hold,
        );
        let thr_unsafe = thr_result.is_err();

        // For helicopters, SA Down is the required Throttle Hold position.
        // For all other models, SA Up is the safe position.
        let is_heli = model_type == safety::ModelType::Heli;
        let sa_unsafe = if is_heli {
            state.switches.sa != SwitchPos::Down
        } else {
            state.switches.sa != SwitchPos::Up
        };
        let sb_unsafe = state.switches.sb != SwitchPos::Up;
        let sc_unsafe = state.switches.sc != SwitchPos::Up;
        let sd_unsafe = state.switches.sd != SwitchPos::Up;
        let sw_unsafe = sa_unsafe || sb_unsafe || sc_unsafe || sd_unsafe;

        let cancel_pressed = (keys & (1 << 11)) != 0;

        if (!thr_unsafe && !sw_unsafe) || cancel_pressed {
            if warned {
                buzzer.play_tone(2200, 40);
            }
            break;
        }
        warned = true;

        // Lock RF transmission to safe idle/failsafe during warning
        rf::set_channels(&SAFE_IDLE_CHANNELS);
        usb::poll(
            now,
            &SAFE_IDLE_CHANNELS,
            &state.switches,
            &rf::get_telemetry(),
            state.battery_mv,
        );

        if now.wrapping_sub(preflight_beep_timer) >= 800 {
            preflight_beep_timer = now;
            buzzer.warn_preflight();
        }

        if now.wrapping_sub(preflight_last_render) >= 33 {
            let dt = (now.wrapping_sub(preflight_last_render)).min(100) as u16;
            preflight_last_render = now;
            buzzer.tick(dt);

            lcd.clear_buffer();
            lcd.draw_str_6x10(16, 2, "SAFETY WARNING!", false);
            lcd.draw_hline(0, 11, 128, true);

            if let Err(err) = thr_result {
                let msg = match err {
                    safety::PreflightError::ThrottleHigh { .. } => "THROTTLE NOT AT IDLE!",
                    safety::PreflightError::ThrottleNotCentered { .. } => "CENTER THROTTLE STICK!",
                    safety::PreflightError::ThrottleHoldDisengaged => "THROTTLE HOLD OFF!",
                };
                lcd.draw_str_6x10(2, 16, msg, false);
            }

            if sw_unsafe {
                lcd.draw_str_6x10(2, 27, "SWITCH WARNING:", false);
                let mut sw_warn = [b' '; 20];
                let mut col = 0;
                for &(unsafe_flag, label) in &[
                    (sa_unsafe, b"[SA]"),
                    (sb_unsafe, b"[SB]"),
                    (sc_unsafe, b"[SC]"),
                    (sd_unsafe, b"[SD]"),
                ] {
                    if unsafe_flag && col + 4 <= sw_warn.len() {
                        sw_warn[col..col + 4].copy_from_slice(label);
                        col += 4;
                        if col < sw_warn.len() {
                            sw_warn[col] = b' ';
                            col += 1;
                        }
                    }
                }
                let sw_str = format::ascii_as_str(&sw_warn[..col.saturating_sub(1).min(16)]);
                lcd.draw_str_6x10(2, 37, sw_str, false);
            }

            lcd.draw_hline(0, 55, 128, true);
            let footer = match thr_result {
                Err(safety::PreflightError::ThrottleNotCentered { .. }) => {
                    "Center Thr/Safe SW [ESC]Skip"
                }
                Err(safety::PreflightError::ThrottleHoldDisengaged) => {
                    "Hold Thr/Safe SW   [ESC]Skip"
                }
                _ => "Lower Thr/Safe SW  [ESC]Skip",
            };
            lcd.draw_str_4x6(2, 57, footer, false);
            lcd.flush();
        }
    }
}

/// Draw centered modal overlay for timer reset hold progress or completion confirmation.
pub fn draw_timer_reset_modal(lcd: &mut St7567, progress_pct: u8, completed: bool) {
    lcd.fill_rect(18, 19, 92, 26, false);
    lcd.draw_rect(18, 19, 92, 26, true);

    if completed {
        lcd.draw_str_6x10(28, 28, "TIMER RESET!", false);
    } else {
        lcd.draw_str_6x10(31, 22, "RESET TIMER", false);

        // Progress bar container: 74 x 6, positioned at (27, 33)
        lcd.draw_rect(27, 33, 74, 6, true);

        // Progress fill: up to 70px wide
        let fill_w = ((progress_pct as u32 * 70) / 100).min(70);
        if fill_w > 0 {
            lcd.fill_rect(29, 35, fill_w, 2, true);
        }
    }
}

/// Unified entry point for Ferrox flight controller runtime execution.
///
/// Executes initialization stages, bootloader checks, peripheral discovery,
/// safety interlocks, and the dual-rate flight loop.
pub fn run(ctx: crate::hal::PlatformContext, on_stable_runtime: Option<fn()>) -> ! {
    // Register explicit platform context drivers
    ctx.register();

    // Relocate VTOR and sanitize bootloader interrupt/SysTick state before any init
    crate::chip::pre_init();

    // 0. Early board initialization (e.g. electronic power latch on FS-i6S)
    ActiveBoard::pre_init();

    // Standalone feed immediately refreshes any watchdog running across soft reboot
    watchdog::feed();

    // Check if reboot was triggered by hardware watchdog (IWDG or WWDG) and clear RCC flags.
    // Under DO-178C flight safety principles, a warm reboot caused by watchdog reset must bypass
    // the throttle-at-idle and switch interlocks to restore flight control in <10ms.
    let was_watchdog_reset = crate::chip::check_and_clear_reset_flags();

    // 1. Fast DFU Bootloader Check
    crate::boot::check_sram_dfu_entry();

    if !was_watchdog_reset {
        crate::boot::check_dfu_entry();
    } else {
        crate::boot::init_keys();
    }

    // 2. Initialize System Clock to 48 MHz using external 8 MHz crystal (HSE) + PLL
    // and wait for 40 kHz LSI oscillator to stabilize
    crate::chip::init_system_clock();

    // Initialize SysTick 1.000 ms hardware monotonic timekeeper
    time::init();

    // Initialize board-specific touch controller (if present)
    ActiveBoard::init_touch();

    // 3. Load persistent radio storage and 20-model configuration
    let storage = storage::get_storage();
    storage::load_storage_into(storage);

    // 4. Initialize Buzzer & Tone Style
    let mut buzzer = Buzzer::new();
    buzzer.init();
    buzzer.enabled = storage.radio.audio_enabled != 0;
    buzzer.tone_style = crate::buzzer::ToneStyle::from_u8(storage.radio.tone_style);
    #[cfg(feature = "voice-audio")]
    crate::audio::init();

    // 5. Initialize ST7567 128×64 LCD (clears framebuffer and flushes before turning on backlight)
    let mut lcd = St7567::new();
    lcd.set_backlight_level(storage.radio.backlight_brightness * 10);
    lcd.set_contrast(storage.radio.lcd_contrast);

    // Render Ferrox-RC Splash Screen and trigger welcome chime simultaneously (bypassed on warm watchdog recovery)
    let splash_start_ms = time::millis();
    let mut buzzer_last_ms = splash_start_ms;
    if was_watchdog_reset {
        // Fast alarm warning chirp indicating watchdog recovery occurred
        buzzer.play_tone_pattern(2600, 60, 40, 3);
    } else {
        crate::ui::splash::draw_splash(&mut lcd);
        lcd.flush();
    }

    // 6. Start deterministic 2.0s Hardware Watchdog (IWDG) via PAC
    watchdog::start();

    // 7. Initialize ADC1 + DMA1 autonomous continuous scanner
    crate::adc::init(storage.radio.ext_adc != 0);
    watchdog::feed();
    {
        let now = time::millis();
        let dt = now.wrapping_sub(buzzer_last_ms) as u16;
        if dt > 0 {
            buzzer.tick(dt);
            buzzer_last_ms = now;
        }
    }

    // 8. Initialize input calibration and capture resting stick centers
    input::init();
    input::apply_calibration(&storage.radio);
    ActiveBoard::init_board_keys(&storage.radio);
    watchdog::feed();
    {
        let now = time::millis();
        let dt = now.wrapping_sub(buzzer_last_ms) as u16;
        if dt > 0 {
            buzzer.tick(dt);
            buzzer_last_ms = now;
        }
    }

    // 9. Initialize A7105 RF transceiver & AFHDS 2A stack
    let uid = crate::chip::read_uid();
    let w0 = u32::from_le_bytes([uid[0], uid[1], uid[2], uid[3]]);
    let w1 = u32::from_le_bytes([uid[4], uid[5], uid[6], uid[7]]);
    let w2 = u32::from_le_bytes([uid[8], uid[9], uid[10], uid[11]]);
    let tx_id = w0 ^ w1 ^ w2;

    let initial_keys = crate::boot::scan_keys();
    let bind_on_boot = (initial_keys & (1 << 12)) != 0 && !was_watchdog_reset;

    let rf_ok = rf::init(tx_id);
    if bind_on_boot {
        rf::set_bind_mode(true);
    }
    watchdog::feed();
    {
        let now = time::millis();
        let dt = now.wrapping_sub(buzzer_last_ms) as u16;
        if dt > 0 {
            buzzer.tick(dt);
            buzzer_last_ms = now;
        }
    }

    // Initialize USB peripheral (Joystick / Serial / Composite / Off)
    usb::init(storage.radio.usb_mode);

    // Initialize CRSF / ExpressLRS expansion bay peripheral
    crsf::init(storage.radio.ext_module_pwr == 0);

    // Initialize Trainer / PPM subsystem (PPM Out generator and Student PPM In capture)
    trainer::init(
        storage.radio.is_ppm_out_enabled(),
        storage.radio.is_ppm_in_enabled(),
    );

    let mut trims = TrimController::new();
    trims.throttle_enabled = storage.radio.throttle_trim != 0;

    // Load active model trims and receiver ID
    let active = storage.active_model();
    trims.values.roll = active.trims[0];
    trims.values.pitch = active.trims[1];
    trims.values.throttle = active.trims[2];
    trims.values.yaw = active.trims[3];
    rf::set_rx_id(active.rx_id);
    rf::set_rf_power(active.rf_power);
    rf::set_rx_settings(
        active.servo_rate_hz,
        active.rx_out_mode,
        active.rx_serial_proto,
    );

    let mut calib_wizard = CalibWizard::new();
    let mut menu_controller = MenuController::new();

    // Check if OK button held at power-on to launch calibration directly
    if (initial_keys & (1 << 10)) != 0 && !was_watchdog_reset {
        calib_wizard.start(&mut buzzer);
    }

    // 10. Hold splash screen for ~1200ms (1.2s) total elapsed time since LCD initialization,
    // continuously advancing buzzer playback to maintain musical cadence
    if !was_watchdog_reset && !calib_wizard.is_active() {
        let _ = buzzer_last_ms;
        buzzer.chime_welcome();
        buzzer_last_ms = time::millis();
        while time::millis().wrapping_sub(splash_start_ms) < 1200 {
            watchdog::feed();
            let now = time::millis();
            let dt = now.wrapping_sub(buzzer_last_ms) as u16;
            if dt >= 5 {
                buzzer.tick(dt);
                buzzer_last_ms = now;
            }
        }
    }

    // 11. Pre-flight Startup Safety Check: Throttle at idle and switches in safe (UP) positions
    // Bypassed on watchdog reset to immediately resume RF control in flight
    run_preflight_check(
        storage,
        &mut buzzer,
        &mut lcd,
        &calib_wizard,
        was_watchdog_reset,
    );

    // Initialize execution tiers
    let init_state = input::read();
    let mut pipeline = FlightPipeline::new(storage, &init_state);
    let mut idle_manager = BackgroundIdleManager::new(&init_state, bind_on_boot);
    let mut dashboard = DashboardController::new();
    let mut last_tick_ms: u32 = 0;

    // Main event loop: decoupled into high-rate flight pipeline and 30 Hz background idle tasks
    loop {
        // Feed watchdog at loop start to guarantee active execution
        watchdog::feed();

        // Service deferred USB reconfiguration outside deep UI call stacks
        if let Some(mode) = usb::take_pending_init() {
            usb::init(mode);
        }

        let now = time::millis();
        let dt_ms = (now.wrapping_sub(last_tick_ms)).min(100) as u16;

        let menu_active = menu_controller.is_active() || calib_wizard.is_active();
        let keys = ActiveBoard::poll_keys(storage, menu_active);

        if dt_ms > 0 {
            last_tick_ms = now;
            buzzer.tick(dt_ms);
            trainer::tick_ms(dt_ms);
            if !menu_active {
                trims.update(keys, dt_ms, &mut buzzer);
            }

            // Once the transmitter has been running steadily in the flight loop for >5 seconds,
            // invoke the board-level panic counter reset callback.
            if now > 5000 {
                if let Some(cb) = on_stable_runtime {
                    cb();
                }
            }
        }

        let is_vario_setup = menu_controller.is_vario_setup();
        // Tier 1: High-Rate Flight Pipeline Tick (multi-kHz)
        let flight_snapshot = pipeline.tick(
            now,
            storage,
            &trims,
            menu_active,
            is_vario_setup,
            &mut buzzer,
        );

        // Tier 2: Background Idle & UI Tick (30 Hz rate-governed)
        idle_manager.tick(
            now,
            dt_ms,
            keys,
            &flight_snapshot,
            &mut pipeline,
            storage,
            &mut trims,
            &mut lcd,
            &mut buzzer,
            &mut dashboard,
            &mut menu_controller,
            &mut calib_wizard,
            rf_ok,
        );

        // Tier 3: Non-blocking WAV/PCM voice playback & periodic telemetry callouts
        #[cfg(feature = "voice-audio")]
        if storage.radio.audio_enabled != 0 {
            let alt_m = (flight_snapshot.telem.last_alt_ms != 0
                && now.wrapping_sub(flight_snapshot.telem.last_alt_ms) < 5000)
                .then_some(flight_snapshot.telem.alt_m);
            idle_manager.audio_callouts.maybe_announce(
                now,
                storage.radio.voice_interval_secs(),
                flight_snapshot.telem.connected,
                alt_m,
                0,
                flight_snapshot.telem.rssi,
            );
            crate::audio::tick(storage.radio.voice_volume_pct());
        }
    }
}
