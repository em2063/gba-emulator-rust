impl CPU {
    //decomposes alu instructions and executes on registers
    fn decode_alu(&mut self, instruction: u32) {
        let opcode = (instruction >> 21) & 0xF;

        match opcode {
            0xD => self.execute_mov(instruction),
            0b100 => self.execute_add(instruction),
            0b10 => self.execute_sub(instruction),
            0xA => self.execute_cmp(instruction),
            0b1001 => self.execute_teq(instruction),
            0b0000 => self.execute_and(instruction),
            0b1100 => self.execute_orr(instruction),
            0b1110 => self.execute_bic(instruction),
            0x8 => self.execute_tst(instruction),
            0xF => self.execute_mvn(instruction),
            0x5 => self.execute_adc(instruction),
            0x6 => self.execute_subc(instruction),
            0x1 => self.execute_xor(instruction),
            0x3 => self.execute_rsb(instruction),
            0x7 => self.execute_rsc(instruction),
            0xB => self.execute_cmn(instruction),
            _ => self.halt(
                &format!(
                    "unimplemented ALU opcode {:#06b}",
                    (instruction >> 21) & 0xF
                ),
                instruction,
            ),
        }
    }

    //MOV rd, op2
    fn execute_mov(&mut self, instruction: u32) {
        let dest = (instruction >> 12) & 0xF;
        let (op2, carry) = self.decode_op2(instruction);

        if dest == 15 {
            let target = op2 & !1;
            self.registers[15] = target;

            if (instruction >> 20) & 1 == 1 {
                // MOVS PC
                let spsr = self.get_spsr();
                self.switch_mode(spsr & 0x1F);
                self.cpsr = spsr;
            } else {
                // MOV PC
                if op2 & 1 == 1 {
                    self.cpsr |= 1 << 5;
                } else {
                    self.cpsr &= !(1 << 5);
                }
            }
        } else {
            self.registers[dest as usize] = op2;
            if (instruction >> 20) & 1 == 1 {
                let (n, z, c, v) = self.logical_flags(op2, carry);
                self.set_flags(n, z, c, v);
            }
        }
    }

    //ADD Rd, Rn, op2
    fn execute_add(&mut self, instruction: u32) {
        let dest_register = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, _carry) = self.decode_op2(instruction);
        self.registers[dest_register as usize] = rn.wrapping_add(op2);

        // ADDS PC, ... — exception return: restore CPSR from SPSR
        if dest_register == 15 && (instruction >> 20) & 1 == 1 {
            let spsr = self.get_spsr();
            self.switch_mode(spsr & 0x1F);
            self.cpsr = spsr;
            return;
        }

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.add_flags(rn, op2);
            self.set_flags(n, z, c, v);
        }
    }

    //SUB rd, rn, op2
    fn execute_sub(&mut self, instruction: u32) {
        let dest_register = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);

        let (op2, _carry) = self.decode_op2(instruction);
        let result = rn.wrapping_sub(op2);
        self.registers[dest_register as usize] = result;

        // SUBS PC, ... — exception return: restore CPSR from SPSR
        if dest_register == 15 && (instruction >> 20) & 1 == 1 {
            let spsr = self.get_spsr();
            self.switch_mode(spsr & 0x1F);
            self.cpsr = spsr;
            return;
        }

        if dest_register == 15 && (instruction >> 20) & 1 == 1 {
            let spsr = self.get_spsr();
            self.switch_mode(spsr & 0x1F);
            self.cpsr = spsr;
        } else if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.sub_flags(rn, op2);
            self.set_flags(n, z, c, v);
        }
    }

    //CMP rn, op2
    fn execute_cmp(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, _carry) = self.decode_op2(instruction);
        let (n, z, c, v) = self.sub_flags(rn, op2);
        self.set_flags(n, z, c, v);
    }

    fn execute_cmn(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, _carry) = self.decode_op2(instruction);
        let (n, z, c, v) = self.add_flags(rn, op2);
        self.set_flags(n, z, c, v);
    }

    fn execute_teq(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn ^ op2;

        let (n, z, c, v) = self.logical_flags(result, carry);
        self.set_flags(n, z, c, v);
    }

    fn execute_and(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn & op2;
        self.registers[rd as usize] = result;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.logical_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_orr(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn | op2;
        self.registers[rd as usize] = rn | op2;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.logical_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_bic(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn & (!op2);
        self.registers[rd as usize] = result;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.logical_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_tst(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn & op2;

        let (n, z, c, v) = self.logical_flags(result, carry);
        self.set_flags(n, z, c, v);
    }

    fn execute_mvn(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let (op2, carry) = self.decode_op2(instruction);
        let result = !op2;
        self.registers[rd as usize] = result;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.logical_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_adc(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let rd = (instruction >> 12) & 0xF;
        let (op2, _carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        self.registers[rd as usize] = rn.wrapping_add(op2).wrapping_add(carry);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.adc_flags(rn, op2, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_subc(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let rd = (instruction >> 12) & 0xF;
        let (op2, _carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        self.registers[rd as usize] = rn.wrapping_sub(op2).wrapping_add(carry).wrapping_sub(1);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.sbc_flags(rn, op2, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_xor(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
        let result = rn ^ op2;
        self.registers[rd as usize] = result;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.logical_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_rsb(&mut self, instruction: u32) {
        let rd = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, _carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        let result = op2.wrapping_sub(rn);
        self.registers[rd as usize] = result;

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.sub_flags(result, carry);
            self.set_flags(n, z, c, v);
        }
    }

    fn execute_rsc(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let rd = (instruction >> 12) & 0xF;
        let (op2, _carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        self.registers[rd as usize] = op2.wrapping_sub(rn).wrapping_add(carry).wrapping_sub(1);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.sbc_flags(rn, op2, carry);
            self.set_flags(n, z, c, v);
        }
    }
}
