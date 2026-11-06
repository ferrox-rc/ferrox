//! Trainer Subsystem & PPM In/Out Engine.
//!
//! Orchestrates:
//! - **PPM Out**: 22.5 ms 8-channel CPPM generator for simulators and external RF modules.
//! - **PPM In**: Input capture decoder for student buddy-box and FPV head-trackers.
//! - **Safety Handover**: Instant instructor stick takeover (> 5% deflection) and signal loss fallback.

#![allow(static_mut_refs)]
#![allow(dead_code)]

pub mod handover;
pub mod ppm_in;
pub mod ppm_out;

pub use handover::{SafetyHandover, TrainerState};
pub use ppm_in::PpmDecoder;
pub use ppm_out::PpmGenerator;

use core::sync::atomic::{AtomicBool, Ordering};
use crate::hal::TrainerDriver;

static PPM_OUT_ENABLED: AtomicBool = AtomicBool::new(false);
static PPM_IN_ENABLED: AtomicBool = AtomicBool::new(false);

static mut PPM_OUT_GEN: PpmGenerator = PpmGenerator::new();
static mut PPM_IN_DEC: PpmDecoder = PpmDecoder::new();
static mut HANDOVER: SafetyHandover = SafetyHandover::new();

static mut TRAINER_DRIVER: Option<&'static mut dyn TrainerDriver> = None;

/// Register platform-specific trainer hardware driver.
pub fn register_trainer_driver(driver: &'static mut dyn TrainerDriver) {
    unsafe { TRAINER_DRIVER = Some(driver) };
}

/// Initialize Trainer Subsystem.
pub fn init(ppm_out_en: bool, ppm_in_en: bool) {
    PPM_OUT_ENABLED.store(ppm_out_en, Ordering::Relaxed);
    PPM_IN_ENABLED.store(ppm_in_en, Ordering::Relaxed);

    if let Some(ref mut driver) = unsafe { TRAINER_DRIVER.as_mut() } {
        driver.init(ppm_out_en, ppm_in_en);
    }
}

/// Enable or disable PPM output.
pub fn set_ppm_out(enabled: bool) {
    PPM_OUT_ENABLED.store(enabled, Ordering::Relaxed);
    if let Some(ref mut driver) = unsafe { TRAINER_DRIVER.as_mut() } {
        driver.set_ppm_out(enabled);
    }
}

/// Enable or disable PPM input.
pub fn set_ppm_in(enabled: bool) {
    PPM_IN_ENABLED.store(enabled, Ordering::Relaxed);
    if let Some(ref mut driver) = unsafe { TRAINER_DRIVER.as_mut() } {
        driver.set_ppm_in(enabled);
    }
}

pub fn is_ppm_out_enabled() -> bool {
    PPM_OUT_ENABLED.load(Ordering::Relaxed)
}

pub fn is_ppm_in_enabled() -> bool {
    PPM_IN_ENABLED.load(Ordering::Relaxed)
}

/// Update output channel pulse widths for PPM transmission (clamped 900..2100 µs).
pub fn update_out_channels(chs: &[u16]) {
    cortex_m::interrupt::free(|_| unsafe {
        PPM_OUT_GEN.update_channels(chs);
    });
}

/// Periodic tick (dt_ms) to manage student signal loss timeout.
pub fn tick_ms(dt_ms: u16) {
    cortex_m::interrupt::free(|_| unsafe {
        PPM_IN_DEC.tick_ms(dt_ms);
    });
}

/// Periodic 10ms tick helper.
pub fn tick_10ms() {
    tick_ms(10);
}

/// Get decoded student channel inputs if valid.
pub fn get_student_channels() -> Option<[u16; 8]> {
    cortex_m::interrupt::free(|_| unsafe { PPM_IN_DEC.get_channels() })
}

/// Get current safety handover status.
pub fn get_handover_state() -> TrainerState {
    cortex_m::interrupt::free(|_| unsafe { HANDOVER.state })
}

/// Process safety handover between instructor sticks and decoded student stream.
pub fn process_handover(
    trainer_switch_active: bool,
    instructor_chs: &[u16],
    output_chs: &mut [u16],
) {
    cortex_m::interrupt::free(|_| unsafe {
        let student_chs = PPM_IN_DEC.get_channels();
        HANDOVER.process(
            trainer_switch_active,
            student_chs.as_ref(),
            instructor_chs,
            output_chs,
        );
    });
}

/// Hardware ISR event callback for PPM compare match (PPM out pulse transition).
/// Returns (duration_us, pin_high).
pub fn on_ppm_out_event() -> (u16, bool) {
    if PPM_OUT_ENABLED.load(Ordering::Relaxed) {
        unsafe { PPM_OUT_GEN.next_interval() }
    } else {
        (0, true)
    }
}

/// Hardware ISR event callback for PPM input capture (edge-to-edge delta in µs).
pub fn on_ppm_in_pulse(delta_us: u16) {
    if PPM_IN_ENABLED.load(Ordering::Relaxed) {
        unsafe {
            PPM_IN_DEC.process_pulse(delta_us);
        }
    }
}
