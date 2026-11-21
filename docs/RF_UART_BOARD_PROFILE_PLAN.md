# RF Protocol, Module Selection, and UART Routing Plan

**Status:** Proposed implementation plan  
**Branch:** `plan/rf-uart-board-profile` (based on `dev` at `6187f063fe0aa841a2b1ea86392d39614b29a34e`)  
**Context:** Issues [#45](https://github.com/ferrox-rc/ferrox/issues/45), [#47](https://github.com/ferrox-rc/ferrox/issues/47), and [#48](https://github.com/ferrox-rc/ferrox/issues/48)

## Goal

Make RF build choices, installed internal hardware, external module-bay wiring, and runtime model settings explicit and independent. A firmware build must never offer a module/protocol combination that the selected radio variant cannot physically support, and a selected route must configure the correct UART, pins, duplex behavior, baud/framing, and module power for that radio.

This is a plan, not a claim that the current hardware paths have been validated. Issues #45 and #47 report that external CRSF/MPM communication still failed on RC11; #48 reports a suspected S.Port/RX-path problem for internal CC2500 and asks for further hardware details. Keep those reports open as verification requirements until tested.

## Design decisions

### 1. Separate four concepts

Keep these distinct throughout the core, board profile, build system, menu, and flasher:

1. **RF protocol implementation** — code compiled into firmware, such as CRSF, DSM serial, or a supported native RF driver.
2. **Physical module** — the internal RF module installed in a particular radio, or an external module connected to its bay. Examples: internal ELRS, internal CC2500, external MPM 4-in-1.
3. **Transport/electrical route** — the radio-specific UART(s), TX/RX pins, direction control, module power, baud/framing, and duplex wiring used to reach that module.
4. **Runtime model choice** — which module target/protocol/subprotocol this model uses, limited to choices available in this build and valid for that board.

Do not infer transport from a numeric baud index, and do not treat “protocol compiled” as proof that the selected radio has the corresponding internal hardware.

### 2. Board profile declares physical capability

Extend the compile-time board description with explicit module and bay capability data. Prefer enums and declarative descriptors over loosely related booleans and numeric IDs. Exact Rust names are for implementation discussion; settle them against the existing API before coding.

A board profile should describe:

- Internal module configuration: none, one fixed/default module, or an explicit set of OEM module variants selected at build time.
- Available module target(s): internal, external bay, or both.
- External bay connector type: at least `5-pin` versus `8-pin`, without assuming they share pin numbering or electrical semantics.
- Per-route transport wiring: UART instance(s), TX and RX pins, optional direction-control pin and polarity, module-power pin and polarity, and any board-specific signal constraints.
- Supported duplex/wiring modes for each module route. Half-duplex and full-duplex are distinct capabilities, not a universal “duplex” switch detached from pin routing.
- The default internal module/protocol for a concrete board build.

A shared family profile may supply defaults, but each concrete radio descriptor must be able to override connector, pin assignment, polarity, and supported internal-module options. In particular, do not assume every radio in a family has the same bay or internal RF configuration.

### 3. Cargo features and Web Flasher selection

Use build features to include optional protocol implementations, including protocols intended for external modules. The Web Flasher should expose:

- Radio model.
- Internal module variant/default RF protocol when the radio has OEM module options (for example, RM Pocket ELRS versus CC2500).
- Optional additional RF protocol implementations to include in the image.

Selecting an internal variant must select a valid board-profile configuration, not just change a displayed default. Firmware metadata/manifest should report the chosen board variant and enabled RF protocols so the UI can explain what is available.

The external bay protocol is selected per model at runtime, but it must be selected from the protocols compiled into that image and the transport modes supported by that board’s bay. The same protocol may be compiled for both internal and external modules; the route remains an independent model setting.

Add compile-time validation for invalid combinations: mutually exclusive board targets, mutually exclusive internal OEM variants, protocol feature dependencies, and a valid default protocol/module for the selected board. Keep a safe, explicit behavior for builds with no optional protocol feature rather than silently defaulting to a protocol that is unavailable.

### 4. Separate CC2500 and MPM completely

CC2500-native and serial MULTI-Protocol Module support must have distinct implementation boundaries, including separate source files and separate protocol/configuration types.

- Move the serial MULTI transport/protocol list and MULTI framing/telemetry handling to an MPM-specific module (for example, `rf/mpm.rs`).
- Put native CC2500 RF driver/protocol handling in a CC2500-specific module (for example, `rf/cc2500.rs`).
- Do not use one shared enum such as `Cc2500Protocol` to represent both a native CC2500 chip protocol and a serial MPM protocol selection.
- Keep each feature’s UI list, binding/configuration, telemetry decoding, and build gating independent. Only share small neutral utilities where their semantics genuinely match.

Preserve storage compatibility deliberately: review persisted protocol IDs and current migrations before changing enum representation. Prefer explicit conversion functions and tests over relying on Rust enum discriminants as a permanent storage contract.

### 5. Represent serial configuration with enums and match cases

Replace opaque byte/index conventions with typed transport configuration. Use explicit enums/matches for such choices as:

- Module target: internal or external bay.
- Bay connector/layout: 5-pin or 8-pin, as defined by the board.
- Duplex: half or full, where physically supported.
- UART route / TX and RX assignment.
- Baud rate and frame format (data bits, parity, stop bits), including MULTI 100 kbaud 8E2.
- Module power and S.Port direction polarity.

The UI should show only valid choices. Runtime route setup should match on the selected protocol, module target, and board transport descriptor, then configure the complete UART state atomically: disable old route, stop DMA/interrupt activity, configure GPIO alternate functions and directions, configure UART baud/framing/duplex and RX path, clear only valid status flags, enable the new route, and apply the correct module power. Avoid deriving transport mode from `baud_idx == 5/6` or other magic values.

### 6. Preserve model-level flexibility

Where a board has both an internal module and external bay, users may select either for a model, including cases where both modules use the same air protocol. Keep `module target` independent from `RF protocol`; do not collapse them into one selection. Persist both, sanitize them against the selected build/profile, and ensure the menu’s displayed protocol and active physical route stay in sync after model changes and reboot.

## Repository changes to plan

### A. Hardware reference and inventory

Before changing low-level routing, create a source-backed hardware matrix for every supported radio:

| Radio/build variant | Internal module options | Bay connector | Pin 1..N signal names/directions | Internal UART/pins | External UART/pins | Direction control | Power control/polarity | Duplex support | Reference and confidence |
|---|---|---|---|---|---|---|---|---|---|

Use manufacturer manuals/schematics where available, then established hardware documentation such as ExpressLRS module-bay references and EdgeTX target/hardware definitions. Treat these as leads, not substitutes for the exact revision’s schematic or continuity/logic-analyzer verification. Record the source URL, document/version/page or source lines, radio hardware revision, and uncertainty for every row. Do not copy generic JR/“Lite” pin numbering into a radio profile without validating orientation and connector view.

The externally supplied search summary contains potentially inconsistent claims (including model-specific bay assignments and pin numbering); **do not use those claims as facts until verified against primary references**. Specifically inspect Pocket, TX12 MKII, Zorro variants, Boxer, and Jumper T12, and identify which models actually use 5-pin versus 8-pin bays. Verify both serial TX and telemetry RX signal paths, not just module power and pin 1 output.

### B. Compile-time board and build model

- Add typed, per-board internal module and bay/transport descriptors.
- Add explicit OEM internal-module build variants for radios sold with different internal RF hardware.
- Model protocol features separately from board/module features.
- Expose enabled protocol list and chosen internal variant in build metadata/Web Flasher manifest.
- Validate feature combinations and compute the default protocol from the selected board variant, not from an incidental ordering in a generic protocol list.

### C. RF modules and model setup

- Split CC2500 and MPM code, configuration types, protocol lists, and features.
- Review persisted RF IDs and add migration/sanitization tests if data representation changes.
- Update Protocol Setup so target selection, runtime external protocol selection, and subprotocol choices are distinct and constrained by compile-time capabilities.
- Verify save/reboot behavior for MPM and external selection, including the reported RC10 reset-to-CRSF issue.
- Keep bind state in the protocol setup screen; leaving the screen should stop binding intentionally, not because navigation accidentally exits.

### D. UART/HAL routing

- Replace the F4 `BusMode` heuristics based on baud indexes with a typed route/configuration table supplied by the board profile.
- Apply equivalent explicit routing to F0 boards where relevant; do not assume the F0 and F4 bay layouts are interchangeable.
- Support 5-pin and 8-pin bay routes as separate physical descriptions. A given protocol may have both half-duplex and full-duplex options, but each option must map to the appropriate pins and direction-control behavior for that radio/bay.
- Make route changes safe across active UART/DMA state and module power; reset parser/buffer state when changing endpoint.
- Audit USART flags, parity/data-width/stop-bit configuration, DMA ownership, RXNE/ORE/FE/NE handling, and direction-buffer timing against the MCU reference manual and the actual board schematic.
- Add a diagnostic/status view that reports selected module target, protocol, transport, UART, baud/framing, duplex, and RX activity. This is especially important for issue #47’s report that MPM module status was absent.

### E. Tests, hardware validation, and issue closure

Host tests cannot prove pins or signal direction work. Add tests for configuration selection and routing decisions, then use physical tests for the electrical path.

**Configuration/build tests**

- One valid board target per build; invalid feature combinations fail with a useful diagnostic.
- Internal default is the selected OEM variant; optional compiled protocols appear, and omitted protocols are unavailable in UI/runtime sanitization.
- Internal/external target remains independent of protocol, including same-protocol modules.
- Protocol-specific option lists do not cross-contaminate CC2500 and MPM.
- Enum-to-storage mappings, legacy migrations, persistence across reboot, and route selection are covered.
- Build matrix covers supported board variants, protocol subsets, and 5-/8-pin bay descriptors.

**Hardware matrix tests** (one row per actual radio revision/module combination)

- Confirm connector pin orientation using cited documentation and continuity checks; record module brand/model and wiring.
- Confirm TX waveform at the module input and telemetry/response waveform at the radio RX input using a logic analyzer/oscilloscope.
- Exercise external CRSF/ELRS in half-duplex and full-duplex only where the bay supports it.
- Exercise external MPM with both its TX and telemetry RX connections; verify MULTI module status/telemetry is received, not merely that the transmitter emits frames.
- Exercise internal CC2500 and internal ELRS on the relevant OEM variants and verify the configured internal UART path.
- Check power polarity/voltage, S.Port direction control, baud and 8E2 framing, module power cycling, bind start/stop, model switching, and reboot persistence.

For #45/#47/#48, request and attach radio revision, internal/external module make/model, exact connector pin numbers/wiring, chosen build variant/features, selected model protocol/target/duplex/baud settings, and measured TX/RX activity. Do not close an issue on the basis of firmware compilation or UI behavior alone.

## Suggested implementation sequence

1. **Inventory first:** produce and review the source-backed radio/bay/pin/duplex matrix. Resolve contradictory generic pinout claims before coding route changes.
2. **Define typed model:** agree on board physical capabilities, internal-module variants, module targets, bay layouts, transport modes, and protocol feature semantics.
3. **Separate protocol code:** split CC2500-native and MPM implementation/types/UI/features; add storage compatibility tests.
4. **Wire compile-time selection:** expose internal defaults/variants and optional protocol inclusion through Cargo and Web Flasher; add manifest and invalid-combination checks.
5. **Implement routing:** replace baud-index heuristics with explicit route matches, configure pins/framing/DMA/power as one coherent transition, and update F0/F4 profiles individually.
6. **Update menus/diagnostics:** expose only supported options and display route/RX status.
7. **Verify:** run host/build matrix, then hardware tests on TX12 and each representative 5-/8-pin board and module type. Update issues with evidence.

## Source references to verify against exact hardware revisions

- RadioMaster documentation/downloads: https://www.radiomasterrc.com/downloads
- ExpressLRS module bay pinout reference: https://www.expresslrs.org/hardware/module-bays/pinouts/
- EdgeTX radio hardware/target references: https://wiki.edgetx.org/hardware/radios/
- Jumper T12 manual reference supplied for follow-up verification: https://www.jumper.xyz/download/Jumper-T12-User-Manual.pdf
- Current Ferrox F4 UART implementation: https://github.com/ferrox-rc/ferrox/blob/6187f063fe0aa841a2b1ea86392d39614b29a34e/crates/ferrox-stm32f4/src/uart.rs
- Current Ferrox board profile/feature declarations: https://github.com/ferrox-rc/ferrox/blob/6187f063fe0aa841a2b1ea86392d39614b29a34e/crates/ferrox-core/src/board/qx7_family.rs and https://github.com/ferrox-rc/ferrox/blob/6187f063fe0aa841a2b1ea86392d39614b29a34e/boards/ferrox-f4/Cargo.toml

**Caution:** The source list is a starting point. Verify each connector drawing’s viewing orientation, pin labels, signal direction, voltage, and radio revision before treating a pin map as authoritative. Record source/page/line evidence in the hardware matrix and cite the relevant STM32 reference manual sections for register-level implementation.
