//! Direct PAC USB 2.0 Full-Speed (12 Mbps) driver for STM32F072.
//!
//! Replaces external USB crates (`usb-device`, `stm32-usbd`, `usbd-hid`, `usbd-serial`)
//! with a zero-dependency, low-latency, compact hardware driver.
//! Supports Joystick (HID), Serial (CDC-ACM), and Composite modes with deferred reboot/DFU.

#![allow(static_mut_refs, dead_code)]

pub mod descriptors;
pub mod hid;
pub mod serial;

use crate::input::Switches;
use crate::rf::afhds2a::TelemetryData;
use descriptors::{
    config_descriptor, device_descriptor, string_descriptor, CDC_PACKET_SIZE, CONTROL_PACKET_SIZE,
    LINE_CODING_DEFAULT,
};
pub use serial::ResetAction;
use serial::SerialHandler;

#[cfg(not(test))]
use stm32f0xx_hal::pac::{self, interrupt};

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UsbMode {
    Off = 0,       // USB disconnected / charging only (Default)
    Joystick = 1,  // Native USB Gamepad (RF disabled for simulator play)
    Serial = 2,    // Virtual COM Port (RF active with live telemetry streaming)
    Composite = 3, // Both Joystick and Serial active simultaneously
}

impl UsbMode {
    pub fn from_u8(val: u8) -> Self {
        match val {
            1 => Self::Joystick,
            2 => Self::Serial,
            3 => Self::Composite,
            _ => Self::Off,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Off => "OFF",
            Self::Joystick => "JOYSTICK",
            Self::Serial => "SERIAL",
            Self::Composite => "COMPOSITE",
        }
    }
}

// PMA byte offsets in 1KB USB dedicated SRAM (0x4000_6000)
const PMA_BASE: usize = 0x4000_6000;
const BTABLE_OFF: u16 = 0;
const EP0_TX_OFF: u16 = 32;
const EP0_RX_OFF: u16 = 96;
const EP1_TX_OFF: u16 = 160;
const EP2_TX_OFF: u16 = 192;
const EP2_RX_OFF: u16 = 256;
const EP3_TX_OFF: u16 = 320;
const RX_COUNT_64: u16 = 0x8400; // BLSIZE=1, NUM_BLOCK=1 -> (1+1)*32 = 64 bytes

// Endpoint register bit masks
const EP_KEEP_MASK: u16 = 0x070F; // EP_TYPE | EP_KIND | EP_ADDR
const EP_CTR_MASK: u16 = 0x8080;  // Preserves CTR_RX and CTR_TX when written as 1

const STAT_DISABLED: u16 = 0;
const STAT_STALL: u16 = 1;
const STAT_NAK: u16 = 2;
const STAT_VALID: u16 = 3;

const EP_TYPE_BULK: u16 = 0x0000;
const EP_TYPE_CONTROL: u16 = 0x0200;
const EP_TYPE_INTERRUPT: u16 = 0x0600;

#[derive(Copy, Clone, PartialEq, Eq)]
enum ControlStage {
    Idle,
    DataIn,
    DataOut,
    StatusIn,
    StatusOut,
}

static mut CURRENT_MODE: UsbMode = UsbMode::Off;
static mut CONFIG_VALUE: u8 = 0;
static mut PENDING_ADDR: u8 = 0xFF;
static mut CONTROL_STAGE: ControlStage = ControlStage::Idle;
static mut CONTROL_DATA: [u8; 128] = [0; 128];
static mut CONTROL_DATA_LEN: usize = 0;
static mut CONTROL_DATA_POS: usize = 0;
static mut CONTROL_ZLP: bool = false;

static mut LAST_REPORT: [u8; hid::REPORT_SIZE] = [0; hid::REPORT_SIZE];
static mut LAST_HID_POLL_MS: u32 = 0;
static mut LAST_TELEM_POLL_MS: u32 = 0;

static mut SERIAL_HANDLER: SerialHandler = SerialHandler::new();
static mut PENDING_INIT_MODE: Option<u8> = None;

/// Request deferred USB re-initialization.
pub fn request_init(mode: u8) {
    cortex_m::interrupt::free(|_| unsafe {
        PENDING_INIT_MODE = Some(mode);
    });
}

/// Retrieve and clear any pending USB re-initialization request.
pub fn take_pending_init() -> Option<u8> {
    cortex_m::interrupt::free(|_| unsafe { PENDING_INIT_MODE.take() })
}

/// Returns true if USB is in active flight simulator mode (Joystick configured and attached).
pub fn is_sim_mode() -> bool {
    unsafe { CURRENT_MODE == UsbMode::Joystick && CONFIG_VALUE != 0 }
}

#[inline(always)]
unsafe fn pma_write(offset: u16, src: &[u8]) {
    let base = (PMA_BASE + offset as usize) as *mut u16;
    let mut i = 0;
    while i < src.len() {
        let lo = src[i] as u16;
        let hi = if i + 1 < src.len() {
            (src[i + 1] as u16) << 8
        } else {
            0
        };
        core::ptr::write_volatile(base.add(i >> 1), lo | hi);
        i += 2;
    }
}

#[inline(always)]
unsafe fn pma_read(offset: u16, dst: &mut [u8]) {
    let base = (PMA_BASE + offset as usize) as *const u16;
    let mut i = 0;
    while i < dst.len() {
        let w = core::ptr::read_volatile(base.add(i >> 1));
        dst[i] = (w & 0xFF) as u8;
        if i + 1 < dst.len() {
            dst[i + 1] = (w >> 8) as u8;
        }
        i += 2;
    }
}

#[inline(always)]
unsafe fn pma_write_halfword(offset: u16, val: u16) {
    let ptr = (PMA_BASE + offset as usize) as *mut u16;
    core::ptr::write_volatile(ptr, val);
}

#[inline(always)]
unsafe fn pma_read_halfword(offset: u16) -> u16 {
    let ptr = (PMA_BASE + offset as usize) as *const u16;
    core::ptr::read_volatile(ptr)
}

#[inline(always)]
unsafe fn ep_read(n: u8) -> u16 {
    let ptr = (0x4000_5C00 + 4 * (n as usize)) as *const u16;
    core::ptr::read_volatile(ptr)
}

#[inline(always)]
unsafe fn ep_write(n: u8, val: u16) {
    let ptr = (0x4000_5C00 + 4 * (n as usize)) as *mut u16;
    core::ptr::write_volatile(ptr, val);
}

unsafe fn ep_set_stat_tx(n: u8, status: u16) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        let target = status << 4;
        let toggle = (cur ^ target) & 0x0030;
        ep_write(n, (cur & EP_KEEP_MASK) | EP_CTR_MASK | toggle);
    });
}

unsafe fn ep_set_stat_rx(n: u8, status: u16) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        let target = status << 12;
        let toggle = (cur ^ target) & 0x3000;
        ep_write(n, (cur & EP_KEEP_MASK) | EP_CTR_MASK | toggle);
    });
}

unsafe fn ep_clear_dtog_tx(n: u8) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        let toggle = if (cur & (1 << 6)) != 0 { 1 << 6 } else { 0 };
        ep_write(n, (cur & EP_KEEP_MASK) | EP_CTR_MASK | toggle);
    });
}

unsafe fn ep_clear_dtog_rx(n: u8) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        let toggle = if (cur & (1 << 14)) != 0 { 1 << 14 } else { 0 };
        ep_write(n, (cur & EP_KEEP_MASK) | EP_CTR_MASK | toggle);
    });
}

unsafe fn ep_ack_rx(n: u8) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        ep_write(n, (cur & EP_KEEP_MASK) | 0x0080); // CTR_TX = 1, CTR_RX = 0
    });
}

unsafe fn ep_ack_tx(n: u8) {
    cortex_m::interrupt::free(|_| {
        let cur = ep_read(n);
        ep_write(n, (cur & EP_KEEP_MASK) | 0x8000); // CTR_RX = 1, CTR_TX = 0
    });
}

unsafe fn set_buffers(ep: u8, tx_off: u16, rx_off: u16, rx_count: u16) {
    let base = (8 * ep) as u16;
    pma_write_halfword(base, tx_off);
    pma_write_halfword(base + 2, 0);
    pma_write_halfword(base + 4, rx_off);
    pma_write_halfword(base + 6, rx_count);
}

unsafe fn ep_send(ep: u8, tx_off: u16, data: &[u8]) {
    pma_write(tx_off, data);
    pma_write_halfword(8 * ep as u16 + 2, data.len() as u16);
    ep_set_stat_tx(ep, STAT_VALID);
}

unsafe fn ep0_send_data(data: &[u8], wanted: usize) {
    let len = data.len().min(wanted);
    CONTROL_DATA_LEN = len;
    CONTROL_DATA_POS = 0;
    if len > 0 {
        CONTROL_DATA[..len].copy_from_slice(&data[..len]);
    }
    CONTROL_ZLP = len > 0 && len < wanted && (len % CONTROL_PACKET_SIZE) == 0;
    CONTROL_STAGE = ControlStage::DataIn;
    ep0_send_next();
}

unsafe fn ep0_send_next() {
    let remaining = CONTROL_DATA_LEN - CONTROL_DATA_POS;
    let chunk = remaining.min(CONTROL_PACKET_SIZE);
    let slice = &CONTROL_DATA[CONTROL_DATA_POS..CONTROL_DATA_POS + chunk];
    CONTROL_DATA_POS += chunk;
    ep_send(0, EP0_TX_OFF, slice);
}

unsafe fn ep0_status_in() {
    CONTROL_STAGE = ControlStage::StatusIn;
    ep_send(0, EP0_TX_OFF, &[]);
}

unsafe fn ep0_expect_out(stage: ControlStage) {
    CONTROL_STAGE = stage;
    ep_set_stat_rx(0, STAT_VALID);
}

unsafe fn ep0_stall() {
    ep_set_stat_tx(0, STAT_STALL);
    ep_set_stat_rx(0, STAT_STALL);
}

unsafe fn open_endpoints() {
    if CONFIG_VALUE == 0 {
        return;
    }
    let mode = CURRENT_MODE;
    if mode == UsbMode::Joystick || mode == UsbMode::Composite {
        // EP1: HID Interrupt IN
        ep_write(1, EP_TYPE_INTERRUPT | 1);
        set_buffers(1, EP1_TX_OFF, 0, 0);
        ep_clear_dtog_tx(1);
        ep_set_stat_tx(1, STAT_NAK);
    }
    if mode == UsbMode::Serial || mode == UsbMode::Composite {
        // EP2: CDC Bulk IN (TX) & Bulk OUT (RX)
        ep_write(2, EP_TYPE_BULK | 2);
        set_buffers(2, EP2_TX_OFF, EP2_RX_OFF, RX_COUNT_64);
        ep_clear_dtog_tx(2);
        ep_clear_dtog_rx(2);
        ep_set_stat_tx(2, STAT_NAK);
        ep_set_stat_rx(2, STAT_VALID);

        // EP3: CDC Interrupt IN (Notify)
        ep_write(3, EP_TYPE_INTERRUPT | 3);
        set_buffers(3, EP3_TX_OFF, 0, 0);
        ep_clear_dtog_tx(3);
        ep_set_stat_tx(3, STAT_NAK);
    }
}

/// Initialize the hardware USB peripheral according to the configured USB mode.
pub fn init(mode: u8) {
    let usb_mode = UsbMode::from_u8(mode);
    #[cfg(not(test))]
    unsafe {
        // 1. Immediately deactivate USB processing, mask NVIC
        cortex_m::peripheral::NVIC::mask(pac::Interrupt::USB);
        cortex_m::interrupt::free(|_| {
            CURRENT_MODE = UsbMode::Off;
            CONFIG_VALUE = 0;
            PENDING_ADDR = 0xFF;
            CONTROL_STAGE = ControlStage::Idle;
            SERIAL_HANDLER = SerialHandler::new();
        });

        let rcc = &*pac::RCC::ptr();
        let gpioa = &*pac::GPIOA::ptr();
        let usb = &*pac::USB::ptr();

        // 2. Enable USB clock to access BCDR
        rcc.apb1enr.modify(|_, w| w.usben().set_bit());

        // 3. Disable DPPU and drive PA11 (D-) and PA12 (D+) LOW (SE0) for 250 ms to force detach
        usb.bcdr.write(|w| w.bits(0));
        gpioa.moder.modify(|r, w| {
            w.bits((r.bits() & !((0b11 << 22) | (0b11 << 24))) | ((0b01 << 22) | (0b01 << 24)))
        });
        gpioa
            .bsrr
            .write(|w| w.bits((1 << (11 + 16)) | (1 << (12 + 16))));

        for _ in 0..25 {
            cortex_m::asm::delay(160_000); // 10 ms at 48 MHz
            crate::watchdog::feed();
        }

        // 4. Hardware reset USB peripheral via RCC
        rcc.apb1rstr.modify(|_, w| w.usbrst().set_bit());
        cortex_m::asm::delay(48_000);
        rcc.apb1rstr.modify(|_, w| w.usbrst().clear_bit());

        if usb_mode == UsbMode::Off {
            rcc.apb1enr.modify(|_, w| w.usben().clear_bit());
            gpioa
                .moder
                .modify(|r, w| w.bits(r.bits() | (0b11 << 22) | (0b11 << 24)));
            return;
        }

        // Configure PA11 & PA12 as Input Floating (MODER = 00, PUPDR = 00)
        // STM32F072 connects the USB analog PHY directly to PA11/PA12 when configured as inputs.
        gpioa.moder.modify(|r, w| {
            w.bits(r.bits() & !((0b11 << 22) | (0b11 << 24)))
        });
        gpioa.pupdr.modify(|r, w| {
            w.bits(r.bits() & !((0b11 << 22) | (0b11 << 24)))
        });

        // 5. Initialize USB hardware registers
        // Clear PDWN to wake analog transceiver from power-down
        usb.cntr.modify(|_, w| w.pdwn().clear_bit());

        // Wait t_STARTUP (datasheet requires >= 1 µs, wait 10 µs / 480 cycles at 48 MHz)
        cortex_m::asm::delay(480);

        // Configure BTABLE offset
        usb.btable.write(|w| w.btable().bits(BTABLE_OFF));

        // Release reset (FRES=0, PDWN=0) and enable interrupts in one clean write
        usb.cntr.write(|w| {
            w.pdwn()
                .clear_bit()
                .fres()
                .clear_bit()
                .resetm()
                .set_bit()
                .ctrm()
                .set_bit()
        });

        // Clear any spurious pending interrupt flags in ISTR
        usb.istr.write(|w| w.bits(0));

        // Enable USB device function (Address 0)
        usb.daddr.write(|w| w.ef().set_bit());

        // Setup EP0 (Control)
        ep_write(0, EP_TYPE_CONTROL);
        set_buffers(0, EP0_TX_OFF, EP0_RX_OFF, RX_COUNT_64);
        ep_set_stat_rx(0, STAT_VALID);
        ep_set_stat_tx(0, STAT_NAK);

        // 6. Connect internal pull-up on D+ (DPPU) to signal Full-Speed attachment to the host
        usb.bcdr.write(|w| w.dppu().set_bit());

        // 7. Configure NVIC for USB Interrupt (IRQ 31)
        let mut cp = cortex_m::peripheral::Peripherals::steal();
        cp.NVIC.set_priority(pac::Interrupt::USB, 0xC0);
        cortex_m::peripheral::NVIC::unpend(pac::Interrupt::USB);

        CURRENT_MODE = usb_mode;
        cortex_m::peripheral::NVIC::unmask(pac::Interrupt::USB);
    }
    #[cfg(test)]
    let _ = usb_mode;
}

#[cfg(not(test))]
#[interrupt]
fn USB() {
    on_interrupt();
}

pub fn on_interrupt() {
    #[cfg(not(test))]
    unsafe {
        let usb = &*pac::USB::ptr();

        for _ in 0..16 {
            let istr = usb.istr.read();
            let bits = istr.bits();

            // USB Reset event
            if (bits & (1 << 10)) != 0 {
                // RESET: clear flag by writing 0 to bit 10
                usb.istr.write(|w| w.bits(0xFFFF & !(1 << 10)));
                usb.btable.write(|w| w.btable().bits(BTABLE_OFF));
                ep_write(0, EP_TYPE_CONTROL);
                set_buffers(0, EP0_TX_OFF, EP0_RX_OFF, RX_COUNT_64);
                ep_set_stat_rx(0, STAT_VALID);
                ep_set_stat_tx(0, STAT_NAK);
                usb.daddr.write(|w| w.ef().set_bit());
                CONFIG_VALUE = 0;
                PENDING_ADDR = 0xFF;
                CONTROL_STAGE = ControlStage::Idle;
                continue;
            }

            // Clear harmless status flags (SUSP, WKUP, ERR, PMAOVR)
            let misc = (1 << 11) | (1 << 12) | (1 << 13) | (1 << 14);
            if (bits & misc) != 0 {
                usb.istr.write(|w| w.bits(0xFFFF & !(bits & misc)));
            }

            // Correct Transfer (CTR)
            if (bits & (1 << 15)) == 0 {
                break;
            }

            let ep_id = (bits & 0x0F) as u8;

            if ep_id == 2 {
                // CDC EP2
                let reg = ep_read(2);
                if (reg & 0x8000) != 0 {
                    // CTR_RX (Bulk OUT)
                    let count = (pma_read_halfword(8 * 2 + 6) & 0x3FF) as usize;
                    let n = count.min(CDC_PACKET_SIZE);
                    let mut buf = [0u8; CDC_PACKET_SIZE];
                    pma_read(EP2_RX_OFF, &mut buf[..n]);
                    SERIAL_HANDLER.push_rx(&buf[..n]);
                    ep_ack_rx(2);
                    ep_set_stat_rx(2, STAT_VALID);
                }
                if (reg & 0x0080) != 0 {
                    // CTR_TX (Bulk IN completion)
                    ep_ack_tx(2);
                }
                continue;
            }

            if ep_id != 0 {
                // Class EP1 (HID) or EP3 (Notify)
                let reg = ep_read(ep_id);
                if (reg & 0x0080) != 0 {
                    ep_ack_tx(ep_id);
                }
                if (reg & 0x8000) != 0 {
                    ep_ack_rx(ep_id);
                }
                continue;
            }

            // EP0 transfers
            let ep0 = ep_read(0);
            if (ep0 & 0x8000) != 0 {
                // CTR_RX on EP0
                ep_ack_rx(0);
                if (ep0 & (1 << 11)) != 0 {
                    // SETUP packet
                    handle_ep0_setup();
                } else {
                    handle_ep0_rx_done();
                }
            }
            if (ep_read(0) & 0x0080) != 0 {
                // CTR_TX on EP0
                ep_ack_tx(0);
                handle_ep0_tx_done();
            }
        }
    }
}

#[cfg(not(test))]
unsafe fn handle_ep0_setup() {
    ep_set_stat_rx(0, STAT_NAK);
    ep_set_stat_tx(0, STAT_NAK);

    let mut raw = [0u8; 8];
    pma_read(EP0_RX_OFF, &mut raw);

    let req_type = raw[0];
    let request = raw[1];
    let value = (raw[2] as u16) | ((raw[3] as u16) << 8);
    let index = (raw[4] as u16) | ((raw[5] as u16) << 8);
    let length = (raw[6] as usize) | ((raw[7] as usize) << 8);

    let kind = (req_type >> 5) & 0x03;

    if kind == 0 {
        // Standard request
        match request {
            0x00 => {
                // GET_STATUS
                ep0_send_data(&[0, 0], length);
            }
            0x01 | 0x03 => {
                // CLEAR_FEATURE / SET_FEATURE
                ep0_status_in();
            }
            0x05 => {
                // SET_ADDRESS
                PENDING_ADDR = (value & 0x7F) as u8;
                ep0_status_in();
            }
            0x06 => {
                // GET_DESCRIPTOR
                let desc_type = (value >> 8) as u8;
                let desc_idx = (value & 0xFF) as u8;
                match desc_type {
                    1 => {
                        let d = device_descriptor(CURRENT_MODE);
                        ep0_send_data(d.as_slice(), length);
                    }
                    2 => {
                        let d = config_descriptor(CURRENT_MODE);
                        ep0_send_data(d.as_slice(), length);
                    }
                    3 => {
                        let d = string_descriptor(CURRENT_MODE, desc_idx);
                        ep0_send_data(d.as_slice(), length);
                    }
                    0x22 => {
                        // HID Report Descriptor
                        if CURRENT_MODE == UsbMode::Joystick || CURRENT_MODE == UsbMode::Composite {
                            ep0_send_data(hid::GAMEPAD_REPORT_DESC, length);
                        } else {
                            ep0_stall();
                        }
                    }
                    _ => ep0_stall(),
                }
            }
            0x08 => {
                // GET_CONFIGURATION
                ep0_send_data(&[CONFIG_VALUE], length);
            }
            0x09 => {
                // SET_CONFIGURATION
                CONFIG_VALUE = (value & 0xFF) as u8;
                open_endpoints();
                ep0_status_in();
            }
            0x0A => {
                // GET_INTERFACE
                ep0_send_data(&[0], length);
            }
            0x0B => {
                // SET_INTERFACE
                ep0_status_in();
            }
            _ => ep0_stall(),
        }
    } else if kind == 1 {
        // Class request
        let iface = (index & 0xFF) as u8;
        let lay = descriptors::layout(CURRENT_MODE);

        if (CURRENT_MODE == UsbMode::Serial || CURRENT_MODE == UsbMode::Composite)
            && iface == lay.ctrl_if
        {
            // CDC requests
            match request {
                0x20 => {
                    // SET_LINE_CODING
                    if length == LINE_CODING_DEFAULT.len() {
                        ep0_expect_out(ControlStage::DataOut);
                    } else {
                        ep0_stall();
                    }
                }
                0x21 => {
                    // GET_LINE_CODING
                    ep0_send_data(&LINE_CODING_DEFAULT, length);
                }
                0x22 => {
                    // SET_CONTROL_LINE_STATE
                    ep0_status_in();
                }
                _ => ep0_stall(),
            }
        } else if CURRENT_MODE == UsbMode::Joystick || CURRENT_MODE == UsbMode::Composite {
            // HID requests
            match request {
                0x01 => {
                    // GET_REPORT
                    ep0_send_data(&LAST_REPORT, length);
                }
                0x02 => {
                    // GET_IDLE
                    ep0_send_data(&[0], length);
                }
                0x0A | 0x0B => {
                    // SET_IDLE / SET_PROTOCOL
                    ep0_status_in();
                }
                _ => ep0_stall(),
            }
        } else {
            ep0_stall();
        }
    } else {
        ep0_stall();
    }
}

#[cfg(not(test))]
unsafe fn handle_ep0_rx_done() {
    if CONTROL_STAGE == ControlStage::StatusOut {
        CONTROL_STAGE = ControlStage::Idle;
    } else if CONTROL_STAGE == ControlStage::DataOut {
        ep0_status_in();
    }
    ep_set_stat_rx(0, STAT_NAK);
}

#[cfg(not(test))]
unsafe fn handle_ep0_tx_done() {
    match CONTROL_STAGE {
        ControlStage::DataIn => {
            if CONTROL_DATA_POS < CONTROL_DATA_LEN {
                ep0_send_next();
            } else if CONTROL_ZLP {
                CONTROL_ZLP = false;
                ep_send(0, EP0_TX_OFF, &[]);
            } else {
                ep0_expect_out(ControlStage::StatusOut);
            }
        }
        ControlStage::StatusIn => {
            CONTROL_STAGE = ControlStage::Idle;
            if PENDING_ADDR != 0xFF {
                let usb = &*pac::USB::ptr();
                usb.daddr
                    .write(|w| w.ef().set_bit().add().bits(PENDING_ADDR));
                PENDING_ADDR = 0xFF;
            }
            ep_set_stat_tx(0, STAT_NAK);
        }
        _ => {
            ep_set_stat_tx(0, STAT_NAK);
        }
    }
}

/// Periodic USB task (called from the main loop).
pub fn poll(
    now_ms: u32,
    rf_chs: &[u16; crate::mixer::NUM_CHANNELS],
    switches: &Switches,
    telem: &TelemetryData,
    battery_mv: u16,
) {
    #[cfg(not(test))]
    unsafe {
        let mode = CURRENT_MODE;
        if mode == UsbMode::Off || CONFIG_VALUE == 0 {
            return;
        }

        // 1. Process deferred reset timer (DFU / Reboot)
        match SERIAL_HANDLER.pending_reset() {
            ResetAction::Reboot(at) => {
                if now_ms >= at {
                    cortex_m::peripheral::SCB::sys_reset();
                }
            }
            ResetAction::Dfu(at) => {
                if now_ms >= at {
                    crate::boot::reboot_to_dfu();
                }
            }
            ResetAction::None => {}
        }

        // 2. HID Gamepad report transmission (~100 Hz / 10 ms interval)
        if (mode == UsbMode::Joystick || mode == UsbMode::Composite)
            && now_ms.wrapping_sub(LAST_HID_POLL_MS) >= 10
        {
            LAST_HID_POLL_MS = now_ms;
            // Check if EP1 TX is ready (STAT_TX == NAK)
            let reg = ep_read(1);
            if ((reg >> 4) & 0x03) == STAT_NAK {
                hid::build_gamepad_report(rf_chs, switches, &mut LAST_REPORT);
                ep_send(1, EP1_TX_OFF, &LAST_REPORT);
            }
        }

        // 3. Serial CDC-ACM handling & telemetry streaming
        if mode == UsbMode::Serial || mode == UsbMode::Composite {
            SERIAL_HANDLER.update(now_ms, rf_chs, telem, battery_mv);

            // Stream continuous telemetry at 10 Hz (100 ms) if enabled
            if SERIAL_HANDLER.is_streaming() && now_ms.wrapping_sub(LAST_TELEM_POLL_MS) >= 100 {
                LAST_TELEM_POLL_MS = now_ms;
                SERIAL_HANDLER.send_telemetry_line(rf_chs, telem, battery_mv);
            }

            // Flush pending TX queue into PMA if EP2 TX is ready
            let reg = ep_read(2);
            if ((reg >> 4) & 0x03) == STAT_NAK && SERIAL_HANDLER.has_pending_tx() {
                let mut chunk = [0u8; CDC_PACKET_SIZE];
                let count = SERIAL_HANDLER.pop_tx(&mut chunk);
                if count > 0 {
                    ep_send(2, EP2_TX_OFF, &chunk[..count]);
                }
            }
        }
    }
    #[cfg(test)]
    let _ = (now_ms, rf_chs, switches, telem, battery_mv);
}
