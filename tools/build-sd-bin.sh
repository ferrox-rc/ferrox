#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage: ./tools/build-sd-bin.sh <board> [output-file]

Build a bootloader-flashable EdgeTX SD card firmware image (.bin) for an
STM32F4 target. Links the application at 0x08008000 and prepends the 32 KiB
(32,768-byte) bootloader region padding required by EdgeTX Bootloader's
"Write Firmware" validator.

Supported boards:
  pocket, boxer, zorro, tx12, qx7, tprov2, t12, x9d, x9e, at9spro, gx12

Example:
  ./tools/build-sd-bin.sh zorro
  # Produces ./ferrox-zorro-sd.bin (~134 KiB)
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" || $# -lt 1 ]]; then
    usage
    exit $([[ $# -lt 1 ]] && echo 1 || echo 0)
fi

BOARD="$1"
OUT_FILE="${2:-ferrox-${BOARD}-sd.bin}"

case "${BOARD}" in
    pocket|boxer|zorro|tx12|qx7|tprov2|t12|x9d|x9e|at9spro|gx12) ;;
    *)
        echo "Error: unsupported board '${BOARD}'." >&2
        usage >&2
        exit 1
        ;;
esac

if ! command -v cargo >/dev/null 2>&1; then
    echo "Error: 'cargo' not found in PATH." >&2
    exit 1
fi

if ! cargo objcopy --help >/dev/null 2>&1; then
    echo "Error: 'cargo-objcopy' not installed. Run:" >&2
    echo "  cargo install cargo-binutils && rustup component add llvm-tools-preview" >&2
    exit 1
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

APP_BIN="${TMP_DIR}/app.bin"
PAD_BIN="${TMP_DIR}/pad32k.bin"

echo "Building ferrox-${BOARD} (edgetx-bootloader @ 0x08008000)..."
cargo objcopy \
    -p ferrox-pocket \
    --bin ferrox-pocket \
    --release \
    --target thumbv7em-none-eabihf \
    --no-default-features \
    --features "${BOARD},edgetx-bootloader" \
    -- -O binary "${APP_BIN}"

# EdgeTX Bootloader seeks 32,768 bytes into the .bin before validating the
# Cortex-M vector table (initial SP in 0x20000000.. and Reset_Handler in 0x08008000..).
dd if=/dev/zero bs=1024 count=32 2>/dev/null | tr '\000' '\377' > "${PAD_BIN}"
cat "${PAD_BIN}" "${APP_BIN}" > "${OUT_FILE}"

TOTAL_BYTES=$(wc -c < "${OUT_FILE}")
APP_BYTES=$(wc -c < "${APP_BIN}")
echo "Created ${OUT_FILE}: ${TOTAL_BYTES} bytes (32768 B bootloader pad + ${APP_BYTES} B app)"
if LATEST_COMMIT=$(git log -1 --oneline 2>/dev/null) && [ -n "${LATEST_COMMIT}" ]; then
    echo "Commit : ${LATEST_COMMIT}"
fi
