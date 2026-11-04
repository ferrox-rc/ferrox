#![no_std]
#![no_main]

use stm32f0xx_hal as _;

// Panic loop guard placed in .uninit section so RAM value survives warm resets
#[link_section = ".uninit"]
static mut PANIC_COUNT: u32 = 0;

const PANIC_MAGIC: u32 = 0x5041_4E00; // 'PAN\0'
const PANIC_MAX_CONSECUTIVE: u32 = 3;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Disable interrupts to prevent ISR execution during fault handling
    cortex_m::interrupt::disable();

    unsafe {
        let val = core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT));
        let count = if (val & 0xFFFF_FF00) == PANIC_MAGIC {
            val & 0xFF
        } else {
            0
        };

        if count < PANIC_MAX_CONSECUTIVE {
            core::ptr::write_volatile(
                core::ptr::addr_of_mut!(PANIC_COUNT),
                PANIC_MAGIC | (count + 1),
            );
            // Trigger rapid Cortex-M0 warm reset (<20ms recovery vs 2.0s watchdog)
            cortex_m::peripheral::SCB::sys_reset();
        }

        // Exceeded consecutive panic threshold: deterministic crash loop detected.
        // Halt and wait for hardware watchdog or power cycle rather than thrashing in an infinite loop.
        loop {
            cortex_m::asm::wfi();
        }
    }
}

use cortex_m_rt::entry;

use ferrox_core::{
    adc, boot, buzzer, calib, chip, crsf, display, hw, input, rf, runtime, storage,
    time, trainer, trim, ui, usb, watchdog,
};
pub use runtime::{run_preflight_check, BackgroundIdleManager, FlightPipeline, FlightSnapshot};
pub use ui::menu;

use display::St7567;

#[entry]
fn main() -> ! {
    // 0. Early board initialization (e.g. electronic power latch on FS-i6S)
    hw::pre_init();

    // Standalone feed immediately refreshes any watchdog running across soft reboot
    watchdog::feed();

    // Check if reboot was triggered by hardware watchdog (IWDG or WWDG) and clear RCC flags.
    // Under DO-178C flight safety principles, a warm reboot caused by watchdog reset must bypass
    // the throttle-at-idle and switch interlocks to restore flight control in <10ms.
    let was_watchdog_reset = chip::check_and_clear_reset_flags();

    // 1. MCU Profile & Fast DFU Bootloader Check
    let mcu_profile = chip::get_mcu_profile();
    boot::check_sram_dfu_entry(&mcu_profile);

    if !was_watchdog_reset {
        boot::check_dfu_entry(&mcu_profile);
    } else {
        boot::init_keys();
    }

    // 2. Initialize System Clock to 48 MHz using external 8 MHz crystal (HSE) + PLL
    // and wait for 40 kHz LSI oscillator to stabilize
    chip::init_system_clock();

    // Initialize SysTick 1.000 ms hardware monotonic timekeeper
    time::init();

    // Initialize board-specific touch controller (if present)
    hw::init_touch();

    // 3. Load persistent radio storage and 20-model configuration
    let storage = storage::get_storage();
    storage::load_storage_into(storage);

    // 4. Initialize Buzzer & Tone Style
    let mut buzzer = buzzer::Buzzer::new();
    buzzer.init();
    buzzer.enabled = storage.radio.audio_enabled != 0;
    buzzer.tone_style = buzzer::ToneStyle::from_u8(storage.radio.tone_style);

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
        ui::splash::draw_splash(&mut lcd);
        lcd.flush();
        buzzer.chime_welcome();
    }

    // 6. Start deterministic 2.0s Hardware Watchdog (IWDG) via PAC
    watchdog::start();

    // 7. Initialize ADC1 + DMA1 autonomous continuous scanner
    adc::init(storage.radio.ext_adc != 0);
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
    hw::init_board_keys(&storage.radio);
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
    let uid = chip::read_uid(&mcu_profile);
    let w0 = u32::from_le_bytes([uid[0], uid[1], uid[2], uid[3]]);
    let w1 = u32::from_le_bytes([uid[4], uid[5], uid[6], uid[7]]);
    let w2 = u32::from_le_bytes([uid[8], uid[9], uid[10], uid[11]]);
    let tx_id = w0 ^ w1 ^ w2;

    let initial_keys = boot::scan_keys();
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

    // Initialize Trainer / PPM subsystem (TIM15, PF9 In, PF10 Out)
    trainer::init(
        storage.radio.is_ppm_out_enabled(),
        storage.radio.is_ppm_in_enabled(),
    );

    let mut trims = trim::TrimController::new();
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

    let mut calib_wizard = calib::CalibWizard::new();
    let mut menu_controller = menu::MenuController::new();

    // Check if OK button held at power-on to launch calibration directly
    if (initial_keys & (1 << 10)) != 0 && !was_watchdog_reset {
        calib_wizard.start(&mut buzzer);
    }

    // 10. Hold splash screen for ~1200ms (1.2s) total elapsed time since LCD initialization,
    // continuously advancing buzzer playback to maintain musical cadence
    if !was_watchdog_reset && !calib_wizard.is_active() {
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
    let mut dashboard = ui::dashboard::DashboardController::new();
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
        let keys = hw::poll_keys(storage, menu_active);

        if dt_ms > 0 {
            last_tick_ms = now;
            buzzer.tick(dt_ms);
            trainer::tick_ms(dt_ms);
            if !menu_active {
                trims.update(keys, dt_ms, &mut buzzer);
            }

            // Once the transmitter has been running steadily in the flight loop for >5 seconds,
            // clear the consecutive panic count.
            if now > 5000 {
                unsafe {
                    if core::ptr::read_volatile(core::ptr::addr_of!(PANIC_COUNT)) != 0 {
                        core::ptr::write_volatile(core::ptr::addr_of_mut!(PANIC_COUNT), 0);
                    }
                }
            }
        }

        // Tier 1: High-Rate Flight Pipeline Tick (multi-kHz)
        let vario_preview = menu_controller.state == menu::MenuState::VarioSetup;
        let flight_snapshot = pipeline.tick(
            now,
            storage,
            &trims,
            menu_active,
            vario_preview,
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
    }
}
