/* Memory map for RadioMaster Pocket (STM32F407VET6) */
MEMORY
{
  /* 512 KB internal flash. For standalone flashing via DFU/SWD: ORIGIN = 0x08000000.
     For EdgeTX SD card bootloader: change ORIGIN to 0x08008000 and LENGTH to 480K. */
  FLASH : ORIGIN = 0x08000000, LENGTH = 512K

  /* 128 KB standard SRAM (SRAM1 112K + SRAM2 16K) */
  RAM : ORIGIN = 0x20000000, LENGTH = 128K

  /* 64 KB Core Coupled Memory (CCM) */
  CCM : ORIGIN = 0x10000000, LENGTH = 64K
}

_stack_start = ORIGIN(RAM) + LENGTH(RAM);
