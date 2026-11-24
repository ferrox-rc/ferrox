//! CRSF MultiSwitch runtime controller (PR #37).
//!
//! Manages 4 independent 8-switch slots (`0..4`), momentary vs. latching bits,
//! and a non-blocking Set4 transmission scheduler that retries cleanly via
//! `crate::uart::try_write_bytes` without ever stalling the flight control loop.

use crate::crsf::protocol::{build_set4, CRSF_SET4_FRAME_SIZE};
use crate::storage::{
    ModelConfig, MULTISWITCH_LABEL_LEN, NUM_MULTISWITCH_SLOTS, NUM_MULTISWITCH_SWITCHES,
};

/// Periodic keep-alive retransmission interval for active MultiSwitch slots (ms).
pub const MULTISWITCH_KEEPALIVE_MS: u32 = 500;
pub const SWITCH_COUNT: u8 = NUM_MULTISWITCH_SWITCHES as u8;
pub const MAX_PAGES: usize = NUM_MULTISWITCH_SLOTS;
pub type Addresses = [i16; MAX_PAGES];

pub const fn enabled_count(addresses: &Addresses) -> usize {
    let mut count = 0;
    let mut slot = 0;
    while slot < MAX_PAGES {
        if addresses[slot] >= 0 && addresses[slot] <= 255 {
            count += 1;
        }
        slot += 1;
    }
    count
}

pub fn slot_at(addresses: &Addresses, mut ordinal: usize) -> Option<usize> {
    for (slot, &address) in addresses.iter().enumerate() {
        if (0..=255).contains(&address) {
            if ordinal == 0 {
                return Some(slot);
            }
            ordinal -= 1;
        }
    }
    None
}

/// Returns `(x, y, width, height)` for a function index in `0..8`.
pub const fn switch_rect(index: u8) -> (i32, i32, u32, u32) {
    (
        1 + (index % 4) as i32 * 32,
        14 + (index / 4) as i32 * 18,
        30,
        17,
    )
}

pub fn label_text<'a>(
    label: &[u8; MULTISWITCH_LABEL_LEN],
    index: u8,
    buf: &'a mut [u8; MULTISWITCH_LABEL_LEN],
) -> &'a str {
    for (out, &c) in buf.iter_mut().zip(label) {
        *out = if (32..=126).contains(&c) { c } else { b' ' };
    }
    if buf.iter().all(|&c| c == b' ') {
        buf[0] = b'1' + index;
    }
    crate::ui::format::ascii_as_str(buf).trim()
}

/// Per-model live MultiSwitch output state and non-blocking CRSF Set4 scheduler.
#[derive(Copy, Clone, Debug)]
pub struct MultiSwitchController {
    /// Live 8-bit switch states for each of the 4 slots.
    pub states: [u8; NUM_MULTISWITCH_SLOTS],
    /// Dirty bitmask (bits 0..4) indicating slots needing immediate Set4 transmission.
    dirty_mask: u8,
    /// Round-robin cursor for fair multi-slot scheduling.
    next_slot: usize,
    /// Last keep-alive timestamp (ms).
    last_keepalive_ms: u32,
}

impl Default for MultiSwitchController {
    fn default() -> Self {
        Self::new()
    }
}

impl MultiSwitchController {
    pub const fn new() -> Self {
        Self {
            states: [0; NUM_MULTISWITCH_SLOTS],
            dirty_mask: 0,
            next_slot: 0,
            last_keepalive_ms: 0,
        }
    }

    /// Reset all switch states on model change.
    pub fn reset(&mut self) {
        self.states = [0; NUM_MULTISWITCH_SLOTS];
        self.dirty_mask = 0x0F;
        self.next_slot = 0;
        self.last_keepalive_ms = 0;
    }

    /// Returns true if at least one MultiSwitch slot has a valid address (`0..=255`).
    #[inline]
    pub fn has_active_slot(model: &ModelConfig) -> bool {
        model.multiswitch_addresses.iter().any(|&a| (0..=255).contains(&a))
    }

    /// Toggle a latching switch (`slot` in `0..4`, `sw` in `0..8`) and mark the slot dirty.
    pub fn toggle(&mut self, slot: usize, sw: usize) {
        if slot < NUM_MULTISWITCH_SLOTS && sw < NUM_MULTISWITCH_SWITCHES {
            self.states[slot] ^= 1u8 << sw;
            self.dirty_mask |= 1u8 << slot;
        }
    }

    /// Set or clear a switch (`slot` in `0..4`, `sw` in `0..8`) and mark dirty if changed.
    pub fn set_switch(&mut self, slot: usize, sw: usize, active: bool) {
        if slot >= NUM_MULTISWITCH_SLOTS || sw >= NUM_MULTISWITCH_SWITCHES {
            return;
        }
        let bit = 1u8 << sw;
        let next = if active {
            self.states[slot] | bit
        } else {
            self.states[slot] & !bit
        };
        if next != self.states[slot] {
            self.states[slot] = next;
            self.dirty_mask |= 1u8 << slot;
        }
    }

    /// Release all momentary switches in `slot` according to `model.multiswitch_momentary[slot]`.
    pub fn release_momentary(&mut self, slot: usize, model: &ModelConfig) {
        if slot >= NUM_MULTISWITCH_SLOTS {
            return;
        }
        let mask = model.multiswitch_momentary[slot];
        if mask != 0 && (self.states[slot] & mask) != 0 {
            self.states[slot] &= !mask;
            self.dirty_mask |= 1u8 << slot;
        }
    }

    /// Emit at most one pending Set4 CRSF frame per tick via non-blocking `try_write_bytes`.
    /// Returns `true` if a frame was accepted by the UART driver.
    pub fn tick(&mut self, now_ms: u32, model: &ModelConfig, crsf_enabled: bool) -> bool {
        if !crsf_enabled || !Self::has_active_slot(model) {
            return false;
        }

        if now_ms.wrapping_sub(self.last_keepalive_ms) >= MULTISWITCH_KEEPALIVE_MS {
            self.last_keepalive_ms = now_ms;
            for (slot, addr) in model
                .multiswitch_addresses
                .iter()
                .enumerate()
                .take(NUM_MULTISWITCH_SLOTS)
            {
                if (0..=255).contains(addr) {
                    self.dirty_mask |= 1u8 << slot;
                }
            }
        }

        for offset in 0..NUM_MULTISWITCH_SLOTS {
            let slot = (self.next_slot + offset) % NUM_MULTISWITCH_SLOTS;
            let bit = 1u8 << slot;
            if (self.dirty_mask & bit) == 0 {
                continue;
            }
            let addr = model.multiswitch_addresses[slot];
            if !(0..=255).contains(&addr) {
                self.dirty_mask &= !bit;
                continue;
            }
            let mut frame = [0u8; CRSF_SET4_FRAME_SIZE];
            let len = build_set4(addr as u8, self.states[slot], &mut frame);
            if crate::uart::try_write_bytes(&frame[..len]) {
                self.dirty_mask &= !bit;
                self.next_slot = (slot + 1) % NUM_MULTISWITCH_SLOTS;
                return true;
            }
            // UART TX busy — leave dirty bit set so the next tick retries cleanly.
            return false;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiswitch_toggle_momentary_and_nonblocking_retry() {
        crate::uart::mock::clear();
        let mut model = ModelConfig::default_for_index(0);
        model.multiswitch_addresses[0] = 42;
        model.multiswitch_momentary[0] = 0b0000_0010; // SW2 is momentary

        let mut ctrl = MultiSwitchController::new();
        ctrl.toggle(0, 0); // SW1 ON
        ctrl.set_switch(0, 1, true); // SW2 ON (momentary)
        assert_eq!(ctrl.states[0], 0b0000_0011);

        // Simulate busy TX -> frame stays pending and does not clear dirty flag
        crate::uart::mock::set_tx_blocked(true);
        assert!(!ctrl.tick(10, &model, true));
        assert_eq!(crate::uart::mock::tx_count(), 0);

        // Unblock TX -> retry succeeds on next tick
        crate::uart::mock::set_tx_blocked(false);
        assert!(ctrl.tick(20, &model, true));
        let sent = crate::uart::mock::take_tx();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].len(), CRSF_SET4_FRAME_SIZE);
        assert_eq!(sent[0][7], 42); // address

        // Release momentary SW2 -> leaves SW1 ON and queues update
        ctrl.release_momentary(0, &model);
        assert_eq!(ctrl.states[0], 0b0000_0001);
        assert!(ctrl.tick(30, &model, true));
    }
}
