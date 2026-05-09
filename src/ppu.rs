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
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);

        // Fill with backdrop color (BG palette entry 0)
        let bd = palette[0] as u16 | ((palette[1] as u16) << 8);
        let (bd_r, bd_g, bd_b) = (
            ((bd & 0x1F) as u8) << 3,
            (((bd >> 5) & 0x1F) as u8) << 3,
            (((bd >> 10) & 0x1F) as u8) << 3,
        );
        for i in 0..(240 * 160) {
            self.framebuffer[i * 3] = bd_r;
            self.framebuffer[i * 3 + 1] = bd_g;
            self.framebuffer[i * 3 + 2] = bd_b;
        }

        // Render BG3 → BG0 (low priority first, high priority on top)
        for bg_num in (0usize..4).rev() {
            if (dispcnt >> (8 + bg_num)) & 1 == 0 {
                continue;
            }
            let cnt_off = 8 + bg_num * 2; // BG0CNT=io[8], BG1CNT=io[10], ...
            let bgcnt = io[cnt_off] as u16 | ((io[cnt_off + 1] as u16) << 8);
            let char_base = ((bgcnt >> 2) & 0b11) as usize * 16384;
            let is_8bpp = (bgcnt >> 7) & 1 == 1;
            let screen_base = ((bgcnt >> 8) & 0b11111) as usize * 2048;

            for tile_y in 0..20usize {
                for tile_x in 0..30usize {
                    let entry_idx = tile_y * 32 + tile_x;
                    let e0 = screen_base + entry_idx * 2;
                    if e0 + 1 >= vram.len() { continue; }
                    let entry = vram[e0] as u16 | ((vram[e0 + 1] as u16) << 8);

                    let tile_num = (entry & 0x3FF) as usize;
                    let flip_h = (entry >> 10) & 1 == 1;
                    let flip_v = (entry >> 11) & 1 == 1;
                    let pal_base = if is_8bpp { 0 } else { ((entry >> 12) & 0xF) as usize * 16 };

                    for py in 0..8usize {
                        for px in 0..8usize {
                            let sy = if flip_v { 7 - py } else { py };
                            let sx = if flip_h { 7 - px } else { px };

                            let colour_index = if is_8bpp {
                                let off = char_base + tile_num * 64 + sy * 8 + sx;
                                if off >= vram.len() { continue; }
                                vram[off] as usize
                            } else {
                                let off = char_base + tile_num * 32 + sy * 4 + sx / 2;
                                if off >= vram.len() { continue; }
                                let byte = vram[off];
                                if sx % 2 == 0 { (byte & 0xF) as usize } else { ((byte >> 4) & 0xF) as usize }
                            };

                            if colour_index == 0 { continue; } // transparent

                            let pal_idx = (pal_base + colour_index) * 2;
                            if pal_idx + 1 >= palette.len() { continue; }
                            let colour = palette[pal_idx] as u16 | ((palette[pal_idx + 1] as u16) << 8);
                            let r = ((colour & 0x1F) as u8) << 3;
                            let g = (((colour >> 5) & 0x1F) as u8) << 3;
                            let b = (((colour >> 10) & 0x1F) as u8) << 3;

                            let fb_idx = ((tile_y * 8 + py) * 240 + tile_x * 8 + px) * 3;
                            self.framebuffer[fb_idx] = r;
                            self.framebuffer[fb_idx + 1] = g;
                            self.framebuffer[fb_idx + 2] = b;
                        }
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
