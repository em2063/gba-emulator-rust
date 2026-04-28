use std::string::FromUtf16Error;

pub struct MemoryBus {
    rom: Vec<u8>,
    bios: [u8; 16 * 1024],   //system rom
    ewram: [u8; 256 * 1024], //on-board work RAM
    iwram: [u8; 32 * 1024],  //on-chip work RAM
    io: [u8; 0x400],         //input/output - 0 for now
    pallete: [u8; 1024],     //pallete
    vram: [u8; 96 * 1024],   //virtual ram
    oam: [u8; 1024],         //object ram
    sram: [u8; 64 * 1024],
    vblank_toggle: bool,
}

impl MemoryBus {
    pub fn new(rom: Vec<u8>) -> MemoryBus {
        MemoryBus {
            rom,
            bios: [0; 16 * 1024],   //system rom
            ewram: [0; 256 * 1024], //on-board work RAM
            iwram: [0; 32 * 1024],  //on-chip work RAM
            io: [0; 0x400],         //input/output - 0 for now
            pallete: [0; 1024],     //pallete
            vram: [0; 96 * 1024],   //virtual ram
            oam: [0; 1024],         //object ram
            sram: [0; 64 * 1024],
            vblank_toggle: false,
        }
    }

    pub fn read_u8(&mut self, addr: u32) -> u8 {
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr as usize],
            0x02000000..=0x0203FFFF => self.ewram[(addr - 0x02000000) as usize],
            0x03000000..=0x03007FFF => self.iwram[(addr - 0x03000000) as usize],
            0x04000000..=0x040003FE => {
                if addr == 0x04000004 {
                    self.vblank_toggle = !self.vblank_toggle;
                    return if self.vblank_toggle { 1 } else { 0 };
                }
                self.io[(addr - 0x04000000) as usize]
            }
            0x05000000..=0x050003FF => self.pallete[(addr - 0x05000000) as usize],
            0x06000000..=0x06017FFF => self.vram[(addr - 0x06000000) as usize],
            0x07000000..=0x070003FF => self.oam[(addr - 0x07000000) as usize],
            0x0E000000..=0x0E00FFFF => self.sram[(addr - 0x0E000000) as usize],
            0x08000000..=0x09FFFFFF => self.rom[(addr - 0x08000000) as usize],
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
        self.write_u8(addr, (value & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(2), ((value >> 16) & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(3), ((value >> 24) & 0xFF) as u8);
    }

    pub fn write_u8(&mut self, addr: u32, value: u8) {
        match addr {
            0x00000000..=0x00003FFF => self.bios[addr as usize] = value,
            0x02000000..=0x0203FFFF => self.ewram[(addr - 0x02000000) as usize] = value,
            0x03000000..=0x03007FFF => self.iwram[(addr - 0x03000000) as usize] = value,
            0x04000000..=0x040003FE => self.io[(addr - 0x04000000) as usize] = value,
            0x05000000..=0x050003FF => self.pallete[(addr - 0x05000000) as usize] = value,
            0x06000000..=0x06017FFF => self.vram[(addr - 0x06000000) as usize] = value,
            0x07000000..=0x070003FF => self.oam[(addr - 0x07000000) as usize] = value,
            0x0E000000..=0x0E00FFFF => self.sram[(addr - 0x0E000000) as usize] = value,
            _ => {}
        };
    }

    pub fn read_u16(&mut self, addr: u32) -> u16 {
        let b0 = self.read_u8(addr) as u16;
        let b1 = self.read_u8(addr.wrapping_add(1)) as u16;
        b0 | (b1 << 8)
    }

    pub fn write_u16(&mut self, addr: u32, value: u32) {
        self.write_u8(addr, (value & 0xFF) as u8);
        self.write_u8(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8);
    }
}
