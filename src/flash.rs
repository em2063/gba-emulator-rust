#[derive(PartialEq, Eq)]
enum FlashHandshake {
    Idle,
    GotAA,
    GotAA55,
}

enum FlashCommand {
    None,
    Program,
    BankSwitch,
}

pub struct FlashState {
    buffer: [u8; 0x20000],
    handshake: FlashHandshake,
    in_id_mode: bool,
    command: FlashCommand,
    current_bank: usize,
}

impl FlashState {
    pub fn new() -> FlashState {
        FlashState {
            buffer: [0xFF; 0x20000],
            handshake: FlashHandshake::Idle,
            in_id_mode: false,
            command: FlashCommand::None,
            current_bank: 0,
        }
    }

    pub fn read(&self, addr: u32) -> u8 {
        let off = (addr & 0xFFFF) as usize;
        let val = if self.in_id_mode {
            match off {
                0x0000 => 0x62, //Sanyo
                0x0001 => 0x13, //128k device
                _ => 0xFF,
            }
        } else {
            self.buffer[self.current_bank * 0x10000 + off]
        };
        val
    }

    pub fn write(&mut self, addr: u32, value: u8) {
        let addr = addr & 0xFFFF;
        use FlashHandshake::*;
        match (&self.handshake, addr, value) {
            (Idle, 0x5555, 0xAA) => self.handshake = GotAA,
            (GotAA, 0x2AAA, 0x55) => self.handshake = GotAA55,
            (GotAA55, 0x5555, 0x90) => {
                self.in_id_mode = true;
                self.handshake = Idle;
            }
            (GotAA55, 0x5555, 0xF0) => {
                self.in_id_mode = false;
                self.handshake = Idle;
            }
            (GotAA55, 0x5555, 0xA0) => {
                self.command = FlashCommand::Program;
                self.handshake = Idle;
            }
            (GotAA55, 0x5555, 0xB0) => {
                self.command = FlashCommand::BankSwitch;
                self.handshake = Idle;
            }
            _ => {
                match self.command {
                    FlashCommand::Program => {
                        self.buffer[self.current_bank * 0x10000 + addr as usize] = value;
                    }
                    FlashCommand::BankSwitch => {
                        if addr == 0x0000 && value < 2 {
                            self.current_bank = value as usize;
                        }
                    }
                    FlashCommand::None => {}
                }
                self.command = FlashCommand::None;
                self.handshake = Idle;
            }
        }
    }
}
