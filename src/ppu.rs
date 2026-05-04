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

    pub fn render_mode0(&mut self, vram: &[u8; 96 * 1024], io: &[u8; 0x400], palette: &[u8; 1024]) {
        let bg0cnt = io[8] as u16 | ((io[9] as u16) << 8);
        let char_base = ((bg0cnt >> 2) & 0b11) as usize * 16384;
        let screen_base = ((bg0cnt >> 8) & 0b11111) as usize * 2048;

        let charblock = &vram[char_base..];
        let screenblock = &vram[screen_base..];

        //20 rows of tiles (160 / 8)
        for tile_y in 0..20 {
            //30 columns of tiles (240/8)
            for tile_x in 0..30 {
                let entry_idx = (tile_y * 32 + tile_x) as usize;
                let entry = screenblock[entry_idx * 2] as u16
                    | ((screenblock[entry_idx * 2 + 1] as u16) << 8);

                let tile_number = (entry & 0x3FF) as usize;
                for py in 0..8 {
                    for px in 0..8 {
                        let byte = charblock[tile_number * 32 + py * 4 + px / 2];
                        let colour_index = (if px % 2 == 0 {
                            byte & 0xF //lower nibble for even pixels
                        } else {
                            (byte >> 4) & 0xF //upper nibble for odd pixels
                        }) as usize;

                        let colour = palette[colour_index * 2] as u16
                            | ((palette[colour_index * 2 + 1] as u16) << 8);
                        let r = ((colour & 0x1F) as u8) << 3;
                        let g = (((colour >> 5) & 0x1F) as u8) << 3;
                        let b = (((colour >> 10) & 0x1F) as u8) << 3;

                        let fb_x = tile_x * 8 + px;
                        let fb_y = tile_y * 8 + py;
                        let fb_idx = (fb_y * 240 + fb_x) * 3;
                        self.framebuffer[fb_idx] = r;
                        self.framebuffer[fb_idx + 1] = g;
                        self.framebuffer[fb_idx + 2] = b;
                    }
                }
            }
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

    //render mode 4, extracting colour used from pallete RAM
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
