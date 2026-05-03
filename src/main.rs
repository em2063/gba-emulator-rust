mod cpu;
mod memory_bus;
mod ppu;
mod thumb;

extern crate sdl2;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

fn main() {
    let rom: Vec<u8> = std::fs::read("hello.gba").unwrap();
    let mut bus = memory_bus::MemoryBus::new(rom);
    let mut cpu = cpu::CPU::new();
    let mut ppu = ppu::PPU::new();

    cpu.registers[13] = 0x03008000; //top of iwram
    cpu.registers[15] = 0x08000000;

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

        for _ in 0..280896 {
            let pc = cpu.registers[15];
            let t_flag = (cpu.cpsr >> 5) & 1;
            if t_flag == 1 {
                let instruction = bus.read_u16(cpu.registers[15]);
                // println!("PC: {:#010x} THUMB instruction: {:#010x}", pc, instruction);
                cpu.registers[15] = cpu.registers[15].wrapping_add(2);
                cpu.execute_thumb_instruction(&mut bus, instruction);
            } else {
                let instruction = bus.read_u32(cpu.registers[15]);
                // println!("PC: {:#010x} instruction: {:#010x}", pc, instruction);
                cpu.registers[15] = cpu.registers[15].wrapping_add(4);
                cpu.execute_instruction(&mut bus, instruction);
            }
        }

        // render and display
        ppu.render_mode3(&bus.vram);
        texture.update(None, &ppu.framebuffer, 240 * 3).unwrap();
        canvas.copy(&texture, None, None).unwrap();
        canvas.present();
    }
}
