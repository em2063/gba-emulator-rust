mod cpu;
mod memory_bus;
mod ppu;
mod thumb;

extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn main() {
    let rom: Vec<u8> = std::fs::read("roms/pokemon.gba").unwrap();
    let mut bus = memory_bus::MemoryBus::new(rom);
    let bios_data = std::fs::read("bios.bin").unwrap();
    bus.bios[..bios_data.len()].copy_from_slice(&bios_data);
    let mut cpu = cpu::CPU::new();
    let mut ppu = ppu::PPU::new();

    // post-BIOS state — values the real BIOS would have set up
    cpu.registers[13] = 0x03007F00; // SP_usr/sys
    cpu.registers[14] = 0x08000000; // LR sentinel
    cpu.registers[15] = 0x08000000; // ROM entry point
    cpu.cpsr = 0x6000001F;          // System mode, Z+C, ARM
    cpu.r13_irq = 0x03007FA0;       // IRQ stack (BIOS default)
    cpu.r13_svc = 0x03007FE0;       // SVC stack (BIOS default)

    //SDL2 setup
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
        .window("GBA Emulator", 480, 320)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 240, 160)
        .unwrap();
    let mut event_pump = sdl_context.event_pump().unwrap();

    // dump instructions around the stuck PC so we can decode the loop

    let mut frame = 0u32;
    let mut in_rom = false;
    let mut last_dispcnt = 0u16;
    'running: loop {
        //handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        for _cycle in 0..280896u32 {
            bus.io[6] = (_cycle / 1232) as u8; // VCOUNT
            let pc = cpu.registers[15];

            if !in_rom && pc >= 0x08000000 {
                println!("[frame {frame}] BIOS done — entered ROM at {pc:#010x}");
                in_rom = true;
            }

            // Null function pointer guard: BX to address 0 means an unset callback.
            // Return via LR instead of running the BIOS reset handler.
            if pc == 0 {
                let lr = cpu.registers[14];
                if lr & 1 == 1 {
                    cpu.cpsr |= 1 << 5;
                } else {
                    cpu.cpsr &= !(1 << 5);
                }
                cpu.registers[15] = lr & !1;
                continue;
            }

            let t_flag = (cpu.cpsr >> 5) & 1;
            if t_flag == 1 {
                let instruction = bus.read_u16(pc);
                cpu.registers[15] = pc.wrapping_add(2);
                cpu.execute_thumb_instruction(&mut bus, instruction);
            } else {
                let instruction = bus.read_u32(pc);
                cpu.registers[15] = pc.wrapping_add(4);
                cpu.execute_instruction(&mut bus, instruction);
            }
        }

        let dispcnt = bus.read_u16(0x04000000);
        if dispcnt != last_dispcnt {
            println!(
                "[frame {frame}] DISPCNT {:#06x}: mode={} forced_blank={} obj={} win0={} win1={} obj_win={} | bg0={} bg1={} bg2={} bg3={}",
                dispcnt,
                dispcnt & 0b111,          // bits 2:0 — display mode
                (dispcnt >> 7) & 1,       // bit 7  — forced blank (white screen)
                (dispcnt >> 12) & 1,      // bit 12 — OBJ/sprite layer enabled
                (dispcnt >> 13) & 1,      // bit 13 — window 0 enable
                (dispcnt >> 14) & 1,      // bit 14 — window 1 enable
                (dispcnt >> 15) & 1,      // bit 15 — OBJ window enable
                (dispcnt >> 8) & 1,       // bit 8  — BG0
                (dispcnt >> 9) & 1,       // bit 9  — BG1
                (dispcnt >> 10) & 1,      // bit 10 — BG2
                (dispcnt >> 11) & 1,      // bit 11 — BG3
            );
            last_dispcnt = dispcnt;
        }

        if frame % 600 == 0 {
            println!(
                "[frame {frame}] PC={:#010x} cpsr={:#010x}",
                cpu.registers[15], cpu.cpsr
            );
        }

        frame += 1;

        // render and display
        let dispcnt = bus.read_u16(0x4000000);
        let mode = dispcnt & 0b111;
        match mode {
            0 => ppu.render_mode0(&bus.vram, &bus.io, &bus.pallete),
            3 => ppu.render_mode3(&bus.vram),
            4 => ppu.render_mode4(&bus.vram, &bus.pallete),
            _ => {}
        }

        texture.update(None, &ppu.framebuffer, 240 * 3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
}
