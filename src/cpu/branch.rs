impl CPU {
    //B label
    //BL label
    fn decode_branch(&mut self, instruction: u32) {
        let mut offset = (instruction & 0x00FFFFFF) << 2;
        offset = ((offset << 6) as i32 >> 6) as u32;

        if (instruction >> 24) & 1 == 1 {
            self.registers[14] = self.registers[15];
        }

        self.registers[15] = self.registers[15].wrapping_add(offset).wrapping_add(4);
    }

    fn decode_branch_exchange(&mut self, instruction: u32) {
        let opcode = (instruction >> 4) & 0xF;
        let rn = (instruction) & 0xF;
        match opcode {
            0b0001 => {
                let target = self.registers[rn as usize];
                let thumb = target & 1;
                if thumb == 1 {
                    self.cpsr |= 1 << 5;
                } else {
                    self.cpsr &= !(1 << 5);
                }
                self.registers[15] = target & !1;
            }
            0b0011 => self.halt("unimplemented BLX", instruction),
            _ => self.halt("unimplemented branch/exchange variant", instruction),
        }
    }
}
