impl CPU {
    pub fn check_condition(&mut self, condition: u32) -> bool {
        let n = (self.cpsr >> 31) & 1 == 1;
        let z = (self.cpsr >> 30) & 1 == 1;
        let c = (self.cpsr >> 29) & 1 == 1;
        let v = (self.cpsr >> 28) & 1 == 1;

        match condition {
            0b0000 => z,
            0b0001 => !z,
            0b0010 => c,
            0b0011 => !c,
            0b0100 => n,
            0b0101 => !n,
            0b0110 => v,
            0b0111 => !v,
            0b1000 => c && !z,
            0b1001 => !c || z,
            0b1010 => n == v,
            0b1011 => n != v,
            0b1100 => !z && (n == v),
            0b1101 => z || (n != v),
            0b1110 => true,
            0b1111 => false,
            _ => unreachable!(),
        }
    }

    fn check_flags(&mut self, instruction: u32) -> bool {
        self.check_condition((instruction >> 28) & 0xF)
    }

    //executes instructions
    pub fn execute_instruction(&mut self, bus: &mut MemoryBus, instruction: u32) {
        if !self.check_flags(instruction) {
            return;
        }

        let bits_27_24 = (instruction >> 24) & 0xF;
        match bits_27_24 {
            0b1111 => {
                self.execute_swi(bus, instruction);
            }
            _ => {
                let bits_27_25 = (instruction >> 25) & 0b111;
                match bits_27_25 {
                    0b110 | 0b111 => {
                        self.trigger_undefined();
                    }
                    0b010 | 0b011 => self.execute_ldr_str(bus, instruction),
                    0b000 | 0b001 => {
                        if (instruction >> 8) & 0xFFFFF == 0b00010010111111111111 {
                            self.decode_branch_exchange(instruction);
                        } else if (instruction >> 25) & 1 == 0  // bit 25 MUST be 0 for halfword transfer
                            && (instruction >> 4) & 1 == 1
                            && (instruction >> 7) & 1 == 1
                            && (instruction >> 5) & 0b11 != 0
                        {
                            self.execute_halfword_transfer(bus, instruction);
                        } else {
                            if (instruction >> 23) & 0b11 == 0b10 && (instruction >> 20) & 1 == 0b0
                            {
                                self.execute_psr(instruction);
                            } else if ((instruction >> 4) & 0xF == 0b1001)
                                && (instruction >> 25) & 1 == 0
                            {
                                self.execute_mul_and_mul_acc_fullwords(instruction);
                            } else {
                                self.decode_alu(instruction);
                            }
                        }
                    }
                    0b101 => self.decode_branch(instruction),
                    0b100 => self.decode_memory(bus, instruction),
                    _ => self.halt("unimplemented ARM instruction group", instruction),
                }
            }
        }
    }

    pub fn halt(&self, context: &str, instruction: u32) -> ! {
        eprintln!("\n=== EMULATOR HALT ===");
        eprintln!("  reason : {}", context);
        eprintln!("  instr  : {:#010x}  ({:#034b})", instruction, instruction);
        eprintln!(
            "  PC     : {:#010x}  (before fetch: {:#010x})",
            self.registers[15],
            self.registers[15].wrapping_sub(4)
        );
        eprintln!(
            "  CPSR   : {:#010x}  mode={:#07b}  T={}  N={}  Z={}  C={}  V={}",
            self.cpsr,
            self.cpsr & 0x1F,
            (self.cpsr >> 5) & 1,
            (self.cpsr >> 31) & 1,
            (self.cpsr >> 30) & 1,
            (self.cpsr >> 29) & 1,
            (self.cpsr >> 28) & 1,
        );
        for row in 0..4 {
            eprint!("  ");
            for col in 0..4 {
                let i = row * 4 + col;
                eprint!("r{:<2}={:#010x}  ", i, self.registers[i]);
            }
            eprintln!();
        }
        eprintln!("=====================\n");
        panic!("halted");
    }
}
