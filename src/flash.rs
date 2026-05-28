#[derive(PartialEq, Eq)]
enum FlashHandshake {
    Idle,
    GotAA,
    GotAA55,
}

pub struct FlashState {
    buffer: [u8; 0x20000],
    handshake: FlashHandshake,
    in_id_mode: bool,
}

impl FlashState {
    pub fn new() -> FlashState {
        FlashState {
            buffer: [0xFF; 0x20000],
            handshake: FlashHandshake::Idle,
            in_id_mode: false,
        }
    }

    pub fn read(&self, addr: u32) -> u8 {
        let off = (addr & 0xFFFF) as usize;
        if self.in_id_mode {
            match off {
                0x0000 => 0xC2, //Macronix
                0x0001 => 0x09, //128k device
                _ => 0xFF,
            }
        } else {
            self.buffer[off]
        }
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        let addr = addr & 0xFFFF;
        if self.handshake == FlashHandshake::Idle && addr == 0x5555 && value == 0xAA {
            self.handshake = FlashHandshake::GotAA;
        } else if self.handshake == FlashHandshake::GotAA && addr == 0x2AAA && value == 0x55 {
            self.handshake = FlashHandshake::GotAA55;
        } else if self.handshake == FlashHandshake::GotAA55 && addr == 0x5555 && value == 0x90 {
            self.in_id_mode = true;
            self.handshake = FlashHandshake::Idle;
        } else if self.handshake == FlashHandshake::GotAA55 && addr == 0x5555 && value == 0xF0 {
            self.in_id_mode = false;
            self.handshake = FlashHandshake::Idle;
        } else {
            self.handshake = FlashHandshake::Idle;
        }
    }
}
