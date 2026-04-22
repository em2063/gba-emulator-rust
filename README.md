# gba-emu

A Game Boy Advance emulator written from scratch in Rust. Currently focused on accurate CPU emulation (ARM7TDMI core that powered the GBA). After I will move on to graphics, audio, and peripherals.

---

## Why Rust?

Rust is a natural fit. The combination of exhaustive `match` statements, wrapping arithmetic, and zero-cost bit manipulation makes it feel easy. Common implementations use C or C++.

---

## Architecture

```
src/
├── main.rs        — fetch-decode-execute loop, mode detection (ARM/THUMB)
├── cpu.rs         — ARM instruction set, ALU, barrel shifter, PSR, flags
├── thumb.rs       — THUMB instruction set (16-bit mode), implemented as CPU extension
└── memory_bus.rs  — GBA memory map, all memory regions, little-endian r/w
```

The CPU is split across two files: `cpu.rs` handles the 32-bit ARM instruction set, and `thumb.rs` extends the same `CPU` struct with the 16-bit THUMB instruction set via a separate `impl` block. This mirrors the real hardware since THUMB is not a separate processor, it's the same ARM7TDMI running in a compressed instruction mode, toggled by the T bit in CPSR.

---

## Running

```sh
# Place a ROM at the project root
cargo run
```

---

## References

- [ARM7TDMI Technical Reference Manual](https://developer.arm.com/documentation/ddi0210/latest)
- [GBATEK — comprehensive GBA hardware documentation](https://problemkaputt.de/gbatek.htm)
