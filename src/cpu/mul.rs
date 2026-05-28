impl CPU {
    fn execute_mul_and_mul_acc_fullwords(&mut self, instruction: u32) {
        let rd = (instruction >> 16) & 0xF; //dest register
        let rn = (instruction >> 12) & 0xF; //accumulate register
        let rs = (instruction >> 8) & 0xF; //operand register
        let rm = instruction & 0xF; //operand register 2?

        let rm_reg = self.registers[rm as usize];
        let rs_reg = self.registers[rs as usize];

        let opcode = (instruction >> 21) & 0xF;
        match opcode {
            //mul
            0b0000 => {
                let result = rm_reg.wrapping_mul(rs_reg);
                self.registers[rd as usize] = result;

                if (instruction >> 20) & 1 == 1 {
                    let c = (self.cpsr >> 29) & 1 == 1;
                    let v = (self.cpsr >> 28) & 1 == 1;
                    let n = (result >> 31) == 1;
                    let z = result == 0;
                    self.set_flags(n, z, c, v);
                }
            }
            //mla
            0b0001 => {
                let result = rm_reg.wrapping_mul(rs_reg).wrapping_add(rn);
                self.registers[rd as usize] = result;

                if (instruction >> 20) & 1 == 1 {
                    let c = (self.cpsr >> 29) & 1 == 1;
                    let v = (self.cpsr >> 28) & 1 == 1;
                    let n = (result >> 31) == 1;
                    let z = result == 0;
                    self.set_flags(n, z, c, v);
                }
            }
            //smlal
            0b0111 => {
                let rdlo = self.registers[rn as usize] as u64;
                let rdhi = self.registers[rd as usize] as u64;
                let existing = ((rdhi << 32) | rdlo) as i64;
                let result = (rm_reg as i64)
                    .wrapping_mul(rs_reg as i64)
                    .wrapping_add(existing);
                self.registers[rn as usize] = result as u32;
                self.registers[rd as usize] = (result >> 32) as u32;

                if (instruction >> 20) & 1 == 1 {
                    let n = (result >> 63) as u32 == 1;
                    let z = result == 0;
                    let c = (self.cpsr >> 29) & 1 == 1;
                    let v = (self.cpsr >> 28) & 1 == 1;
                    self.set_flags(n, z, c, v);
                }
            }

            //UMULL — unsigned multiply long
            0b0100 => {
                let result = (rm_reg as u64).wrapping_mul(rs_reg as u64);
                self.registers[rn as usize] = result as u32; // RdLo
                self.registers[rd as usize] = (result >> 32) as u32; // RdHi
                if (instruction >> 20) & 1 == 1 {
                    let n = (result >> 63) as u32 == 1;
                    let z = result == 0;
                    let c = (self.cpsr >> 29) & 1 == 1;
                    let v = (self.cpsr >> 28) & 1 == 1;
                    self.set_flags(n, z, c, v);
                }
            }
            //UMLAL — unsigned multiply accumulate long
            0b0101 => {
                let rdlo = self.registers[rn as usize] as u64;
                let rdhi = self.registers[rd as usize] as u64;
                let existing = (rdhi << 32) | rdlo;
                let result = (rm_reg as u64)
                    .wrapping_mul(rs_reg as u64)
                    .wrapping_add(existing);
                self.registers[rn as usize] = result as u32;
                self.registers[rd as usize] = (result >> 32) as u32;
                if (instruction >> 20) & 1 == 1 {
                    let n = (result >> 63) as u32 == 1;
                    let z = result == 0;
                    let c = (self.cpsr >> 29) & 1 == 1;
                    let v = (self.cpsr >> 28) & 1 == 1;
                    self.set_flags(n, z, c, v);
                }
            }
            _ => self.halt("unimplemented MUL variant", instruction),
        }
    }
}
