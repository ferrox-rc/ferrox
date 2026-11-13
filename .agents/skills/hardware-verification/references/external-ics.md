# External Peripheral & IC Datasheets

## ST7567 (128x64 Monochrome LCD)
- **Interface**: SPI (Mode 0/3) or 8-bit 8080 parallel; max SCLK ~10–20 MHz.
- **Control**: `DC` (`A0`) must be stable before `CS` assertion and clocking; wait for `SPI_SR.BSY == 0` before toggling `DC`/`CS`.
- **Addressing**: 8 pages (`0..7`), 128 columns (`0..127`). Per page: set page `0xB0 | page`, column high `0x10 | (col >> 4)`, column low `col & 0x0F`, then burst 128 data bytes.

## FT6236 (Capacitive Touch Controller)
- **I2C**: 7-bit address `0x38`, up to `400 kHz`.
- **Reset Timing**: Pull `TOUCH_RST` LOW for >= 5 ms (use `20 ms` = `960_000` cycles @ `48 MHz`), then wait >= 20 ms (`960_000` cycles @ `48 MHz`) after releasing HIGH before I2C reads.

## M24256 / 24C256 (32 KB I2C EEPROM)
- **I2C**: 7-bit address `0x50`, 16-bit big-endian byte address, `64`-byte page write boundary.
- **Write Cycle**: ~5 ms internal write; use `ACK` polling (`poll_ack`) before the next read/write.
- **Bounds**: Always check `addr.checked_add(len)` to avoid `u16` wraparound.

## SD Card (SPI Mode)
- **Initialization**: Run SPI <= `400 kHz` during `CMD0` -> `CMD8` -> `CMD55` + `ACMD41` (up to `1000 ms`) -> `CMD58` (OCR bit 30 `CCS` = SDHC/SDXC block addressing).
- **Watchdog**: Feed `IWDG` (`0xAAAA`) inside the `ACMD41` retry loop so 1 s init does not trigger a reset.
- **Addressing**: For SDSC (`!is_sdhc`), use `sector_num.checked_mul(512)` to prevent `u32` overflow.
- **Speedup**: After `CMD58`, switch SPI clock to fast mode (<= `21–25 MHz`) and ensure `OSPEEDR = High Speed`.

## A7105 (FlySky 2.4 GHz RF Transceiver)
- **Interface**: SPI up to `10 MHz`, `GIO2` TX/RX completion interrupt.
- **Concurrency**: `GIO2` EXTI ISR and main loop share RF driver / `TripleBuffer` state; protect all shared accesses with `critical_section` (`cortex_m::interrupt::free` on ARM).
