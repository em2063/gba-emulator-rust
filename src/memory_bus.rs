pub struct MemoryBus {
    rom: Vec<u8>,
    pub bios: [u8; 16 * 1024], //system rom
    ewram: [u8; 256 * 1024],   //on-board work RAM
    iwram: [u8; 32 * 1024],    //on-chip work RAM
    pub io: [u8; 0x400],       //input/output - 0 for now
    pub palette: [u8; 1024],   //pallete
    pub vram: [u8; 96 * 1024], //virtual ram
    pub oam: [u8; 1024],       //object ram
    sram: [u8; 64 * 1024],
}

impl MemoryBus {
    pub fn new(rom: Vec<u8>) -> MemoryBus {
        MemoryBus {
            rom,
            bios: [0; 16 * 1024],   //system rom
            ewram: [0; 256 * 1024], //on-board work RAM
            iwram: [0; 32 * 1024],  //on-chip work RAM
            io: [0; 0x400],         //input/output - 0 for now
            palette: [0; 1024],     //pallete
            vram: [0; 96 * 1024],   //virtual ram
            oam: [0; 1024],         //object ram
            sram: [0; 64 * 1024],
        }
    }

    pub fn read_u8(&mut self, addr: u32) -> u8 {
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr as usize],
            0x02000000..=0x02FFFFFF => self.ewram[((addr - 0x02000000) & 0x3FFFF) as usize],
            0x03000000..=0x03FFFFFF => self.iwram[((addr - 0x03000000) & 0x7FFF) as usize],
            0x04000000..=0x040003FE => {
                if addr == 0x04000004 {
                    //DISPSTAT low byte: bit 0 = VBlank (VCOUNT >= 160)
                    let vcount = self.io[6] as u16;
                    let vblank = if vcount >= 160 { 1u8 } else { 0u8 };
                    return vblank;
                }
                self.io[(addr - 0x04000000) as usize]
            }
            0x05000000..=0x05FFFFFF => self.palette[((addr - 0x05000000) & 0x3FF) as usize],
            0x06000000..=0x06FFFFFF => self.vram[((addr - 0x06000000) & 0x17FFF) as usize],
            0x07000000..=0x07FFFFFF => self.oam[((addr - 0x07000000) & 0x3FF) as usize],
            0x0E000000..=0x0E00FFFF => self.sram[(addr - 0x0E000000) as usize],
            0x08000000..=0x09FFFFFF => {
                let offset = (addr - 0x08000000) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            0x0A000000..=0x0BFFFFFF => {
                let offset = (addr - 0x0A000000) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            0x0C000000..=0x0DFFFFFF => {
                let offset = (addr - 0x0C000000) as usize;
                if offset < self.rom.len() {
                    self.rom[offset]
                } else {
                    0xFF
                }
            }
            _ => 0xFF,
        }
    }

    pub fn read_u32(&mut self, addr: u32) -> u32 {
        let b0 = self.read_u8(addr) as u32;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u32;
        let b2 = self.read_u8(addr.wrapping_add(2)) as u32;
        let b3 = self.read_u8(addr.wrapping_add(3)) as u32;
        b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)
    }

    pub fn write_u32(&mut self, addr: u32, value: u32) {
        self.write_u8_internal(addr, (value & 0xFF) as u8);
        self.write_u8_internal(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8);
        self.write_u8_internal(addr.wrapping_add(2), ((value >> 16) & 0xFF) as u8);
        self.write_u8_internal(addr.wrapping_add(3), ((value >> 24) & 0xFF) as u8);

        //STR to a DMA channel's CNT register (count + control, 32-bit).
        //the high half-word of value is the control word.
        match addr {
            0x040000B8 => self.check_dma(0, value >> 16),
            0x040000C4 => self.check_dma(1, value >> 16),
            0x040000D0 => self.check_dma(2, value >> 16),
            0x040000DC => self.check_dma(3, value >> 16),
            _ => {}
        }
    }

    pub fn write_u8(&mut self, addr: u32, value: u8) {
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr as usize] = value,
            0x02000000..=0x02FFFFFF => self.ewram[((addr - 0x02000000) & 0x3FFFF) as usize] = value,
            0x03000000..=0x03FFFFFF => self.iwram[((addr - 0x03000000) & 0x7FFF) as usize] = value,
            0x04000000..=0x040003FE => self.io[(addr - 0x04000000) as usize] = value,
            0x05000000..=0x05FFFFFF => {
                let offset = ((addr - 0x05000000) & 0x3FF & !1) as usize;
                self.palette[offset] = value;
                self.palette[offset + 1] = value;
            }
            0x06010000..=0x06017FFF => {}
            0x06000000..=0x0600FFFF => {
                //byte store writes to both bytes of the halfword
                let offset = ((addr - 0x06000000) & !1) as usize;
                self.vram[offset] = value;
                self.vram[offset + 1] = value;
            }
            0x0E000000..=0x0E00FFFF => self.sram[(addr - 0x0E000000) as usize] = value,
            0x07000000..=0x07FFFFFF => {}
            _ => self.write_u8_internal(addr, value),
        };
    }

    fn write_u8_internal(&mut self, addr: u32, value: u8) {
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr as usize] = value,
            0x02000000..=0x02FFFFFF => self.ewram[((addr - 0x02000000) & 0x3FFFF) as usize] = value,
            0x03000000..=0x03FFFFFF => self.iwram[((addr - 0x03000000) & 0x7FFF) as usize] = value,
            0x04000000..=0x040003FE => self.io[(addr - 0x04000000) as usize] = value,
            0x05000000..=0x050003FF => self.palette[(addr - 0x05000000) as usize] = value,
            0x06000000..=0x06017FFF => self.vram[(addr - 0x06000000) as usize] = value,
            0x0E000000..=0x0E00FFFF => self.sram[(addr - 0x0E000000) as usize] = value,
            0x07000000..=0x07FFFFFF => self.oam[((addr - 0x07000000) & 0x3FF) as usize] = value,
            _ => {}
        }
    }

    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let b0 = self.read_u8(addr) as u16;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    pub fn write_u16(&mut self, addr: u32, value: u32) {
        self.write_u8_internal(addr, (value & 0xFF) as u8);
        self.write_u8_internal(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8);

        //check if it is a direct memory access register
        match addr {
            0x040000BA => self.check_dma(0, value), //DMA0 control
            0x040000C6 => self.check_dma(1, value), //DMA1 control
            0x040000D2 => self.check_dma(2, value), //DMA2 control
            0x040000DE => self.check_dma(3, value), //DMA3 control
            _ => {}
        }
    }

    pub fn check_dma(&mut self, channel: usize, control: u32) {
        if control & (1 << 15) == 0 {
            return; //enable bit not set
        }

        //only run immediate-mode DMA here. VBlank/HBlank/Special start timings
        //need to be deferred until the actual trigger event (TODO).
        let start_timing = (control >> 12) & 0b11;
        if start_timing != 0 {
            return;
        }

        let base = 0x040000B0 + channel * 0xC;
        let src = self.read_u32(base as u32);
        let dst = self.read_u32(base as u32 + 4);
        let count = self.read_u16(base as u32 + 8) as u32;
        let count = if count == 0 {
            if channel == 3 { 0x10000 } else { 0x4000 }
        } else {
            count
        };

        let is_32bit = (control >> 10) & 1 == 1;
        let dst_ctrl = (control >> 5) & 0b11;
        let src_ctrl = (control >> 7) & 0b11;

        let unit = if is_32bit { 4u32 } else { 2u32 };

        let mut src_addr = src;
        let mut dst_addr = dst;
        for _ in 0..count {
            if is_32bit {
                let value = self.read_u32(src_addr);
                self.write_u32(dst_addr, value);
            } else {
                let value = self.read_u16(src_addr) as u32;
                self.write_u16(dst_addr, value);
            }

            src_addr = match src_ctrl {
                0b00 => src_addr.wrapping_add(unit),
                0b01 => src_addr.wrapping_sub(unit),
                _ => src_addr, //fixed
            };

            dst_addr = match dst_ctrl {
                0b00 | 0b11 => dst_addr.wrapping_add(unit),
                0b01 => dst_addr.wrapping_sub(unit),
                _ => dst_addr, //fixed
            };
        }

        let ctrl_io_addr = (0x040000BA + channel * 0xC) - 0x04000000;
        self.io[ctrl_io_addr + 1] &= !(1 << 7);
    }
}
