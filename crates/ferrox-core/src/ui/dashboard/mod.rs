//! Live Flight Dashboard subsystem for FlySky FS-i6X.
//!
//! Provides the primary 4-page flight display:
//! - Page 0 (P1/5): Primary Gimbals, Trims, Switches, Pots
//! - Page 1 (P2/5): 18-Channel Dual-Column Live Monitor
//! - Page 2 (P3/5): 18-Channel Dual-Column Live Monitor cont.
//! - Page 3 (P4/5): Model Dashboard, Type, RX ID, Throttle Curve
//! - Page 4 (P5/5): Telemetry & RF Diagnostics (CRSF Link Diag / AFHDS 2A)

pub mod pages;
pub mod status_bar;

use crate::board::BoardProfile;
use crate::buzzer::Buzzer;
use crate::display::St7567;
use crate::input::InputState;
use crate::rf::afhds2a::TelemetryData;
use crate::storage::RadioStorage;
use crate::trim::TrimController;

pub struct DashboardController {
    pub page: usize,
    pub blink_phase: u8,
    pub vbat_alarm_timer: u32,
    pub rssi_alarm_timer: u32,
    pub min_rssi: u8,
    pub min_rx_v_mv: u16,
    pub telem_seen: bool,
    pub max_pages: usize,
}

impl Default for DashboardController {
    fn default() -> Self {
        Self::new()
    }
}

impl DashboardController {
    pub fn new() -> Self {
        Self {
            page: 0,
            blink_phase: 0,
            vbat_alarm_timer: 7000,
            rssi_alarm_timer: 0,
            min_rssi: 100,
            min_rx_v_mv: 0xFFFF,
            telem_seen: false,
            max_pages: 5,
        }
    }

    /// Advance to the next dashboard page.
    pub fn next_page(&mut self, buzzer: &mut Buzzer) {
        self.page = (self.page + 1) % self.max_pages.max(1);
        buzzer.play_tone(2200, 30);
    }

    /// Return to the previous dashboard page.
    #[allow(dead_code)]
    pub fn prev_page(&mut self, buzzer: &mut Buzzer) {
        self.page = if self.page == 0 {
            self.max_pages.saturating_sub(1)
        } else {
            self.page - 1
        };
        buzzer.play_tone(2200, 30);
    }

    /// Handle UP/DOWN key presses to cycle dashboard pages (0..3).
    #[allow(dead_code)]
    pub fn handle_keys(&mut self, newly_pressed: u16, buzzer: &mut Buzzer) {
        if newly_pressed & (1 << 9) != 0 {
            // Up
            self.prev_page(buzzer);
        } else if newly_pressed & (1 << 8) != 0 {
            // Down
            self.next_page(buzzer);
        }
    }

    /// Render the current dashboard page onto the LCD.
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        lcd: &mut St7567,
        state: &InputState,
        storage: &RadioStorage,
        trims: &TrimController,
        rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
        rf_ok: bool,
        is_binding: bool,
        telem: &TelemetryData,
        #[cfg(any(feature = "soaring", test))] f5j: &crate::soaring::f5j::F5jEngine,
        timer_str: Option<&str>,
        timer_expired: bool,
        buzzer: &mut Buzzer,
        now_ms: u32,
    ) {
        #[cfg(any(feature = "soaring", test))]
        {
            self.max_pages = if storage.active_model().soaring.is_f5j() {
                6
            } else {
                5
            };
        }
        #[cfg(not(any(feature = "soaring", test)))]
        {
            self.max_pages = 5;
        }
        if self.page >= self.max_pages {
            self.page = 0;
        }

        lcd.clear_buffer();

        status_bar::render(
            lcd,
            storage,
            state.battery_mv,
            rf_ok,
            is_binding,
            telem,
            buzzer,
            self.blink_phase,
            &mut self.vbat_alarm_timer,
            &mut self.rssi_alarm_timer,
            &mut self.min_rssi,
            &mut self.min_rx_v_mv,
            &mut self.telem_seen,
        );

        let blink_on = (self.blink_phase & 0x08) != 0;
        self.blink_phase = self.blink_phase.wrapping_add(1);

        match self.page {
            0 => pages::gimbals::render(
                lcd,
                state,
                storage,
                trims,
                is_binding,
                timer_str,
                timer_expired,
                blink_on,
            ),
            1 => pages::channels::render(lcd, rf_chs, is_binding, 0),
            2 => pages::channels::render(lcd, rf_chs, is_binding, 1),
            3 => pages::model::render(lcd, storage, telem, is_binding, now_ms),
            4 => {
                let proto =
                    crate::board::ActiveBoard::resolve_protocol(storage.active_model().rf_protocol);
                let is_crsf = proto == crate::board::RfProtocol::Crsf;
                pages::telemetry::render(
                    lcd,
                    storage,
                    is_crsf,
                    telem,
                    state.battery_mv,
                    self.min_rssi,
                    self.min_rx_v_mv,
                    self.telem_seen,
                    is_binding,
                    timer_str,
                    now_ms,
                );
            }
            #[cfg(any(feature = "soaring", test))]
            _ => pages::f5j::render(lcd, storage, telem, f5j, now_ms),
            #[cfg(not(any(feature = "soaring", test)))]
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_5page_cycling() {
        let mut dash = DashboardController::new();
        let mut buzzer = Buzzer::new();

        assert_eq!(dash.page, 0);

        // Next page advances through all 5 pages and wraps
        dash.next_page(&mut buzzer);
        assert_eq!(dash.page, 1);
        dash.next_page(&mut buzzer);
        assert_eq!(dash.page, 2);
        dash.next_page(&mut buzzer);
        assert_eq!(dash.page, 3);
        dash.next_page(&mut buzzer);
        assert_eq!(dash.page, 4);
        dash.next_page(&mut buzzer);
        assert_eq!(dash.page, 0);

        // Prev page wraps backwards from 0 to 4
        dash.prev_page(&mut buzzer);
        assert_eq!(dash.page, 4);
        dash.prev_page(&mut buzzer);
        assert_eq!(dash.page, 3);
        dash.prev_page(&mut buzzer);
        assert_eq!(dash.page, 2);
        dash.prev_page(&mut buzzer);
        assert_eq!(dash.page, 1);
        dash.prev_page(&mut buzzer);
        assert_eq!(dash.page, 0);
    }

    #[test]
    fn test_dashboard_custom_telemetry_render() {
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();
        let telem = TelemetryData::default();

        storage.active_model.telem_slots = [4, 5, 6, 11]; // Custom slots: CAPACITY, TX POWER, SNR, TIMER

        pages::telemetry::render(
            &mut lcd,
            &storage,
            false,
            &telem,
            4100,
            0,
            0,
            false,
            false,
            Some("05:32"),
            10000,
        );
    }

    #[test]
    fn test_dashboard_stale_telemetry_indicator() {
        let mut lcd = St7567::new();
        let mut storage = RadioStorage::default_factory();
        // Custom slots: RPM (15), TEMP (16), VSPD (12), ALT (13)
        storage.active_model.telem_slots = [15, 16, 12, 13];

        let telem = TelemetryData {
            connected: true,
            rpm: 4500,
            last_rpm_ms: 1000,
            temp_c_x10: 350, // 35.0 C
            last_temp_ms: 1000,
            vspeed_cms: 150,
            last_vspd_ms: 1000,
            alt_m: 120,
            last_alt_ms: 1000,
            ..Default::default()
        };

        // Render at 2000ms: only 1000ms elapsed (< 2500ms) -> Live, no stale indicator
        pages::telemetry::render(
            &mut lcd, &storage, false, &telem, 4000, 100, 4000, true, false, None, 2000,
        );

        // Render at 5000ms: 4000ms elapsed (>= 2500ms) -> Stale, displays last known values with *
        pages::telemetry::render(
            &mut lcd, &storage, false, &telem, 4000, 100, 4000, true, false, None, 5000,
        );
    }

    #[test]
    fn test_model_dashboard_stale_telemetry_indicator() {
        let mut lcd = St7567::new();
        let storage = RadioStorage::default_factory();

        let telem = TelemetryData {
            connected: true,
            rx_voltage_mv: 12600,
            last_rx_voltage_ms: 1000,
            rssi: 99,
            ..Default::default()
        };

        // Render at 2000ms: 1000ms elapsed (< 2500ms) -> Live, no stale indicator
        pages::model::render(&mut lcd, &storage, &telem, false, 2000);

        // Render at 5000ms: 4000ms elapsed (>= 2500ms) -> Stale, displays last known values with *
        pages::model::render(&mut lcd, &storage, &telem, false, 5000);

        // Also test CRSF mode (rf_protocol = 1)
        let mut storage_crsf = RadioStorage::default_factory();
        storage_crsf.active_model.rf_protocol = 1;
        pages::model::render(&mut lcd, &storage_crsf, &telem, false, 2000);
        pages::model::render(&mut lcd, &storage_crsf, &telem, false, 5000);
    }
}
