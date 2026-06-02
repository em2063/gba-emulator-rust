mod cpu;
mod flash;
mod memory_bus;
mod ppu;
mod thumb;
mod timer;

extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn main() {
    let rom: Vec<u8> = std::fs::read("roms/pokemon.gba").unwrap();
    let mut bus = memory_bus::MemoryBus::new(rom); //mem bus setup

    //setup gba bios
    let bios_data = std::fs::read("bios.bin").unwrap();
    bus.bios[..bios_data.len()].copy_from_slice(&bios_data);

    //cpu, ppu
    let mut cpu = cpu::CPU::new();
    let mut ppu = ppu::PPU::new();

    cpu.registers[15] = 0x00000000; //start PC from 0   
    cpu.cpsr = 0x000000D3; //SVC mode, IRQ/FIQ disabled
    bus.write_u16(0x04000130, 0x03FF); //set keyinput so all buttons are unpressed (1)

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
    let mut lcd_blend = true; //emulate the GBA LCD's pixel bleed (toggle with L)
    let mut blend_buf = vec![0u8; 240 * 160 * 3]; //scratch for the blend pass

    'running: loop {
        //handle events
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(k), ..
                } => {
                    if k == Keycode::Backquote {
                        dump_ppu_regs(&bus, frame); //` dumps the current PPU register state
                    } else if k == Keycode::L {
                        lcd_blend = !lcd_blend; //L toggles the LCD blend filter
                        println!("LCD blend: {}", if lcd_blend { "on" } else { "off" });
                    } else if let Some(b) = key_to_bit(k) {
                        bus.set_key(b, true);
                    }
                }
                Event::KeyUp {
                    keycode: Some(k), ..
                } => {
                    if let Some(b) = key_to_bit(k) {
                        bus.set_key(b, false);
                    }
                }
                _ => {}
            }
        }

        for _cycle in 0..280896u32 {
            ppu.tick(&mut cpu, &mut bus);

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
                            bus.io[0x202] |= 1u8 << (3 + i);
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

        let pixels = if lcd_blend {
            lcd_blend_frame(&ppu.framebuffer, &mut blend_buf);
            &blend_buf
        } else {
            &ppu.framebuffer[..]
        };
        texture.update(None, pixels, 240 * 3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
}

//Light spatial blur approximating the GBA LCD's pixel bleed. Each output pixel is
//a weighted average of itself (weight 4) and its 4 neighbours (weight 1 each). For
//a 2-colour dither checkerboard this resolves to the mean of the two shades — i.e.
//the smooth gradient real hardware shows — while leaving solid areas near-untouched.
fn lcd_blend_frame(src: &[u8], dst: &mut [u8]) {
    const W: usize = 240;
    const H: usize = 160;
    for y in 0..H {
        for x in 0..W {
            for c in 0..3 {
                let idx = (y * W + x) * 3 + c;
                let mut sum = src[idx] as u32 * 4;
                let mut weight = 4u32;
                if x > 0 {
                    sum += src[idx - 3] as u32;
                    weight += 1;
                }
                if x < W - 1 {
                    sum += src[idx + 3] as u32;
                    weight += 1;
                }
                if y > 0 {
                    sum += src[idx - W * 3] as u32;
                    weight += 1;
                }
                if y < H - 1 {
                    sum += src[idx + W * 3] as u32;
                    weight += 1;
                }
                dst[idx] = (sum / weight) as u8;
            }
        }
    }
}

//On-demand snapshot of the PPU control registers. Reads the IO block directly
//(same layout the PPU itself uses) so it reflects exactly what the renderer sees.
fn dump_ppu_regs(bus: &memory_bus::MemoryBus, frame: u32) {
    let r = |off: usize| bus.io[off] as u16 | ((bus.io[off + 1] as u16) << 8);

    let dispcnt = r(0x00);
    let mode = dispcnt & 0b111;
    println!("=== PPU register dump @ frame {frame} ===");
    println!(
        "DISPCNT {dispcnt:#06x}  mode={mode}  forced_blank={}  obj={}  1d_map={}  win0={} win1={} objwin={}",
        (dispcnt >> 7) & 1,
        (dispcnt >> 12) & 1,
        (dispcnt >> 6) & 1,
        (dispcnt >> 13) & 1,
        (dispcnt >> 14) & 1,
        (dispcnt >> 15) & 1,
    );

    let mosaic = r(0x4C);
    println!(
        "MOSAIC  {mosaic:#06x}  bg={}x{}  obj={}x{}",
        (mosaic & 0xF) + 1,
        ((mosaic >> 4) & 0xF) + 1,
        ((mosaic >> 8) & 0xF) + 1,
        ((mosaic >> 12) & 0xF) + 1,
    );

    for bg in 0..4usize {
        let enabled = (dispcnt >> (8 + bg)) & 1;
        let cnt = r(0x08 + bg * 2);
        let hofs = r(0x10 + bg * 4) & 0x1FF;
        let vofs = r(0x12 + bg * 4) & 0x1FF;
        let char_block = ((cnt >> 2) & 0b11) as u32;
        let screen_block = ((cnt >> 8) & 0b11111) as u32;
        println!(
            "BG{bg} en={enabled} cnt={cnt:#06x} prio={} char_base={} ({:#08x}) screen_base={} ({:#08x}) 8bpp={} mosaic={} size={} overflow={} scroll=({hofs},{vofs})",
            cnt & 0b11,
            char_block,
            0x06000000 + char_block * 0x4000,
            screen_block,
            0x06000000 + screen_block * 0x800,
            (cnt >> 7) & 1,
            (cnt >> 6) & 1,
            (cnt >> 14) & 0b11,
            (cnt >> 13) & 1,
        );

        //for enabled text BGs, sample VRAM so we can tell a good tilemap from
        //corrupt tile graphics: map indices should be small & structured, while
        //tile bytes reveal whether the pixel data is real or noise.
        let is_text_bg = mode == 0 || (mode == 1 && bg < 2);
        if enabled == 1 && is_text_bg {
            let char_off = char_block as usize * 0x4000;
            let screen_off = screen_block as usize * 0x800;
            let is_8bpp = (cnt >> 7) & 1 == 1;

            let mut map = String::new();
            for e in 0..16usize {
                let o = screen_off + e * 2;
                let entry = bus.vram[o] as u16 | ((bus.vram[o + 1] as u16) << 8);
                map.push_str(&format!("{:04x} ", entry));
            }
            println!("     map[0..16] @ {:#08x}: {map}", 0x06000000 + screen_off as u32);

            let tile_bytes = if is_8bpp { 64 } else { 32 };
            for t in 0..2usize {
                let mut row = String::new();
                for b in 0..tile_bytes {
                    row.push_str(&format!("{:02x}", bus.vram[char_off + t * tile_bytes + b]));
                    if b % 2 == 1 {
                        row.push(' ');
                    }
                }
                println!(
                    "     tile{t} @ {:#08x}: {row}",
                    0x06000000 + (char_off + t * tile_bytes) as u32
                );
            }
        }
    }

    let bldcnt = r(0x50);
    println!(
        "BLDCNT {bldcnt:#06x} mode={} 1st_tgt={:#04x} 2nd_tgt={:#04x}  BLDALPHA {:#06x}  BLDY {:#06x}",
        (bldcnt >> 6) & 0b11,
        bldcnt & 0x3F,
        (bldcnt >> 8) & 0x3F,
        r(0x52),
        r(0x54),
    );
    println!(
        "WININ {:#06x} WINOUT {:#06x}  WIN0H {:#06x} WIN0V {:#06x}  WIN1H {:#06x} WIN1V {:#06x}",
        r(0x48),
        r(0x4A),
        r(0x40),
        r(0x44),
        r(0x42),
        r(0x46),
    );

    //BG palette banks 0-3 as (r,g,b) 0-31 triples. Lets us see whether adjacent
    //dither indices (e.g. 9 vs 10) are near-identical shades (smooth fill on real
    //hardware) or contrasting colours (a wrong/garish palette).
    let pal = |i: usize| bus.palette[i * 2] as u16 | ((bus.palette[i * 2 + 1] as u16) << 8);
    for bank in 0..4usize {
        let mut s = String::new();
        for c in 0..16usize {
            let v = pal(bank * 16 + c);
            s.push_str(&format!("{:02},{:02},{:02}  ", v & 0x1F, (v >> 5) & 0x1F, (v >> 10) & 0x1F));
        }
        println!("BGPAL bank{bank} (idx0..15 as r,g,b): {s}");
    }
    println!("==========================================");
}

fn key_to_bit(k: Keycode) -> Option<u8> {
    match k {
        Keycode::O => Some(0),         //A
        Keycode::P => Some(1),         //B
        Keycode::Backspace => Some(2), //select
        Keycode::Return => Some(3),    //start
        Keycode::D => Some(4),         //right
        Keycode::A => Some(5),         //left
        Keycode::W => Some(6),         //up
        Keycode::S => Some(7),         //down
        Keycode::E => Some(8),         //right bumper
        Keycode::Q => Some(9),         //left bumper
        _ => None,
    }
}
