---
name: hardware-verification
description: >-
  Verify bare-metal, HAL, and peripheral driver code in ferrox-stm32f0,
  ferrox-stm32f4, and boards/* against STM32 reference manuals (RM0091, RM0090),
  silicon errata (ES0260, ES0182), and external IC datasheets. Use when writing,
  debugging, or reviewing low-level MCU or peripheral driver changes.
---

# Hardware, Datasheet & Errata Verification

Use this workflow to review or author bare-metal / HAL / peripheral driver code efficiently without re-searching basic datasheet facts.

## Workflow

### 1. Scope & Partition
- When reviewing changes, restrict inspection to hardware-touching diffs (e.g., `git diff <base>..HEAD -- crates/ferrox-stm32f0 crates/ferrox-stm32f4 boards/`).
- If reviewing both MCU families at once, split into two parallel `research` subagents (`STM32F0` vs `STM32F4`) so datasheet lookups stay out of the main context.

### 2. Load Cached Reference
Read only the reference file(s) matching the target crate/peripheral:
- STM32F072 / APM32F072 (`crates/ferrox-stm32f0`): [stm32f0-rm0091-es0260.md](./references/stm32f0-rm0091-es0260.md)
- STM32F407 (`crates/ferrox-stm32f4`): [stm32f4-rm0090-es0182.md](./references/stm32f4-rm0090-es0182.md)
- External ICs (ST7567, FT6236, M24256, SD SPI, A7105): [external-ics.md](./references/external-ics.md)

If a register, erratum, or IC detail is not covered in `references/`, verify it against primary vendor documentation before relying on another firmware's sequence.

### 3. Five-Point Hardware Checklist
Check each driver change against:
1. **Register & Bitfield Semantics**: Verify reset states, `rc_w0` / `rc_w1` / toggle bits (`TIMx_SR`, `USART_ICR`, `USB_EPnR`), and avoid writing `1` to reserved bits.
2. **Bus Clocks, Barriers & Errata**: Check `APB1`/`APB2` clock domains, `RCC` clock-enable read-back delays, `DSB`/`ISB` around DFU/VTOR jumps, and I2C/SPI errata workarounds.
3. **DMA, SPI & ISR Sequencing**: Check `DMA_EN == 0` wait + flag clear before re-arming, `read_volatile` on DMA buffers, `SPI_SR.OVR` clearing, and critical sections on ISR-shared state.
4. **Clock & Timing Math**: Verify `cortex_m::asm::delay` cycle counts against `SYSCLK` (`48 MHz` F0, `168 MHz` F4), I2C/SPI prescalers, bounded timeouts on all hardware waits, and `IWDG` feeding in long loops.
5. **Pin & Alternate-Function Mapping**: Verify `descriptor.rs` / GPIO setup for pin collisions (e.g., output LED vs switch input, rotary encoder vs key) and AF numbers.

### 4. Cite & Validate
- Cite the specific manual/datasheet/errata section when reporting or fixing a hardware bug.
- Verify affected targets with `cargo clippy ... -- -D warnings` and `cargo build --release`.
