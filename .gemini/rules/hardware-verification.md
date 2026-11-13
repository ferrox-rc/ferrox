# Hardware, Datasheet & Errata Verification

When writing, debugging, or reviewing bare-metal / HAL / peripheral driver code (`crates/ferrox-stm32f0`, `crates/ferrox-stm32f4`, `boards/*`), activate and follow the [`hardware-verification`](../../.agents/skills/hardware-verification/SKILL.md) skill and its cached `references/`:

1. **Cross-reference primary silicon & IC docs** (cached in `.agents/skills/hardware-verification/references/` for RM0091/ES0260, RM0090/ES0182, and external ICs; verify any uncached details against vendor manuals/errata and reputable references like EdgeTX, Betaflight, Embassy).
2. **Never rely solely on another firmware's code** without confirming why its register sequence works against the hardware datasheet.
3. **Cite the specific manual/datasheet/errata section** when diagnosing or fixing low-level hardware bugs.
