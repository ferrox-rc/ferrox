use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let memory_x_content = if env::var("CARGO_FEATURE_EDGETX_BOOTLOADER").is_ok() {
        r#"/* Memory map for RadioMaster Pocket (STM32F407VET6) under EdgeTX SD Bootloader */
MEMORY
{
  /* 480 KB application flash offset by 32 KB bootloader at 0x08000000..0x08007FFF */
  FLASH : ORIGIN = 0x08008000, LENGTH = 480K

  /* 128 KB standard SRAM (SRAM1 112K + SRAM2 16K) */
  RAM : ORIGIN = 0x20000000, LENGTH = 128K

  /* 64 KB Core Coupled Memory (CCM) */
  CCM : ORIGIN = 0x10000000, LENGTH = 64K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
"#
    } else {
        r#"/* Memory map for RadioMaster Pocket (STM32F407VET6) standalone DFU/SWD */
MEMORY
{
  /* 512 KB internal flash */
  FLASH : ORIGIN = 0x08000000, LENGTH = 512K

  /* 128 KB standard SRAM (SRAM1 112K + SRAM2 16K) */
  RAM : ORIGIN = 0x20000000, LENGTH = 128K

  /* 64 KB Core Coupled Memory (CCM) */
  CCM : ORIGIN = 0x10000000, LENGTH = 64K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
"#
    };

    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(memory_x_content.as_bytes())
        .unwrap();

    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_EDGETX_BOOTLOADER");
}
