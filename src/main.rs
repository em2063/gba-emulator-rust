mod cpu;
mod memory_bus;
mod ppu;
mod thumb;
mod timer;

extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn main() {
    let rom: Vec<u8> = std::fs::read("tests/ppu/win_demo.gba").unwrap();
    let mut bus = memory_bus::MemoryBus::new(rom); //mem bus setup

    //setup gba bios
    let bios_data = std::fs::read("bios.bin").unwrap();
    bus.bios[..bios_data.len()].copy_from_slice(&bios_data);

    //cpu, ppu
    let mut cpu = cpu::CPU::new();
    let mut ppu = ppu::PPU::new();

    //start PC at 0 and CPSR in supervisor mode
    cpu.registers[15] = 0x00000000;
    cpu.cpsr = 0x000000D3;

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

        let mut last_mirror = 0u32;
        let mut vblank_irq_fired = false;
        for _cycle in 0..280896u32 {
            ppu.tick(&mut bus);
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

                let dispstat_vblank_irq_enable = (bus.io[4] >> 3) & 1 == 1;
                let user_irq_handler = bus.read_u32(0x03007FFC);
                if !cpsr_irq_disabled
                    && ime & 1 == 1
                    && ie & 1 == 1
                    && dispstat_vblank_irq_enable
                    && user_irq_handler != 0
                {
                    let if_val = bus.read_u16(0x04000202) as u32;
                    bus.write_u16(0x04000202, if_val | 1);
                    cpu.trigger_irq(&mut bus);
                }
                bus.trigger_vblank_dma();
            }
            if vcount == 0 {
                vblank_irq_fired = false;
            }

            cpu.registers[15] = match cpu.registers[15] >> 24 {
                0x09 => cpu.registers[15] - 0x01000000,
                0x0A | 0x0B => cpu.registers[15] - 0x02000000,
                0x0C | 0x0D => cpu.registers[15] - 0x04000000,
                _ => cpu.registers[15],
            };

            let pc = cpu.registers[15];

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

            let mirror = cpu.registers[15] >> 24;
            if mirror != last_mirror && mirror >= 8 {
                // println!(
                //      "[cycle {}] Mirror change: 0x{:02x} -> 0x{:02x}, PC={:#010x}, LR={:#010x}, SP={:#010x}, CPSR={:#010x}",
                //     _cycle,
                //     last_mirror,
                //     mirror,
                //     cpu.registers[15],
                //     cpu.registers[14],
                //     cpu.registers[13],
                //     cpu.cpsr
                // );
                last_mirror = mirror;
            }

            for i in 0..4 {
                let overflow = if bus.timers[i].cascade && i > 0 {
                    false
                } else {
                    bus.timers[i].tick(1)
                };

                if overflow {
                    //fire irq
                    if bus.timers[i].irq_enabled {
                        let ie = bus.read_u16(0x04000200);
                        let ime = bus.read_u16(0x04000208);
                        let cpsr_irq_disabled = (cpu.cpsr >> 7) & 1 == 1;
                        if !cpsr_irq_disabled && ime & 1 == 1 && (ie >> (3 + i)) & 1 == 1 {
                            let if_val = bus.read_u16(0x04000202) as u32;
                            bus.write_u16(0x04000202, if_val | (1 << (3 + i)));
                            cpu.trigger_irq(&mut bus);
                        }
                    }

                    //cascade to next timer
                    if i < 3 && bus.timers[i + 1].cascade {
                        bus.timers[i + 1].counter = bus.timers[i + 1].counter.wrapping_add(1);
                        if bus.timers[i + 1].counter == 0 {
                            bus.timers[i + 1].counter = bus.timers[i + 1].reload;
                            //could also fire IRQ for timer i+1 here (TODO)
                        }
                    }
                }
            }
            if vcount < 160 && _cycle % 1232 == 960 {
                bus.trigger_hblank_dma();
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

        ppu.render_sprites(&bus.oam, &bus.palette, &bus.vram, dispcnt);

        texture.update(None, &ppu.framebuffer, 240 * 3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
}
