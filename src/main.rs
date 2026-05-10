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

    cpu.registers[15] = 0x00000000;
    cpu.cpsr = 0x600000D3; //supervisor mode

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

        let mut vblank_irq_fired = false;
        for _cycle in 0..280896u32 {
            let vcount = (_cycle / 1232) % 228;
            bus.io[6] = vcount as u8; //VCOUNT low byte
            bus.io[7] = (vcount >> 8) as u8; //VCOUNT high byte

            //update DISPSTAT vblank flag
            if vcount >= 160 {
                bus.io[4] |= 1; //set vblank bit
            } else {
                bus.io[4] &= !1; //clear vblank bit
            }

            if vcount == 160 && !vblank_irq_fired {
                vblank_irq_fired = true;
                let ie = bus.read_u16(0x04000200);
                let ime = bus.read_u16(0x04000208);
                let cpsr_irq_disabled = (cpu.cpsr >> 7) & 1 == 1; // bit 7 = I flag
                if !cpsr_irq_disabled && ime & 1 == 1 && ie & 1 == 1 {
                    // only fire if IRQs are enabled
                    let if_val = bus.read_u16(0x04000202) as u32;
                    bus.write_u16(0x04000202, if_val | 1);
                    cpu.trigger_irq(&mut bus);
                }
            }
            if vcount == 0 {
                vblank_irq_fired = false;
            }

            let pc = cpu.registers[15];
            if pc == 0xFFFFFFFE || pc == 0xFFFFFFFF {
                println!("Bad PC! Last instruction area: {:#010x}", cpu.registers[14]);
                // print nearby memory
                for i in 0..4 {
                    println!(
                        "  [{:#010x}] = {:#010x}",
                        cpu.registers[14].wrapping_sub(8).wrapping_add(i * 4),
                        bus.read_u32(cpu.registers[14].wrapping_sub(8).wrapping_add(1 * 4) & !1)
                    );
                }
            }

            if !in_rom && pc >= 0x08000000 {
                println!("[frame {frame}] BIOS done — entered ROM at {pc:#010x}");
                in_rom = true;
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
                dispcnt & 0b111,     // bits 2:0 — display mode
                (dispcnt >> 7) & 1,  // bit 7  — forced blank (white screen)
                (dispcnt >> 12) & 1, // bit 12 — OBJ/sprite layer enabled
                (dispcnt >> 13) & 1, // bit 13 — window 0 enable
                (dispcnt >> 14) & 1, // bit 14 — window 1 enable
                (dispcnt >> 15) & 1, // bit 15 — OBJ window enable
                (dispcnt >> 8) & 1,  // bit 8  — BG0
                (dispcnt >> 9) & 1,  // bit 9  — BG1
                (dispcnt >> 10) & 1, // bit 10 — BG2
                (dispcnt >> 11) & 1, // bit 11 — BG3
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

        //render and display
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
