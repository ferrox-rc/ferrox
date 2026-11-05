//! USB Descriptors and Endpoint Layout for FS-i6S and FS-i6X.
//!
//! Provides static USB 2.0 descriptors for Joystick (HID), Serial (CDC-ACM),
//! and Composite (HID + CDC-ACM) modes matching OpenTX / EdgeTX standards.

#![allow(dead_code)]

use crate::usb::hid::GAMEPAD_REPORT_DESC;
use crate::usb::UsbMode;

pub const CONTROL_PACKET_SIZE: usize = 64;
pub const CDC_PACKET_SIZE: usize = 64;
pub const HID_PACKET_SIZE: usize = 18;
pub const NOTIFY_PACKET_SIZE: usize = 16;

pub const EP_HID_IN: u8 = 0x81;
pub const EP_CDC_OUT: u8 = 0x02;
pub const EP_CDC_IN: u8 = 0x82;
pub const EP_CDC_NOTIFY: u8 = 0x83;

pub const LINE_CODING_DEFAULT: [u8; 7] = [
    0x00, 0xC2, 0x01, 0x00, // 115,200 baud
    0x00, // 1 stop bit
    0x00, // no parity
    0x08, // 8 data bits
];

pub struct DeviceIdentity {
    pub vid: u16,
    pub pid: u16,
    pub product: &'static str,
    pub serial: &'static str,
}

pub fn identity(mode: UsbMode) -> DeviceIdentity {
    use crate::board::{ActiveBoard, BoardProfile};
    match mode {
        UsbMode::Serial => DeviceIdentity {
            vid: 0x0483,
            pid: 0x5740,
            product: ActiveBoard::USB_PRODUCT_SERIAL,
            serial: ActiveBoard::USB_SERIAL_SERIAL,
        },
        UsbMode::Composite => DeviceIdentity {
            vid: 0x1209,
            pid: 0x4968,
            product: ActiveBoard::USB_PRODUCT_COMPOSITE,
            serial: ActiveBoard::USB_SERIAL_COMPOSITE,
        },
        UsbMode::Joystick | UsbMode::Off => DeviceIdentity {
            vid: 0x1209,
            pid: 0x4F54,
            product: ActiveBoard::USB_PRODUCT_JOYSTICK,
            serial: ActiveBoard::USB_SERIAL_JOYSTICK,
        },
    }
}

pub struct Layout {
    pub hid_if: u8,
    pub ctrl_if: u8,
    pub data_if: u8,
    pub count: u8,
}

pub fn layout(mode: UsbMode) -> Layout {
    match mode {
        UsbMode::Joystick => Layout { hid_if: 0, ctrl_if: 0, data_if: 0, count: 1 },
        UsbMode::Serial => Layout { hid_if: 0, ctrl_if: 0, data_if: 1, count: 2 },
        UsbMode::Composite => Layout { hid_if: 0, ctrl_if: 1, data_if: 2, count: 3 },
        UsbMode::Off => Layout { hid_if: 0, ctrl_if: 0, data_if: 0, count: 0 },
    }
}

pub struct DescriptorBuf<const N: usize> {
    pub buf: [u8; N],
    pub len: usize,
}

impl<const N: usize> DescriptorBuf<N> {
    pub const fn new() -> Self {
        Self {
            buf: [0; N],
            len: 0,
        }
    }
}

impl<const N: usize> Default for DescriptorBuf<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> DescriptorBuf<N> {

    pub fn push(&mut self, byte: u8) {
        if self.len < N {
            self.buf[self.len] = byte;
            self.len += 1;
        }
    }

    pub fn push_u16(&mut self, val: u16) {
        self.push((val & 0xFF) as u8);
        self.push((val >> 8) as u8);
    }

    pub fn push_slice(&mut self, slice: &[u8]) {
        for &b in slice {
            self.push(b);
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

pub fn device_descriptor(mode: UsbMode) -> DescriptorBuf<18> {
    let id = identity(mode);
    let mut d = DescriptorBuf::<18>::new();
    d.push(18); // bLength
    d.push(0x01); // bDescriptorType = Device
    d.push_u16(0x0200); // bcdUSB = 2.00

    match mode {
        UsbMode::Serial => {
            d.push(0x02); // bDeviceClass = CDC
            d.push(0x00); // bDeviceSubClass
            d.push(0x00); // bDeviceProtocol
        }
        UsbMode::Composite => {
            d.push(0xEF); // bDeviceClass = Miscellaneous Device
            d.push(0x02); // bDeviceSubClass = Common Class
            d.push(0x01); // bDeviceProtocol = Interface Association Descriptor
        }
        _ => {
            d.push(0x00); // bDeviceClass
            d.push(0x00); // bDeviceSubClass
            d.push(0x00); // bDeviceProtocol
        }
    }

    d.push(CONTROL_PACKET_SIZE as u8); // bMaxPacketSize0
    d.push_u16(id.vid); // idVendor
    d.push_u16(id.pid); // idProduct
    d.push_u16(0x0100); // bcdDevice = 1.00
    d.push(1); // iManufacturer
    d.push(2); // iProduct
    d.push(3); // iSerialNumber
    d.push(1); // bNumConfigurations
    d
}

pub fn config_descriptor(mode: UsbMode) -> DescriptorBuf<128> {
    let lay = layout(mode);
    let mut d = DescriptorBuf::<128>::new();

    // Configuration Descriptor header
    d.push(9); // bLength
    d.push(0x02); // bDescriptorType = Configuration
    d.push_u16(0); // wTotalLength (patched at end)
    d.push(lay.count); // bNumInterfaces
    d.push(1); // bConfigurationValue
    d.push(0); // iConfiguration
    d.push(0x80); // bmAttributes = Bus Powered
    d.push(50); // bMaxPower = 100 mA (50 * 2mA)

    if mode == UsbMode::Joystick || mode == UsbMode::Composite {
        // Interface Descriptor: HID
        d.push_slice(&[9, 0x04, lay.hid_if, 0, 1, 0x03, 0, 0, 0]);
        // HID Descriptor
        d.push_slice(&[0x09, 0x21, 0x11, 0x01, 0x00, 0x01, 0x22]);
        d.push_u16(GAMEPAD_REPORT_DESC.len() as u16);
        // Endpoint Descriptor: EP1 IN (Interrupt)
        d.push_slice(&[7, 0x05, EP_HID_IN, 0x03]);
        d.push_u16(HID_PACKET_SIZE as u16);
        d.push(10); // bInterval = 10 ms
    }

    if mode == UsbMode::Serial || mode == UsbMode::Composite {
        if mode == UsbMode::Composite {
            // Interface Association Descriptor (IAD) for CDC
            d.push_slice(&[8, 0x0B, lay.ctrl_if, 2, 0x02, 0x02, 0x01, 0]);
        }
        // Interface Descriptor: CDC Control
        d.push_slice(&[9, 0x04, lay.ctrl_if, 0, 1, 0x02, 0x02, 0x01, 0]);
        // Header Functional Descriptor
        d.push_slice(&[0x05, 0x24, 0x00, 0x10, 0x01]);
        // Call Management Functional Descriptor
        d.push_slice(&[0x05, 0x24, 0x01, 0x00, lay.data_if]);
        // ACM Functional Descriptor
        d.push_slice(&[0x04, 0x24, 0x02, 0x02]);
        // Union Functional Descriptor
        d.push_slice(&[0x05, 0x24, 0x06, lay.ctrl_if, lay.data_if]);
        // Endpoint Descriptor: EP3 IN (Interrupt Notify)
        d.push_slice(&[7, 0x05, EP_CDC_NOTIFY, 0x03]);
        d.push_u16(NOTIFY_PACKET_SIZE as u16);
        d.push(255); // bInterval = 255 ms

        // Interface Descriptor: CDC Data
        d.push_slice(&[9, 0x04, lay.data_if, 0, 2, 0x0A, 0x00, 0x00, 0]);
        // Endpoint Descriptor: EP2 OUT (Bulk)
        d.push_slice(&[7, 0x05, EP_CDC_OUT, 0x02]);
        d.push_u16(CDC_PACKET_SIZE as u16);
        d.push(0);
        // Endpoint Descriptor: EP2 IN (Bulk)
        d.push_slice(&[7, 0x05, EP_CDC_IN, 0x02]);
        d.push_u16(CDC_PACKET_SIZE as u16);
        d.push(0);
    }

    // Patch wTotalLength at index 2..3
    let total = d.len as u16;
    d.buf[2] = (total & 0xFF) as u8;
    d.buf[3] = (total >> 8) as u8;
    d
}

pub fn string_descriptor(mode: UsbMode, index: u8) -> DescriptorBuf<64> {
    let mut d = DescriptorBuf::<64>::new();
    if index == 0 {
        // Supported Languages: US English (0x0409)
        d.push_slice(&[4, 0x03, 0x09, 0x04]);
        return d;
    }

    let id = identity(mode);
    let text = match index {
        1 => "FlySky",
        2 => id.product,
        3 => id.serial,
        _ => return d,
    };

    let b_len = 2 + text.len() * 2;
    d.push(b_len as u8);
    d.push(0x03); // bDescriptorType = String
    for b in text.bytes() {
        d.push(b);
        d.push(0); // UTF-16LE zero high byte
    }
    d
}
