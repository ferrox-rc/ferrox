#!/usr/bin/env python3
"""Build an EdgeTX SD bootloader binary with 32 KiB padding.

Cross-platform: works on Windows, Linux, and macOS.
"""
import argparse
import os
import subprocess
import sys
import tempfile

VALID_BOARDS = [
    "pocket", "boxer", "zorro", "tx12", "qx7", "tprov2", "t12",
    "x9d", "x9e", "at9spro", "gx12"
]

def main():
    parser = argparse.ArgumentParser(
        description="Build an EdgeTX SD bootloader binary with 32 KiB padding."
    )
    parser.add_argument("board", choices=VALID_BOARDS, help="Target board")
    parser.add_argument("outfile", nargs="?", help="Output file (default: ferrox-<board>-sd.bin)")
    args = parser.parse_args()

    board = args.board.lower()
    outfile = args.outfile or f"ferrox-{board}-sd.bin"

    print(f"Building ferrox-{board} (edgetx-bootloader @ 0x08008000)...")

    with tempfile.TemporaryDirectory() as tmpdir:
        app_bin = os.path.join(tmpdir, "app.bin")
        cmd = [
            "cargo", "objcopy",
            "-p", "ferrox-pocket",
            "--bin", "ferrox-pocket",
            "--release",
            "--target", "thumbv7em-none-eabihf",
            "--no-default-features",
            "--features", f"{board},edgetx-bootloader",
            "--", "-O", "binary", app_bin
        ]
        res = subprocess.run(cmd)
        if res.returncode != 0:
            sys.exit(res.returncode)

        with open(app_bin, "rb") as f:
            app_bytes = f.read()

        pad = b"\xff" * 32768
        with open(outfile, "wb") as f:
            f.write(pad)
            f.write(app_bytes)

        total = len(pad) + len(app_bytes)
        print(f"Created {outfile}: {total} bytes (32768 B bootloader pad + {len(app_bytes)} B app)")
        git_res = subprocess.run(
            ["git", "-c", "safe.directory=*", "log", "-1", "--oneline"],
            capture_output=True,
            text=True,
        )
        if git_res.returncode == 0 and git_res.stdout.strip():
            print(f"Commit : {git_res.stdout.strip()}")
        else:
            print("Commit : (unavailable - not a git checkout)")

if __name__ == "__main__":
    main()
