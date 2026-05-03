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
}
