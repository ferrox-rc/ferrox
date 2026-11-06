//! Direct PAC USB 2.0 Full-Speed (12 Mbps) driver for STM32F072.
//!
//! Controls USB PMA registers, descriptors, endpoints, and USB interrupt.

#![allow(static_mut_refs, dead_code)]

use ferrox_core::hal::UsbDriver;
use ferrox_core::input::Switches;
use ferrox_core::rf::afhds2a::TelemetryData;
use ferrox_core::usb::descriptors::{
    config_descriptor, device_descriptor, string_descriptor, CDC_PACKET_SIZE, CONTROL_PACKET_SIZE,
    LINE_CODING_DEFAULT,
};
use ferrox_core::usb::hid;
use ferrox_core::usb::serial::{ResetAction, SerialHandler};
use ferrox_core::usb::UsbMode;
use stm32f0xx_hal::pac::{self, interrupt};

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

unsafe fn ep_clear_ctr_rx(n: u8) {
    let cur = ep_read(n);
    ep_write(n, (cur & EP_KEEP_MASK & !0x8000) | 0x0080);
}

unsafe fn ep_clear_ctr_tx(n: u8) {
    let cur = ep_read(n);
    ep_write(n, (cur & EP_KEEP_MASK & !0x0080) | 0x8000);
}

unsafe fn set_buffers(ep: u8, tx_off: u16, rx_off: u16, rx_count_reg: u16) {
    let b = ep as u16 * 8;
    pma_write_halfword(b, tx_off);
    pma_write_halfword(b + 2, 0);
    pma_write_halfword(b + 4, rx_off);
    pma_write_halfword(b + 6, rx_count_reg);
}

unsafe fn ep_send(ep: u8, tx_off: u16, data: &[u8]) {
    pma_write(tx_off, data);
    let b = ep as u16 * 8;
    pma_write_halfword(b + 2, data.len() as u16);
    ep_set_stat_tx(ep, STAT_VALID);
}

unsafe fn ep0_expect_out(next_stage: ControlStage) {
    CONTROL_STAGE = next_stage;
    ep_set_stat_rx(0, STAT_VALID);
}

unsafe fn ep0_send_next() {
    let rem = CONTROL_DATA_LEN - CONTROL_DATA_POS;
    let chunk = rem.min(CONTROL_PACKET_SIZE);
    ep_send(
        0,
        EP0_TX_OFF,
        &CONTROL_DATA[CONTROL_DATA_POS..CONTROL_DATA_POS + chunk],
    );
    CONTROL_DATA_POS += chunk;
}

unsafe fn ep0_start_in(data: &[u8], req_len: u16) {
    let send_len = data.len().min(req_len as usize);
    CONTROL_DATA[..send_len].copy_from_slice(&data[..send_len]);
    CONTROL_DATA_LEN = send_len;
    CONTROL_DATA_POS = 0;
    CONTROL_ZLP = (send_len < req_len as usize) && send_len.is_multiple_of(CONTROL_PACKET_SIZE);
    CONTROL_STAGE = ControlStage::DataIn;
    ep0_send_next();
}

unsafe fn ep0_stall() {
    CONTROL_STAGE = ControlStage::Idle;
    ep_set_stat_tx(0, STAT_STALL);
    ep_set_stat_rx(0, STAT_STALL);
}

unsafe fn ep0_send_zlp() {
    CONTROL_STAGE = ControlStage::StatusIn;
    ep_send(0, EP0_TX_OFF, &[]);
}

pub struct Stm32f0UsbDriver;

impl Stm32f0UsbDriver {
    pub const fn new() -> Self {
        Self
    }
}

impl Default for Stm32f0UsbDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl UsbDriver for Stm32f0UsbDriver {
    fn init(&mut self, mode: u8) {
        let usb_mode = UsbMode::from_u8(mode);
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
                ferrox_core::watchdog::feed();
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

            gpioa.moder.modify(|r, w| {
                w.bits(r.bits() & !((0b11 << 22) | (0b11 << 24)))
            });
            gpioa.pupdr.modify(|r, w| {
                w.bits(r.bits() & !((0b11 << 22) | (0b11 << 24)))
            });

            // 5. Initialize USB hardware registers
            usb.cntr.modify(|_, w| w.pdwn().clear_bit());
            cortex_m::asm::delay(480);
            usb.btable.write(|w| w.btable().bits(BTABLE_OFF));

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

            usb.istr.write(|w| w.bits(0));
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
    }

    fn poll(
        &mut self,
        now_ms: u32,
        rf_chs: &[u16; ferrox_core::mixer::NUM_CHANNELS],
        switches: &Switches,
        telem: &TelemetryData,
        battery_mv: u16,
    ) {
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
                        ferrox_core::chip::enter_dfu_bootloader();
                    }
                }
                ResetAction::None => {}
            }

            // 2. HID Gamepad report transmission (~100 Hz / 10 ms interval)
            if (mode == UsbMode::Joystick || mode == UsbMode::Composite)
                && now_ms.wrapping_sub(LAST_HID_POLL_MS) >= 10
            {
                LAST_HID_POLL_MS = now_ms;
                let reg = ep_read(1);
                if ((reg >> 4) & 0x03) == STAT_NAK {
                    hid::build_gamepad_report(rf_chs, switches, &mut LAST_REPORT);
                    ep_send(1, EP1_TX_OFF, &LAST_REPORT);
                }
            }

            // 3. Serial CDC-ACM handling & telemetry streaming
            if mode == UsbMode::Serial || mode == UsbMode::Composite {
                SERIAL_HANDLER.update(now_ms, rf_chs, telem, battery_mv);

                if SERIAL_HANDLER.is_streaming() && now_ms.wrapping_sub(LAST_TELEM_POLL_MS) >= 100 {
                    LAST_TELEM_POLL_MS = now_ms;
                    SERIAL_HANDLER.send_telemetry_line(rf_chs, telem, battery_mv);
                }

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
    }

    fn is_sim_mode(&self) -> bool {
        unsafe { CURRENT_MODE == UsbMode::Joystick && CONFIG_VALUE != 0 }
    }
}

#[interrupt]
fn USB() {
    unsafe {
        let usb = &*pac::USB::ptr();
        let istr = usb.istr.read().bits() as u16;

        if (istr & 0x0400) != 0 {
            usb.istr.write(|w| w.bits(!0x0400));
            on_reset();
            return;
        }

        if (istr & 0x8000) != 0 {
            let ep_id = (istr & 0x000F) as u8;
            on_endpoint_ctr(ep_id);
        }
    }
}

unsafe fn on_reset() {
    CONFIG_VALUE = 0;
    PENDING_ADDR = 0xFF;
    CONTROL_STAGE = ControlStage::Idle;

    let usb = &*pac::USB::ptr();
    usb.daddr.write(|w| w.ef().set_bit().add().bits(0));

    ep_write(0, EP_TYPE_CONTROL);
    set_buffers(0, EP0_TX_OFF, EP0_RX_OFF, RX_COUNT_64);
    ep_set_stat_rx(0, STAT_VALID);
    ep_set_stat_tx(0, STAT_NAK);

    let mode = CURRENT_MODE;
    if mode == UsbMode::Joystick || mode == UsbMode::Composite {
        ep_write(1, EP_TYPE_INTERRUPT | 1);
        set_buffers(1, EP1_TX_OFF, 0, 0);
        ep_set_stat_tx(1, STAT_NAK);
        ep_set_stat_rx(1, STAT_DISABLED);
    }

    if mode == UsbMode::Serial || mode == UsbMode::Composite {
        ep_write(2, EP_TYPE_BULK | 2);
        set_buffers(2, EP2_TX_OFF, EP2_RX_OFF, RX_COUNT_64);
        ep_set_stat_tx(2, STAT_NAK);
        ep_set_stat_rx(2, STAT_VALID);

        ep_write(3, EP_TYPE_INTERRUPT | 3);
        set_buffers(3, EP3_TX_OFF, 0, 0);
        ep_set_stat_tx(3, STAT_NAK);
        ep_set_stat_rx(3, STAT_DISABLED);
    }
}

unsafe fn on_endpoint_ctr(ep_id: u8) {
    let reg = ep_read(ep_id);

    if (reg & 0x8000) != 0 {
        ep_clear_ctr_rx(ep_id);
        let b = ep_id as u16 * 8;
        let rx_count = (pma_read_halfword(b + 6) & 0x03FF) as usize;
        let rx_off = pma_read_halfword(b + 4);

        if ep_id == 0 {
            let is_setup = (reg & 0x0800) != 0;
            let mut buf = [0u8; 64];
            pma_read(rx_off, &mut buf[..rx_count.min(64)]);
            on_ep0_rx(is_setup, &buf[..rx_count.min(64)]);
        } else if ep_id == 2 {
            let mut buf = [0u8; CDC_PACKET_SIZE];
            let len = rx_count.min(CDC_PACKET_SIZE);
            pma_read(rx_off, &mut buf[..len]);
            SERIAL_HANDLER.push_rx(&buf[..len]);
            ep_set_stat_rx(2, STAT_VALID);
        }
    }

    if (reg & 0x0080) != 0 {
        ep_clear_ctr_tx(ep_id);
        if ep_id == 0 {
            on_ep0_tx();
        }
    }
}

unsafe fn on_ep0_rx(is_setup: bool, data: &[u8]) {
    if is_setup {
        if data.len() < 8 {
            ep0_stall();
            return;
        }

        let bm_request_type = data[0];
        let b_request = data[1];
        let w_value = u16::from_le_bytes([data[2], data[3]]);
        let w_index = u16::from_le_bytes([data[4], data[5]]);
        let w_length = u16::from_le_bytes([data[6], data[7]]);

        let req_type = (bm_request_type >> 5) & 0x03;

        match req_type {
            0 => handle_standard_request(bm_request_type, b_request, w_value, w_index, w_length),
            1 => handle_class_request(bm_request_type, b_request, w_value, w_index, w_length),
            _ => ep0_stall(),
        }
    } else {
        match CONTROL_STAGE {
            ControlStage::DataOut => {
                ep0_send_zlp();
            }
            ControlStage::StatusOut => {
                CONTROL_STAGE = ControlStage::Idle;
            }
            _ => {}
        }
    }
}

unsafe fn handle_standard_request(
    bm_request_type: u8,
    b_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
) {
    let mode = CURRENT_MODE;
    match b_request {
        0x00 => {
            let zero = [0u8, 0u8];
            ep0_start_in(&zero, w_length);
        }
        0x05 => {
            PENDING_ADDR = (w_value & 0x7F) as u8;
            ep0_send_zlp();
        }
        0x06 => {
            let desc_type = (w_value >> 8) as u8;
            let desc_index = (w_value & 0xFF) as u8;

            match desc_type {
                1 => {
                    let desc = device_descriptor(mode);
                    ep0_start_in(desc.as_slice(), w_length);
                }
                2 => {
                    let desc = config_descriptor(mode);
                    ep0_start_in(desc.as_slice(), w_length);
                }
                3 => {
                    let desc = string_descriptor(mode, desc_index);
                    if desc.len > 0 {
                        ep0_start_in(desc.as_slice(), w_length);
                    } else {
                        ep0_stall();
                    }
                }
                0x22 => {
                    if (mode == UsbMode::Joystick || mode == UsbMode::Composite) && w_index == 0 {
                        ep0_start_in(hid::GAMEPAD_REPORT_DESC, w_length);
                    } else {
                        ep0_stall();
                    }
                }
                _ => ep0_stall(),
            }
        }
        0x08 => {
            let cfg = [CONFIG_VALUE];
            ep0_start_in(&cfg, w_length);
        }
        0x09 => {
            CONFIG_VALUE = (w_value & 0xFF) as u8;
            ep0_send_zlp();
        }
        _ => {
            let _ = (bm_request_type, w_index);
            ep0_stall();
        }
    }
}

unsafe fn handle_class_request(
    bm_request_type: u8,
    b_request: u8,
    _w_value: u16,
    _w_index: u16,
    w_length: u16,
) {
    if (bm_request_type & 0x80) != 0 {
        match b_request {
            0x21 => {
                ep0_start_in(&LINE_CODING_DEFAULT, w_length);
            }
            _ => ep0_stall(),
        }
    } else {
        match b_request {
            0x20 => {
                ep0_expect_out(ControlStage::DataOut);
            }
            0x22 => {
                ep0_send_zlp();
            }
            _ => ep0_stall(),
        }
    }
}

unsafe fn on_ep0_tx() {
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
