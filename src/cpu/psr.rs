impl CPU {
    fn execute_psr(&mut self, instruction: u32) {
        let i = (instruction >> 25) & 1; //immediate op flag
        let source_dest = (instruction >> 22) & 1; //Source/Destination PSR  (0=CPSR, 1=SPSR_<current mode>)
        let opcode = (instruction >> 21) & 1; //opcode: 1 = MSR: ;Psr[field] = Op, 0 = MRS: ;Rd = Psr

        if opcode == 1 {
            //MSR - write to PSR
            let value = if i == 1 {
                let ror = (instruction >> 8) & 0xF;
                (instruction & 0xFF).rotate_right(ror * 2)
            } else {
                self.registers[(instruction & 0xF) as usize]
            };

            if source_dest == 0 {
                if (instruction >> 19) & 1 == 1 {
                    self.cpsr = (self.cpsr & 0x00FFFFFF) | (value & 0xFF000000);
                }
                if (instruction >> 16) & 1 == 1 {
                    let new_mode = value & 0x1F;
                    let old_mode = self.cpsr & 0x1F;
                    if new_mode != old_mode {
                        self.switch_mode(new_mode); //swaps banked regs, updates mode bits in CPSR
                    }
                    self.cpsr = (self.cpsr & !0xE0) | (value & 0xE0);
                }
            } else {
                let mut spsr = self.get_spsr();
                if (instruction >> 19) & 1 == 1 {
                    spsr = (spsr & 0x00FFFFFF) | (value & 0xFF000000);
                }
                if (instruction >> 16) & 1 == 1 {
                    spsr = (spsr & 0xFFFFFF00) | (value & 0x000000FF);
                }
                self.set_spsr(spsr);
            }
        } else {
            //MRS - read from psr to register
            let rd = (instruction >> 12) & 0xF;
            self.registers[rd as usize] = if source_dest == 0 {
                self.cpsr
            } else {
                self.get_spsr()
            };
        }
    }

    fn get_spsr(&self) -> u32 {
        match self.cpsr & 0x1F {
            0b10011 => self.spsr_svc,
            0b10010 => self.spsr_irq,
            0b10001 => self.spsr_fiq,
            0b10111 => self.spsr_abt,
            0b11011 => self.spsr_und,
            _ => self.cpsr,
        }
    }

    fn set_spsr(&mut self, value: u32) {
        match self.cpsr & 0x1F {
            0b10011 => self.spsr_svc = value,
            0b10010 => self.spsr_irq = value,
            0b10001 => self.spsr_fiq = value,
            0b10111 => self.spsr_abt = value,
            0b11011 => self.spsr_und = value,
            _ => {} // User/System mode
        }
    }
}
