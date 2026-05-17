use crate::cpu::CPU;
use crate::memory_bus::MemoryBus;

//Disable STAT IRQs until the IRQ dispatch chain (BIOS VBlankIntrWait) is proven stable.
//Flip to true once the CPU's IRQ return path is fully debugged.
const STAT_IRQS_ENABLED: bool = false;

pub enum PpuMode {
    Oam,
    Drawing,
    HBlank,
    Vblank,
}

pub struct PPU {
    pub framebuffer: [u8; 240 * 160 * 3], //240 x 160 GBA display (RGB pixels)
    pub vcount: u16,
    pub line_cycle: u16,
    pub mode: PpuMode,

    //per-scanline scratch buffers used by the compositor.
    //colors are kept as raw 15-bit GBA values; final RGB888 expansion happens once at the end.
    bg_color: [[u16; 240]; 4],
    bg_opaque: [[bool; 240]; 4],

    //one OBJ pixel per x. priority=4 means "no sprite landed here".
    obj_color: [u16; 240],
    obj_priority: [u8; 240],
    obj_semi: [bool; 240],   //attr0 gfx-mode = 1 (semi-transparent)
    obj_window: [bool; 240], //pixel covered by an obj-mode=2 sprite

    //6-bit per-pixel enable mask: bits 0..3 = BG0..BG3, bit 4 = OBJ, bit 5 = FX
    win_mask: [u8; 240],
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            framebuffer: [0; 240 * 160 * 3],
            vcount: 0,
            line_cycle: 0,
            mode: PpuMode::Oam,
            bg_color: [[0; 240]; 4],
            bg_opaque: [[false; 240]; 4],
            obj_color: [0; 240],
            obj_priority: [4; 240],
            obj_semi: [false; 240],
            obj_window: [false; 240],
            win_mask: [0x3F; 240],
        }
    }

    //Helper functions to help tick the PPU along in time
    //
    //
    //
    //
    pub fn tick(&mut self, cpu: &mut CPU, bus: &mut MemoryBus) {
        self.line_cycle += 1;
        if self.line_cycle == 960 {
            self.enter_hblank(cpu, bus);
        }
        if self.line_cycle == 1232 {
            self.end_scanline(cpu, bus);
            self.line_cycle = 0;
        }
    }

    //Sets the matching IF bit and fires CPU IRQ if IE/IME/CPSR allow.
    //bit: 0=VBlank, 1=HBlank, 2=VCount
    fn fire_irq(&self, cpu: &mut CPU, bus: &mut MemoryBus, bit: u16) {
        let if_val = bus.read_u16(0x04000202) as u32;
        bus.write_u16(0x04000202, if_val | (1u32 << bit));

        let ie = bus.read_u16(0x04000200);
        let ime = bus.read_u16(0x04000208);
        let cpsr_irq_disabled = (cpu.cpsr >> 7) & 1 == 1;
        if !cpsr_irq_disabled && ime & 1 == 1 && (ie >> bit) & 1 == 1 {
            cpu.trigger_irq(bus);
        }
    }

    fn enter_hblank(&mut self, cpu: &mut CPU, bus: &mut MemoryBus) {
        bus.io[4] |= 0x02; //set DISPSTAT hblank bit
        bus.trigger_hblank_dma(); //hblank dma triggered every visible line

        self.mode = PpuMode::HBlank;

        //STAT HBlank IRQ — gated until IRQ dispatch chain is stable
        if STAT_IRQS_ENABLED {
            let dispstat = bus.read_u16(0x04000004);
            if (dispstat >> 4) & 1 == 1 {
                self.fire_irq(cpu, bus, 1);
            }
        }
    }

    fn end_scanline(&mut self, cpu: &mut CPU, bus: &mut MemoryBus) {
        bus.io[4] &= !0x02;
        let dispcnt = bus.read_u16(0x04000000);

        if self.vcount < 160 {
            self.render_current_line(bus, dispcnt);
        }

        self.vcount = (self.vcount + 1) % 228;
        self.update_vcount_register(bus);

        if self.vcount == 160 {
            bus.io[4] |= 0x01;
            self.mode = PpuMode::Vblank;
            bus.trigger_vblank_dma();

            //Set bit 0 of BIOS interrupt flag buffer so IntrWait/VBlankIntrWait unblocks.
            //The BIOS spin-loop reads [0x03007FF8] — no CPU IRQ needed for this to work.
            let intrflag = bus.read_u16(0x03007FF8);
            bus.write_u16(0x03007FF8, (intrflag | 1) as u32);

            //STAT VBlank IRQ — gated until IRQ dispatch chain is stable
            if STAT_IRQS_ENABLED {
                let dispstat = bus.read_u16(0x04000004);
                if (dispstat >> 3) & 1 == 1 {
                    self.fire_irq(cpu, bus, 0);
                }
            }
        }

        if self.vcount == 227 {
            bus.io[4] &= !0x01;
            self.mode = PpuMode::Oam;
        }

        self.update_vcount_match(cpu, bus);
    }

    fn update_vcount_register(&self, bus: &mut MemoryBus) {
        bus.io[6] = (self.vcount & 0xFF) as u8;
        bus.io[7] = (self.vcount >> 8) as u8;
    }

    fn render_current_line(&mut self, bus: &mut MemoryBus, dispcnt: u16) {
        let line = self.vcount as usize;
        self.clear_line_buffers();

        //fill the per-BG line buffers based on current BG mode
        let mode = dispcnt & 0b111;
        match mode {
            0 => self.render_mode0(line, &bus.vram, &bus.io, &bus.palette),
            3 => self.render_mode3(line, &bus.vram, &bus.io),
            4 => self.render_mode4(line, &bus.vram, &bus.palette, &bus.io),
            _ => {}
        }
        //fill the OBJ line buffer (and obj_window flags)
        self.render_sprites(&bus.oam, &bus.palette, &bus.vram, dispcnt, line);

        //resolve window regions then composite top→bottom with blending
        self.build_window_mask(line, &bus.io);
        self.composite_scanline(line, &bus.io, &bus.palette);
    }

    fn clear_line_buffers(&mut self) {
        for bg in 0..4 {
            for x in 0..240 {
                self.bg_opaque[bg][x] = false;
            }
        }
        for x in 0..240 {
            self.obj_priority[x] = 4;
            self.obj_semi[x] = false;
            self.obj_window[x] = false;
            self.win_mask[x] = 0x3F;
        }
    }

    fn update_vcount_match(&mut self, cpu: &mut CPU, bus: &mut MemoryBus) {
        let compare_val = bus.io[5];
        if self.vcount as u8 == compare_val {
            bus.io[4] |= 0x04;
            //STAT VCount-match IRQ — gated until IRQ dispatch chain is stable
            if STAT_IRQS_ENABLED {
                let dispstat = bus.read_u16(0x04000004);
                if (dispstat >> 5) & 1 == 1 {
                    self.fire_irq(cpu, bus, 2);
                }
            }
        } else {
            bus.io[4] &= !0x04;
        }
    }
    //end of helper functions
    //
    //
    //
    //

    //main functions for rendering
    //mode 0-5 using scanline rendering
    pub fn render_mode0(
        &mut self,
        line: usize,
        vram: &[u8; 96 * 1024],
        io: &[u8; 0x400],
        palette: &[u8; 1024],
    ) {
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);
        for bg_num in 0..4usize {
            if (dispcnt >> (8 + bg_num)) & 1 == 0 {
                continue;
            }
            self.render_bg_text(bg_num, line, vram, io, palette);
        }
    }

    //fills bg_color[bg_num] / bg_opaque[bg_num] for one text-mode BG on this scanline
    fn render_bg_text(
        &mut self,
        bg_num: usize,
        line: usize,
        vram: &[u8; 96 * 1024],
        io: &[u8; 0x400],
        palette: &[u8; 1024],
    ) {
        let cnt_off = 8 + bg_num * 2;
        let bgcnt = io[cnt_off] as u16 | ((io[cnt_off + 1] as u16) << 8);
        let char_base = ((bgcnt >> 2) & 0b11) as usize * 16384;
        let is_8bpp = (bgcnt >> 7) & 1 == 1;
        let screen_base = ((bgcnt >> 8) & 0b11111) as usize * 2048;
        let screen_size = (bgcnt >> 14) & 0b11; //bits 14-15 of BGCNT

        let hofs = io[0x10 + bg_num * 4] as u16 | ((io[0x11 + bg_num * 4] as u16) << 8);
        let vofs = io[0x12 + bg_num * 4] as u16 | ((io[0x13 + bg_num * 4] as u16) << 8);
        let hofs = (hofs & 0x1FF) as usize;
        let vofs = (vofs & 0x1FF) as usize;

        for screen_x in 0..240usize {
            //map position accounts for scroll
            let map_x = (screen_x + hofs) % 512;
            let map_y = (line + vofs) % 512;

            //which tile in the map
            let tile_x = map_x / 8;
            let tile_y = map_y / 8;

            //pixels within tile
            let px = map_x % 8;
            let py = map_y % 8;

            let (tx, ty) = (tile_x % 32, tile_y % 32);
            let block_x = tile_x / 32;
            let block_y = tile_y / 32;
            let block_offset = match screen_size {
                0 => 0,                     //256x256 — single block
                1 => block_x,               //512x256 — 2 blocks wide
                2 => block_y,               //256x512 — 2 blocks tall
                3 => block_y * 2 + block_x, //512x512 — 4 blocks
                _ => 0,
            };
            let entry_idx = ty * 32 + tx;
            let e0 = screen_base + block_offset * 2048 + entry_idx * 2;
            if e0 + 1 >= vram.len() {
                continue;
            }
            let entry = vram[e0] as u16 | ((vram[e0 + 1] as u16) << 8);

            let tile_num = (entry & 0x3FF) as usize;
            let flip_h = (entry >> 10) & 1 == 1;
            let flip_v = (entry >> 11) & 1 == 1;
            let pal_base = if is_8bpp {
                0
            } else {
                ((entry >> 12) & 0xF) as usize * 16
            };

            let sy = if flip_v { 7 - py } else { py };
            let sx = if flip_h { 7 - px } else { px };

            let colour_index = if is_8bpp {
                let off = char_base + tile_num * 64 + sy * 8 + sx;
                if off >= vram.len() {
                    continue;
                }
                vram[off] as usize
            } else {
                let off = char_base + tile_num * 32 + sy * 4 + sx / 2;
                if off >= vram.len() {
                    continue;
                }
                let byte = vram[off];
                if sx % 2 == 0 {
                    (byte & 0xF) as usize
                } else {
                    ((byte >> 4) & 0xF) as usize
                }
            };

            if colour_index == 0 {
                continue; //transparent
            }

            let pal_idx = (pal_base + colour_index) * 2;
            if pal_idx + 1 >= palette.len() {
                continue;
            }
            let colour = palette[pal_idx] as u16 | ((palette[pal_idx + 1] as u16) << 8);
            self.bg_color[bg_num][screen_x] = colour;
            self.bg_opaque[bg_num][screen_x] = true;
        }
    }

    //Mode 3: BG2 is a single 240x160 RGB555 bitmap covering the first 0x12C00 bytes of VRAM
    pub fn render_mode3(&mut self, line: usize, vram: &[u8; 96 * 1024], io: &[u8; 0x400]) {
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);
        if (dispcnt >> 10) & 1 == 0 {
            return; //BG2 disabled, backdrop will show through
        }
        for x in 0..240usize {
            let off = (line * 240 + x) * 2; //2 byte pixels
            let colour = vram[off] as u16 | ((vram[off + 1] as u16) << 8);
            self.bg_color[2][x] = colour;
            self.bg_opaque[2][x] = true;
        }
    }

    //Mode 4: 240x160 paletted bitmap, double-buffered (page flip on DISPCNT bit 4)
    pub fn render_mode4(
        &mut self,
        line: usize,
        vram: &[u8; 96 * 1024],
        palette: &[u8; 1024],
        io: &[u8; 0x400],
    ) {
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);
        if (dispcnt >> 10) & 1 == 0 {
            return; //BG2 disabled
        }
        let page_base = if (dispcnt >> 4) & 1 == 1 { 0xA000 } else { 0 };
        for x in 0..240usize {
            let index = vram[page_base + line * 240 + x] as usize;
            if index == 0 {
                continue; //index 0 is transparent in paletted bitmap mode
            }
            let colour = palette[index * 2] as u16 | ((palette[index * 2 + 1] as u16) << 8);
            self.bg_color[2][x] = colour;
            self.bg_opaque[2][x] = true;
        }
    }

    pub fn render_sprites(
        &mut self,
        oam: &[u8; 1024],
        palette_ram: &[u8; 1024],
        vram: &[u8; 96 * 1024],
        dispcnt: u16,
        line: usize,
    ) {
        if (dispcnt >> 12) & 1 == 0 {
            return; //sprites disabled
        }
        //iterate OAM ascending. For each pixel we only overwrite if the new sprite
        //has strictly better (lower) priority, so OAM index 0 wins ties.
        for i in 0..128usize {
            let base = i * 8;
            let attr0 = oam[base] as u16 | ((oam[base + 1] as u16) << 8);
            let attr1 = oam[base + 2] as u16 | ((oam[base + 3] as u16) << 8);
            let attr2 = oam[base + 4] as u16 | ((oam[base + 5] as u16) << 8);

            //attr0 bits 8-9: 00=normal, 01=affine, 10=disabled, 11=affine+double-size
            let rs_bits = (attr0 >> 8) & 0b11;
            if rs_bits == 0b10 {
                continue; //disabled
            }
            //TODO: affine sprites — skip for now rather than draw at the wrong size/position
            if rs_bits == 0b01 || rs_bits == 0b11 {
                continue;
            }

            //attr0 bits 10-11: gfx mode (0=normal, 1=semi-transparent, 2=obj-window, 3=prohibited)
            let gfx_mode = (attr0 >> 10) & 0b11;
            if gfx_mode == 0b11 {
                continue;
            }
            let is_objwin = gfx_mode == 0b10;
            let is_semi = gfx_mode == 0b01;

            let y_coordinate = attr0 & 0xFF; //0-255
            let x_coordinate = attr1 & 0x1FF; //0-511
            let colour_mode = (attr0 >> 13) & 1; //0=16/16, 1=256/1
            let shape = (attr0 >> 14) & 0b11; //(0=Square,1=Horizontal,2=Vertical,3=Prohibited)
            let size = (attr1 >> 14) & 0b11;

            let mapping_1d = (dispcnt >> 6) & 1 == 1;
            let tile_number = (attr2 & 0x3FF) as usize;
            let priority = ((attr2 >> 10) & 0b11) as u8;
            let palette_num = ((attr2 >> 12) & 0xF) as usize;
            let h_flip = (attr1 >> 12) & 1 == 1;
            let v_flip = (attr1 >> 13) & 1 == 1;

            let (width, height) = match (shape, size) {
                (0, 0) => (8, 8),
                (0, 1) => (16, 16),
                (0, 2) => (32, 32),
                (0, 3) => (64, 64),
                (1, 0) => (16, 8),
                (1, 1) => (32, 8),
                (1, 2) => (32, 16),
                (1, 3) => (64, 32),
                (2, 0) => (8, 16),
                (2, 1) => (8, 32),
                (2, 2) => (16, 32),
                (2, 3) => (32, 64),
                _ => continue,
            };

            //sprite top-edge screen Y (with Y wrap mod 256)
            let y_signed: i32 = if y_coordinate >= 160 {
                y_coordinate as i32 - 256
            } else {
                y_coordinate as i32
            };

            //does this sprite intersect the current scanline?
            let line_i = line as i32;
            if line_i < y_signed || line_i >= y_signed + height as i32 {
                continue;
            }
            let py = (line_i - y_signed) as usize;

            //sprite left-edge screen X (with X wrap)
            let x_signed =
                (x_coordinate as i32).wrapping_sub(if x_coordinate > 255 { 512 } else { 0 });

            for px in 0..width {
                let screen_x = x_signed + px as i32;
                if screen_x < 0 || screen_x >= 240 {
                    continue;
                }
                let sx_u = screen_x as usize;

                let tile_y = py / 8;
                let tile_x = px / 8;
                let pixel_x = if h_flip { 7 - (px % 8) } else { px % 8 };
                let pixel_y = if v_flip { 7 - (py % 8) } else { py % 8 };

                //tile_number indexes 32-byte slots in OBJ VRAM regardless of bpp.
                //Each 8x8 tile is 32 bytes in 4bpp, 64 bytes in 8bpp (= 2 slots).
                let tiles_wide = width / 8;
                let tile_size = if colour_mode == 1 { 64 } else { 32 };
                let stride = if mapping_1d {
                    tiles_wide
                } else if colour_mode == 1 {
                    16 //2D 8bpp: 32 slots wide / 2 slots per tile
                } else {
                    32 //2D 4bpp: 32 slots wide
                };
                let tile_base = 0x10000 + tile_number * 32 + (tile_y * stride + tile_x) * tile_size;

                let colour_index = if colour_mode == 1 {
                    vram[tile_base + pixel_y * 8 + pixel_x] as usize
                } else {
                    let byte = vram[tile_base + pixel_y * 4 + pixel_x / 2];
                    if pixel_x % 2 == 0 {
                        (byte & 0xF) as usize
                    } else {
                        ((byte >> 4) & 0xF) as usize
                    }
                };

                if colour_index == 0 {
                    continue; //transparent
                }

                if is_objwin {
                    //obj-window sprites only mark the window region — their pixels aren't drawn
                    self.obj_window[sx_u] = true;
                    continue;
                }

                //don't overwrite a pixel with a worse-or-equal priority sprite.
                //(equal-priority loses → lower OAM index wins ties, since we iterate ascending)
                if self.obj_priority[sx_u] <= priority {
                    continue;
                }

                let palette_offset = if colour_mode == 1 {
                    0x200 + colour_index * 2
                } else {
                    0x200 + palette_num * 32 + colour_index * 2
                };
                let colour = palette_ram[palette_offset] as u16
                    | ((palette_ram[palette_offset + 1] as u16) << 8);

                self.obj_color[sx_u] = colour;
                self.obj_priority[sx_u] = priority;
                self.obj_semi[sx_u] = is_semi;
            }
        }
    }

    //Builds the per-pixel 6-bit enable mask for this scanline.
    //Priority of regions: WIN0 > WIN1 > OBJ-WIN > outside.
    fn build_window_mask(&mut self, line: usize, io: &[u8; 0x400]) {
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);
        let win0_enable = (dispcnt >> 13) & 1 == 1;
        let win1_enable = (dispcnt >> 14) & 1 == 1;
        let objwin_enable = (dispcnt >> 15) & 1 == 1;

        if !win0_enable && !win1_enable && !objwin_enable {
            //no windows in use — everything visible everywhere
            for x in 0..240 {
                self.win_mask[x] = 0x3F;
            }
            return;
        }

        let winin = io[0x48] as u16 | ((io[0x49] as u16) << 8);
        let winout = io[0x4A] as u16 | ((io[0x4B] as u16) << 8);
        let win0_in = (winin & 0x3F) as u8;
        let win1_in = ((winin >> 8) & 0x3F) as u8;
        let outside = (winout & 0x3F) as u8;
        let objwin_in = ((winout >> 8) & 0x3F) as u8;

        //start everything as "outside"
        for x in 0..240 {
            self.win_mask[x] = outside;
        }

        //OBJ-window — lowest of the three explicit windows
        if objwin_enable {
            for x in 0..240 {
                if self.obj_window[x] {
                    self.win_mask[x] = objwin_in;
                }
            }
        }

        //WIN1
        if win1_enable && Self::win_y_in_range(io, 1, line) {
            let (x1, x2) = Self::win_x_range(io, 1);
            for x in 0..240u32 {
                if Self::win_x_in_range(x, x1, x2) {
                    self.win_mask[x as usize] = win1_in;
                }
            }
        }

        //WIN0 — highest priority window
        if win0_enable && Self::win_y_in_range(io, 0, line) {
            let (x1, x2) = Self::win_x_range(io, 0);
            for x in 0..240u32 {
                if Self::win_x_in_range(x, x1, x2) {
                    self.win_mask[x as usize] = win0_in;
                }
            }
        }
    }

    fn win_x_range(io: &[u8; 0x400], n: usize) -> (u32, u32) {
        //WIN0H @ 0x40, WIN1H @ 0x42. Bits 0-7 = X2 (right, exclusive), bits 8-15 = X1 (left, inclusive)
        let reg = io[0x40 + n * 2] as u16 | ((io[0x41 + n * 2] as u16) << 8);
        let x2 = (reg & 0xFF) as u32;
        let x1 = ((reg >> 8) & 0xFF) as u32;
        (x1, x2)
    }

    fn win_y_in_range(io: &[u8; 0x400], n: usize, line: usize) -> bool {
        let reg = io[0x44 + n * 2] as u16 | ((io[0x45 + n * 2] as u16) << 8);
        let y2 = (reg & 0xFF) as u32;
        let y1 = ((reg >> 8) & 0xFF) as u32;
        let l = line as u32;
        if y1 <= y2 {
            l >= y1 && l < y2
        } else {
            //wrap (Y1 > Y2 means "from Y1 to end of frame, then 0 to Y2")
            l >= y1 || l < y2
        }
    }

    fn win_x_in_range(x: u32, x1: u32, x2: u32) -> bool {
        if x1 <= x2 {
            x >= x1 && x < x2
        } else {
            x >= x1 || x < x2
        }
    }

    //Final compositor: walks priority order to find the top & second-from-top
    //visible layer at each pixel, then applies BLDCNT effects (or forced semi-trans OBJ blend).
    fn composite_scanline(&mut self, line: usize, io: &[u8; 0x400], palette: &[u8; 1024]) {
        let dispcnt = io[0] as u16 | ((io[1] as u16) << 8);

        //forced blank wins over everything — whole line is white
        if (dispcnt >> 7) & 1 == 1 {
            let base = line * 240 * 3;
            for i in 0..240usize {
                let fb_idx = base + i * 3;
                self.framebuffer[fb_idx] = 255;
                self.framebuffer[fb_idx + 1] = 255;
                self.framebuffer[fb_idx + 2] = 255;
            }
            return;
        }

        let bldcnt = io[0x50] as u16 | ((io[0x51] as u16) << 8);
        let bldalpha = io[0x52] as u16 | ((io[0x53] as u16) << 8);
        let bldy = io[0x54] as u16 | ((io[0x55] as u16) << 8);

        let blend_mode = ((bldcnt >> 6) & 0b11) as u8; //0=off, 1=alpha, 2=brighten, 3=darken
        let first_target = (bldcnt & 0x3F) as u8; //bits 0..5: BG0,1,2,3,OBJ,BD
        let second_target = ((bldcnt >> 8) & 0x3F) as u8;

        //EVA/EVB/EVY are 5-bit coefficients clamped to 16 (so /16 fixed-point)
        let eva = std::cmp::min((bldalpha & 0x1F) as u32, 16);
        let evb = std::cmp::min(((bldalpha >> 8) & 0x1F) as u32, 16);
        let evy = std::cmp::min((bldy & 0x1F) as u32, 16);

        let backdrop = palette[0] as u16 | ((palette[1] as u16) << 8);

        //BGCNT priority (bits 0-1) for each BG
        let bg_priority = [
            io[0x08] & 0b11,
            io[0x0A] & 0b11,
            io[0x0C] & 0b11,
            io[0x0E] & 0b11,
        ];
        //DISPCNT BG-enable bits
        let bg_dispcnt_enabled = [
            (dispcnt >> 8) & 1 == 1,
            (dispcnt >> 9) & 1 == 1,
            (dispcnt >> 10) & 1 == 1,
            (dispcnt >> 11) & 1 == 1,
        ];

        let base = line * 240 * 3;
        for x in 0..240usize {
            let win = self.win_mask[x];
            let win_bg = win & 0x0F;
            let win_obj = (win >> 4) & 1 == 1;
            let win_fx = (win >> 5) & 1 == 1;

            //find top and second visible layers.
            //layer ids: 0..3 = BG0..BG3, 4 = OBJ, 5 = backdrop
            let mut top_layer: u8 = 5;
            let mut top_color: u16 = backdrop;
            let mut top_is_semi = false;
            let mut second_layer: u8 = 5;
            let mut second_color: u16 = backdrop;
            let mut found_top = false;

            //GBA layer order: OBJ at priority p sits above BGs at priority p,
            //within same priority BG0 > BG1 > BG2 > BG3.
            'priority: for p in 0..4u8 {
                //OBJ first at this priority level
                if win_obj && self.obj_priority[x] == p {
                    if !found_top {
                        top_layer = 4;
                        top_color = self.obj_color[x];
                        top_is_semi = self.obj_semi[x];
                        found_top = true;
                    } else {
                        second_layer = 4;
                        second_color = self.obj_color[x];
                        break 'priority;
                    }
                }
                //then BG0..BG3 at this priority level
                for bg in 0..4u8 {
                    let bi = bg as usize;
                    if !bg_dispcnt_enabled[bi] {
                        continue;
                    }
                    if (win_bg >> bg) & 1 == 0 {
                        continue;
                    }
                    if bg_priority[bi] != p {
                        continue;
                    }
                    if !self.bg_opaque[bi][x] {
                        continue;
                    }
                    if !found_top {
                        top_layer = bg;
                        top_color = self.bg_color[bi][x];
                        found_top = true;
                    } else {
                        second_layer = bg;
                        second_color = self.bg_color[bi][x];
                        break 'priority;
                    }
                }
            }

            //decide blend
            let final_color = if top_is_semi && (second_target >> second_layer) & 1 == 1 && win_fx {
                //semi-transparent OBJ on top: forced alpha-blend when 2nd layer is a BLDCNT 2nd target
                alpha_blend(top_color, second_color, eva, evb)
            } else if win_fx
                && blend_mode == 1
                && (first_target >> top_layer) & 1 == 1
                && (second_target >> second_layer) & 1 == 1
            {
                alpha_blend(top_color, second_color, eva, evb)
            } else if win_fx && blend_mode == 2 && (first_target >> top_layer) & 1 == 1 {
                brighten(top_color, evy)
            } else if win_fx && blend_mode == 3 && (first_target >> top_layer) & 1 == 1 {
                darken(top_color, evy)
            } else {
                top_color
            };

            let r = ((final_color & 0x1F) as u8) << 3;
            let g = (((final_color >> 5) & 0x1F) as u8) << 3;
            let b = (((final_color >> 10) & 0x1F) as u8) << 3;
            let fb_idx = base + x * 3;
            self.framebuffer[fb_idx] = r;
            self.framebuffer[fb_idx + 1] = g;
            self.framebuffer[fb_idx + 2] = b;
        }
    }
}

//Blend math kept as free functions so the compositor body stays readable.
fn alpha_blend(top: u16, bot: u16, eva: u32, evb: u32) -> u16 {
    let r1 = (top & 0x1F) as u32;
    let g1 = ((top >> 5) & 0x1F) as u32;
    let b1 = ((top >> 10) & 0x1F) as u32;
    let r2 = (bot & 0x1F) as u32;
    let g2 = ((bot >> 5) & 0x1F) as u32;
    let b2 = ((bot >> 10) & 0x1F) as u32;
    let r = std::cmp::min((r1 * eva + r2 * evb) >> 4, 31);
    let g = std::cmp::min((g1 * eva + g2 * evb) >> 4, 31);
    let b = std::cmp::min((b1 * eva + b2 * evb) >> 4, 31);
    (r | (g << 5) | (b << 10)) as u16
}

fn brighten(c: u16, evy: u32) -> u16 {
    let r = (c & 0x1F) as u32;
    let g = ((c >> 5) & 0x1F) as u32;
    let b = ((c >> 10) & 0x1F) as u32;
    let r = r + (((31 - r) * evy) >> 4);
    let g = g + (((31 - g) * evy) >> 4);
    let b = b + (((31 - b) * evy) >> 4);
    (r | (g << 5) | (b << 10)) as u16
}

fn darken(c: u16, evy: u32) -> u16 {
    let r = (c & 0x1F) as u32;
    let g = ((c >> 5) & 0x1F) as u32;
    let b = ((c >> 10) & 0x1F) as u32;
    let r = r - ((r * evy) >> 4);
    let g = g - ((g * evy) >> 4);
    let b = b - ((b * evy) >> 4);
    (r | (g << 5) | (b << 10)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mode4_render() {
        let mut ppu = PPU::new();
        let rom: Vec<u8> = std::fs::read("hello.gba").unwrap();
        let mut bus = MemoryBus::new(rom);

        //DISPCNT: mode 4 (bits 0-2 = 4) + BG2 enable (bit 10)
        bus.io[0] = 0x04;
        bus.io[1] = 0x04;

        for i in 0..(96 * 1024) {
            bus.vram[i] = 1;
        }
        bus.palette[2] = 0x1F;
        bus.palette[3] = 0x00;

        ppu.vcount = 0;
        let dispcnt = bus.read_u16(0x04000000);
        ppu.render_current_line(&mut bus, dispcnt);

        assert_eq!(ppu.framebuffer[0], 248)
    }
}
