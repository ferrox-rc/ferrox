//! Four independently addressed, eight-function CRSF multiswitch pages.

pub const REPEAT_MS: u32 = 100;
pub const SWITCH_COUNT: u8 = 8;
pub const MAX_PAGES: usize = 4;
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

pub fn switch_at(x: u8, y: u8) -> Option<u8> {
    let (x, y) = (x as i32, y as i32);
    (0..SWITCH_COUNT).find(|&index| {
        let (left, top, width, height) = switch_rect(index);
        x >= left && x < left + width as i32 && y >= top && y < top + height as i32
    })
}

pub fn label_text<'a>(label: &[u8; 6], index: u8, buf: &'a mut [u8; 6]) -> &'a str {
    for (out, &c) in buf.iter_mut().zip(label) {
        *out = if (32..=126).contains(&c) { c } else { b' ' };
    }
    if buf.iter().all(|&c| c == b' ') {
        buf[0] = b'1' + index;
    }
    crate::ui::format::ascii_as_str(buf).trim()
}

pub use crate::crsf::protocol::build_set4;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub switches: u8,
    pub last_tx_ms: u32,
    pub pending: bool,
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl State {
    pub const fn new() -> Self {
        Self {
            switches: 0,
            last_tx_ms: 0,
            pending: true,
        }
    }

    pub fn toggle(&mut self, index: u8) {
        if index < SWITCH_COUNT {
            self.set_switches(self.switches ^ (1 << index));
        }
    }

    pub fn set_switches(&mut self, value: u8) -> bool {
        if self.switches == value {
            return false;
        }
        self.switches = value;
        self.pending = true;
        true
    }

    pub const fn due(&self, now_ms: u32) -> bool {
        self.pending || now_ms.wrapping_sub(self.last_tx_ms) >= REPEAT_MS
    }

    pub fn sent(&mut self, now_ms: u32) {
        self.last_tx_ms = now_ms;
        self.pending = false;
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Pages {
    pub addresses: Addresses,
    pub momentary: [u8; MAX_PAGES],
    pub states: [State; MAX_PAGES],
    pub next_slot: u8,
}

impl Default for Pages {
    fn default() -> Self {
        Self::new()
    }
}

impl Pages {
    pub const fn new() -> Self {
        Self {
            addresses: [-1; MAX_PAGES],
            momentary: [0; MAX_PAGES],
            states: [State::new(); MAX_PAGES],
            next_slot: 0,
        }
    }

    pub fn configure(&mut self, configured: &Addresses, modes: &[u8; MAX_PAGES]) {
        for slot in 0..MAX_PAGES {
            let address = if (0..=255).contains(&configured[slot]) {
                configured[slot]
            } else {
                -1
            };
            if self.addresses[slot] != address {
                self.states[slot] = State::new();
                self.addresses[slot] = address;
            }
            self.states[slot]
                .set_switches(self.states[slot].switches & !(self.momentary[slot] ^ modes[slot]));
            self.momentary[slot] = modes[slot];
        }
    }

    pub fn update_touch(
        &mut self,
        slot: Option<usize>,
        held: Option<u8>,
        tapped: Option<u8>,
    ) -> bool {
        let mut changed = false;
        for i in 0..MAX_PAGES {
            let mut value = self.states[i].switches & !self.momentary[i];
            if slot == Some(i) && (0..=255).contains(&self.addresses[i]) {
                if let Some(index) = held.filter(|&index| index < SWITCH_COUNT) {
                    value |= (1 << index) & self.momentary[i];
                }
                if let Some(index) = tapped.filter(|&index| index < SWITCH_COUNT) {
                    if self.momentary[i] & (1 << index) == 0 {
                        value ^= 1 << index;
                    }
                }
            }
            changed = self.states[i].set_switches(value) || changed;
        }
        changed
    }

    pub fn due(&self, now_ms: u32) -> Option<u8> {
        (0..MAX_PAGES)
            .map(|i| (self.next_slot as usize + i) % MAX_PAGES)
            .find(|&slot| {
                (0..=255).contains(&self.addresses[slot]) && self.states[slot].due(now_ms)
            })
            .map(|slot| slot as u8)
    }

    pub fn sent(&mut self, slot: u8, now_ms: u32) {
        if let Some(state) = self.states.get_mut(slot as usize) {
            state.sent(now_ms);
            self.next_slot = (slot + 1) % MAX_PAGES as u8;
        }
    }

    pub fn switches(&self) -> [u8; MAX_PAGES] {
        self.states.map(|state| state.switches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touch_releases_momentary_but_preserves_toggles() {
        let mut pages = Pages::new();
        pages.configure(&[0, -1, 255, -1], &[1, 0, 2, 0]);
        assert!(pages.update_touch(Some(0), Some(0), Some(1)));
        assert_eq!(pages.switches(), [3, 0, 0, 0]);
        assert!(pages.update_touch(Some(2), Some(1), None));
        assert_eq!(pages.switches(), [2, 0, 2, 0]);
        assert!(pages.update_touch(None, None, None));
        assert_eq!(pages.switches(), [2, 0, 0, 0]);
        assert!(!pages.update_touch(Some(0), Some(8), Some(255)));
        pages.configure(&[0, -1, 255, -1], &[3, 0, 2, 0]);
        assert_eq!(pages.switches(), [0; 4]);
        pages.states[0].toggle(2);
        pages.configure(&[1, -2, 256, -1], &[3, 0, 2, 0]);
        assert_eq!(pages.addresses, [1, -1, -1, -1]);
        assert_eq!(pages.switches(), [0; 4]);
    }

    #[test]
    fn round_robin_repeat_and_wrapping_clock() {
        let mut pages = Pages::new();
        pages.configure(&[0, -1, 2, 3], &[0; 4]);
        let now = u32::MAX - 49;
        for slot in [0, 2, 3] {
            assert_eq!(pages.due(now), Some(slot));
            pages.sent(slot, now);
        }
        assert_eq!(pages.due(49), None);
        assert_eq!(pages.due(50), Some(0));
        pages.states[2].toggle(7);
        assert_eq!(pages.due(49), Some(2));
        pages.sent(2, 49);
        assert_eq!(pages.due(50), Some(3));
    }

    #[test]
    fn geometry_enabled_slots_and_labels() {
        assert_eq!(enabled_count(&[0, -1, 255, 256]), 2);
        assert_eq!(slot_at(&[0, -1, 255, 256], 1), Some(2));
        assert_eq!(slot_at(&[0, -1, 255, 256], 2), None);
        for index in 0..8 {
            let (x, y, width, height) = switch_rect(index);
            assert_eq!(switch_at(x as u8, y as u8), Some(index));
            assert_eq!(
                switch_at((x + width as i32 - 1) as u8, (y + height as i32 - 1) as u8),
                Some(index)
            );
            assert_eq!(switch_at((x + width as i32) as u8, y as u8), None);
        }
        assert_eq!(switch_at(255, 14), None);
        let mut buf = [0; 6];
        assert_eq!(label_text(b" A\0B  ", 0, &mut buf), "A B");
        assert_eq!(label_text(&[0; 6], 7, &mut buf), "8");
        assert_eq!(label_text(b"ABCDEF", 0, &mut buf), "ABCDEF");
    }
}
