# Ferrox Pilot Hardware Tester Field Guide

Welcome to the pilot hardware testing program for **Ferrox**! This guide is designed for community testers evaluating pre-release builds across supported transmitters:
- **RadioMaster Pocket**
- **RadioMaster Zorro**
- **FrSky Taranis X9E** (Wide 212×64 display)
- **Jumper T12 / T12 Pro**
- **RadioMaster GX12** (OLED display)

> [!IMPORTANT]
> **100% Unbrickable / Zero-Risk Testing:**
> All supported radios feature read-only hardware ROM bootloaders directly in silicon. Furthermore, by using the **EdgeTX SD Bootloader (Method A)**, you do **not** need to install PC drivers, open radio cases, or connect DFU tools. Testing is non-destructive, and your radio can be rolled back to your original EdgeTX firmware in under 10 seconds directly from the radio.

---

## 1. Preparing the Test Binary

You can either download a pre-built `-sd.bin` test build provided by the development team, or build it yourself from source.

> [!NOTE]
> EdgeTX Bootloader seeks **32 KiB (32,768 bytes)** into any `.bin` file on the SD card before validating the Cortex-M vector table and writing flash at `0x08008000`. Use `./tools/build-sd-bin.sh` (which builds with `edgetx-bootloader` and prepends the 32 KiB pad) so your output is ~134 KiB rather than an unpadded ~102 KiB raw binary.

```bash
# Ensure llvm-tools-preview and cargo-binutils are installed:
rustup component add llvm-tools-preview
cargo install cargo-binutils

# Build bootloader-ready SD binary (~134 KiB):
./tools/build-sd-bin.sh pocket   # RadioMaster Pocket -> ferrox-pocket-sd.bin
./tools/build-sd-bin.sh zorro    # RadioMaster Zorro  -> ferrox-zorro-sd.bin
./tools/build-sd-bin.sh x9e      # FrSky Taranis X9E  -> ferrox-x9e-sd.bin
./tools/build-sd-bin.sh t12      # Jumper T12         -> ferrox-t12-sd.bin
./tools/build-sd-bin.sh gx12     # RadioMaster GX12   -> ferrox-gx12-sd.bin
```

If you prefer running the steps manually (e.g. for `zorro`):
```bash
cargo objcopy -p ferrox-pocket --bin ferrox-pocket --release --target thumbv7em-none-eabihf --no-default-features --features "zorro,edgetx-bootloader" -- -O binary app.bin
dd if=/dev/zero bs=1024 count=32 | tr '\000' '\377' > pad32k.bin
cat pad32k.bin app.bin > ferrox-zorro-sd.bin
```

---

## 2. Flashing via EdgeTX SD Bootloader (Recommended)

1. **Copy Binary to MicroSD Card:**
   - Mount your radio's microSD card (via USB Storage mode or card reader).
   - Copy the corresponding `.bin` file (e.g. `ferrox-pocket-sd.bin`) into the `/FIRMWARE/` folder.
   - **Crucial:** Ensure your current stock EdgeTX firmware binary (e.g. `edgetx-pocket.bin`) is also in `/FIRMWARE/` so you can revert anytime.
2. **Enter EdgeTX Bootloader:**
   - Power the radio **OFF**.
   - Push and hold **both horizontal trims inward** (Roll Left + Yaw Right) while pressing and holding the **Power** button.
   - The screen will light up with the EdgeTX Bootloader menu.
3. **Flash Ferrox:**
   - Select **Write Firmware**.
   - Navigate to `ferrox-<target>-sd.bin` and press **Enter** (or long-press the roller).
   - Confirm flashing (takes ~3–5 seconds).
   - Select **[Exit]** or power off.
4. **Booting Ferrox:**
   - Turn the radio ON normally. Ferrox will boot immediately!

---

## 3. Alternative: Direct USB DFU Flashing (Factory Flashing)

If your radio does not have an EdgeTX SD bootloader installed or you wish to flash via USB DFU:

1. Power the radio **OFF** and remove the microSD card.
2. Plug a USB-C data cable into the **top USB port** (data/sim port, not charge port). The screen and LEDs remain dark; the MCU boots into ST System ROM DFU mode (`0483:df11`).
3. Backup your entire stock flash memory (512 KB):
   ```bash
   dfu-util -a 0 -s 0x08000000:524288 -U stock_backup.bin
   ```
4. Flash the standalone `.bin` (built without `edgetx-bootloader` feature):
   ```bash
   dfu-util -a 0 -s 0x08000000:leave -D ferrox-<target>.bin
   ```

---

## 4. Five-Minute Sanity Testing Checklist

Please execute this 5-minute sanity check and copy the results into your feedback report:

| Step | Test Area | What to Look For | Result | Notes |
| :---: | :--- | :--- | :---: | :--- |
| **1** | **Display & Contrast** | Screen turns on with sharp contrast and correct orientation. No ghosting or shifted columns. | `[ ] PASS / [ ] FAIL` | |
| **2** | **Audio & Buzzer** | Clean, distinct beeps and tones on button clicks and scroll wheel turns. | `[ ] PASS / [ ] FAIL` | |
| **3** | **Navigation & Controls** | Rotary encoder (roller) scrolls cleanly up/down without jumping or skipped steps. OK / Back buttons work reliably. | `[ ] PASS / [ ] FAIL` | |
| **4** | **Gimbals & Calibration** | Navigate to `SYS -> CALIBRATION` or `DIAG -> ANALOG`. All 4 stick axes track smoothly from 0% to 100% and center near 50%. | `[ ] PASS / [ ] FAIL` | |
| **5** | **Switches & Pots** | Navigate to `DIAG -> SWITCHES`. Flip all physical switches (`SA`..`SH`) and rotate pots (`S1`, `S2`). All positions detect accurately. | `[ ] PASS / [ ] FAIL` | |
| **6** | **RF Link & Telemetry** | Select model protocol (`CRSF/ELRS` or `CC2500`). Verify internal or external RF module powers up, links/binds to receiver, and telemetry updates. | `[ ] PASS / [ ] FAIL` | |
| **7** | **Power Off & Rollback** | Long-press power button to shut down cleanly. Hold horizontal trims inward + power on to re-enter EdgeTX bootloader, then flash stock EdgeTX to confirm 10-second rollback. | `[ ] PASS / [ ] FAIL` | |

---

## 5. Instant 10-Second Rollback to Stock EdgeTX

1. Power the radio **OFF**.
2. Push and hold **both horizontal trims inward** while pressing the **Power** button.
3. Select **Write Firmware**, select your original EdgeTX `.bin`, and confirm.
4. Power cycle. The transmitter is 100% restored to stock EdgeTX with all models and settings untouched.

---

## 6. Tester Feedback Template

Please copy and paste this template when reporting feedback:

```markdown
### Ferrox Pilot Test Report

- **Radio Model:** (e.g. RadioMaster Pocket ELRS / RadioMaster Zorro CC2500 / FrSky X9E)
- **Flashing Method:** [ ] EdgeTX SD Bootloader  [ ] Direct USB DFU
- **Ferrox Firmware Version / Git Commit:** 

#### Checklist Results
- [ ] Display & Contrast: (PASS / FAIL - Notes: )
- [ ] Audio & Buzzer: (PASS / FAIL - Notes: )
- [ ] Navigation & Scroll Wheel: (PASS / FAIL - Notes: )
- [ ] Gimbal Travel & Calibration: (PASS / FAIL - Notes: )
- [ ] Physical Switches & Pots: (PASS / FAIL - Notes: )
- [ ] RF Module Power & Link: (PASS / FAIL - Protocol tested: )
- [ ] Power Off & Rollback Test: (PASS / FAIL)

#### General Impressions & Observations
(Detail any unexpected behaviour, stick inversions, switch mis-mappings, or UI glitches observed)
```
