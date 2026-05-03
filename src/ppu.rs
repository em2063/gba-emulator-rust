use crate::memory_bus::MemoryBus;

pub struct PPU {
    pub framebuffer: [u8; 240 * 160 * 3], //240 x 160 GBA display (RGB pixels)
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            framebuffer: [0; 240 * 160 * 3],
        }
    }

    //Renders BG3 (bitmap-based, 1 tile, no pallete)
    pub fn render_mode3(&mut self, vram: &[u8; 96 * 1024]) {
        for i in 0..(240 * 160) {
            let byte0 = vram[i * 2] as u16;
            let byte1 = vram[i * 2 + 1] as u16;
            let colour = byte0 | (byte1 << 8);

            let r = (colour & 0x1F) as u8;
            let g = ((colour >> 5) & 0x1F) as u8;
            let b = ((colour >> 10) & 0x1F) as u8;

            let r = r << 3;
            let g = g << 3;
            let b = b << 3;

            self.framebuffer[i * 3] = r;
            self.framebuffer[i * 3 + 1] = g;
            self.framebuffer[i * 3 + 2] = b;
        }
    }

    pub fn render_mode4(&mut self, &vram: &[u8; 96 * 1024], &pallete_ram: &[u8; 1024]) {
        for i in 0..(240 * 160) {
            let index = vram[i] as usize;
            let byte0 = pallete_ram[index * 2] as u16;
            let byte1 = pallete_ram[index * 2 + 1] as u16;
            let colour = byte0 | (byte1 << 8);

            let r = (colour & 0x1F) as u8;
            let g = ((colour >> 5) & 0x1F) as u8;
            let b = ((colour >> 10) & 0x1F) as u8;

            let r = r << 3;
            let g = g << 3;
            let b = b << 3;

            self.framebuffer[i * 3] = r;
            self.framebuffer[i * 3 + 1] = g;
            self.framebuffer[i * 3 + 2] = b;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode4_render() {
        let mut ppu = PPU::new();
        let rom: Vec<u8> = std::fs::read("hello.gba").unwrap();
        let mut bus = MemoryBus::new(rom);

        for i in 0..(96 * 1024) {
            bus.vram[i] = 1;
        }

        bus.pallete[2] = 0x1F;
        bus.pallete[3] = 0x00;

        ppu.render_mode4(&bus.vram, &bus.pallete);

        assert_eq!(ppu.framebuffer[0], 248)
    }
}
