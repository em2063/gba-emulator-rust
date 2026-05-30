impl CPU {
    //switches between banked registers and system registers, used for exceptions and SWI
    pub fn switch_mode(&mut self, new_mode: u32) {
        match self.cpsr & 0x1F {
            //user
            0b11111 | 0b10000 => {
                self.r13_usr = self.registers[13];
                self.r14_usr = self.registers[14];
            }
            //supervisor
            0b10011 => {
                self.r13_svc = self.registers[13];
                self.r14_svc = self.registers[14];
            }
            //irq
            0b10010 => {
                self.r13_irq = self.registers[13];
                self.r14_irq = self.registers[14];
            }
            //abort
            0b10111 => {
                self.r13_abt = self.registers[13];
                self.r14_abt = self.registers[14];
            }
            //fiq
            0b10001 => {
                self.r13_fiq = self.registers[13];
                self.r14_fiq = self.registers[14];

                self.r8_fiq = self.registers[8];
                self.r9_fiq = self.registers[9];
                self.r10_fiq = self.registers[10];
                self.r11_fiq = self.registers[11];
                self.r12_fiq = self.registers[12];
            }
            //undefined
            0b11011 => {
                self.r13_und = self.registers[13];
                self.r14_und = self.registers[14];
            }
            _ => {}
        }

        match new_mode {
            //user
            0b11111 | 0b10000 => {
                self.registers[13] = self.r13_usr;
                self.registers[14] = self.r14_usr;
            }
            //supervisor
            0b10011 => {
                self.registers[13] = self.r13_svc;
                self.registers[14] = self.r14_svc;
            }
            0b10010 => {
                self.registers[13] = self.r13_irq;
                self.registers[14] = self.r14_irq;
            }
            0b10111 => {
                self.registers[13] = self.r13_abt;
                self.registers[14] = self.r14_abt;
            }
            0b10001 => {
                self.registers[13] = self.r13_fiq;
                self.registers[14] = self.r14_fiq;
                self.registers[8] = self.r8_fiq;
                self.registers[9] = self.r9_fiq;
                self.registers[10] = self.r10_fiq;
                self.registers[11] = self.r11_fiq;
                self.registers[12] = self.r12_fiq;
            }
            0b11011 => {
                self.registers[13] = self.r13_und;
                self.registers[14] = self.r14_und;
            }
            _ => {}
        }

        //update CPSR mode bits
        self.cpsr = (self.cpsr & !0x1F) | new_mode;
    }

    //ARM SWI: enter SVC mode and dispatch to the real BIOS handler at 0x00000008.
    //The BIOS reads the SWI instruction at LR_svc-4 to determine which call to run.
    pub fn execute_swi(&mut self, _bus: &mut MemoryBus, _instruction: u32) {
        let saved_cpsr = self.cpsr; //capture before switch_mode modifies cpsr
        self.switch_mode(0b10011);
        self.registers[14] = self.registers[15]; //registers[15] is already pc+4
        self.r14_svc = self.registers[15];
        self.spsr_svc = saved_cpsr;
        self.cpsr = (self.cpsr & !0x3F) | 0x13 | (1 << 7); //SVC mode, IRQ disabled
        self.cpsr &= !(1 << 5); //ARM mode
        self.registers[15] = 0x00000008;
    }

    pub fn trigger_undefined(&mut self) {
        let saved_cpsr = self.cpsr;
        self.switch_mode(0b11011); // UND mode
        self.registers[14] = self.registers[15]; // PC is already past the instruction
        self.r14_und = self.registers[14];
        self.spsr_und = saved_cpsr;
        self.cpsr = (self.cpsr & !0x3F) | 0x1B | (1 << 7); // UND mode, IRQ disabled, ARM
        self.cpsr &= !(1 << 5);
        self.registers[15] = 0x00000004; // undefined instruction vector
    }

    pub fn trigger_irq(&mut self, _bus: &mut MemoryBus) {
        // BIOS IRQ handler returns via `SUBS PC, LR, #4`. registers[15] is the next
        // instruction address (already incremented before dispatch), so LR must be
        // next_instr + 4 so that LR - 4 lands at next_instr.
        let return_addr = self.registers[15].wrapping_add(4);
        let saved_cpsr = self.cpsr;

        self.switch_mode(0b10010);

        //now in IRQ mode, set banked registers
        self.registers[14] = return_addr; //R14_irq
        self.r14_irq = return_addr;
        self.spsr_irq = saved_cpsr;

        self.cpsr = (self.cpsr & !0x3F) | 0x12 | (1 << 7);
        self.cpsr &= !(1 << 5);
        self.registers[15] = 0x00000018;
    }
}
