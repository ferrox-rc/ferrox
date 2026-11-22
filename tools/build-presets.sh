#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="${1:-ci-artifacts}"
mkdir -p "${OUT_DIR}"

echo "=== Building ferrox-i6x-apm32 ==="
cargo build --release -p ferrox-i6x --no-default-features \
  --features apm32,rf-afhds2a,rf-crsf,rf-cc2500,rf-dsm,dsm-fwd-prog,soaring,logical-switches \
  --target thumbv6m-none-eabi
cargo objcopy -p ferrox-i6x --bin ferrox-i6x --release --no-default-features \
  --features apm32,rf-afhds2a,rf-crsf,rf-cc2500,rf-dsm,dsm-fwd-prog,soaring,logical-switches \
  --target thumbv6m-none-eabi -- -O binary "${OUT_DIR}/ferrox-i6x-apm32.bin"

echo "=== Building ferrox-i6s-soaring ==="
cargo build --release -p ferrox-i6s --no-default-features \
  --features stm32,touch,rf-afhds2a,rf-crsf,soaring \
  --target thumbv6m-none-eabi
cargo objcopy -p ferrox-i6s --bin ferrox-i6s --release --no-default-features \
  --features stm32,touch,rf-afhds2a,rf-crsf,soaring \
  --target thumbv6m-none-eabi -- -O binary "${OUT_DIR}/ferrox-i6s-soaring.bin"

cat > "${OUT_DIR}/manifest.json" <<'EOF'
{
  "version": 1,
  "boards": {
    "i6s": [
      { "id": "standard", "label": "Standard (STM32 + All RF)", "bin": "ferrox-i6s.bin" },
      { "id": "soaring", "label": "Soaring / F5J (STM32 + CRSF + AFHDS2A)", "bin": "ferrox-i6s-soaring.bin" }
    ],
    "i6x": [
      { "id": "standard", "label": "Standard (STM32 + All RF + Soaring)", "bin": "ferrox-i6x.bin" },
      { "id": "apm32", "label": "APM32 Clone (APM32 + All RF + Soaring)", "bin": "ferrox-i6x-apm32.bin" }
    ],
    "pocket": [{ "id": "standard", "label": "Standard", "bin": "ferrox-pocket.bin", "sdBin": "ferrox-pocket-sd.bin" }],
    "boxer": [{ "id": "standard", "label": "Standard", "bin": "ferrox-boxer.bin", "sdBin": "ferrox-boxer-sd.bin" }],
    "zorro": [{ "id": "standard", "label": "Standard", "bin": "ferrox-zorro.bin", "sdBin": "ferrox-zorro-sd.bin" }],
    "tx12": [{ "id": "standard", "label": "Standard", "bin": "ferrox-tx12.bin", "sdBin": "ferrox-tx12-sd.bin" }],
    "gx12": [{ "id": "standard", "label": "Standard", "bin": "ferrox-gx12.bin", "sdBin": "ferrox-gx12-sd.bin" }],
    "qx7": [{ "id": "standard", "label": "Standard", "bin": "ferrox-qx7.bin", "sdBin": "ferrox-qx7-sd.bin" }],
    "x9d": [{ "id": "standard", "label": "Standard", "bin": "ferrox-x9d.bin", "sdBin": "ferrox-x9d-sd.bin" }],
    "x9e": [{ "id": "standard", "label": "Standard", "bin": "ferrox-x9e.bin", "sdBin": "ferrox-x9e-sd.bin" }],
    "tprov2": [{ "id": "standard", "label": "Standard", "bin": "ferrox-tprov2.bin", "sdBin": "ferrox-tprov2-sd.bin" }],
    "t12": [{ "id": "standard", "label": "Standard", "bin": "ferrox-t12.bin", "sdBin": "ferrox-t12-sd.bin" }],
    "at9spro": [{ "id": "standard", "label": "Standard", "bin": "ferrox-at9spro.bin", "sdBin": "ferrox-at9spro-sd.bin" }]
  }
}
EOF
