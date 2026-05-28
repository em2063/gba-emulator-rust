impl CPU {
    pub fn apply_shift(
        &self,
        rm: u32,
        shift_type: u32,
        shift: u32,
        carry_in: bool,
        is_register: bool,
    ) -> (u32, bool) {
        match shift_type {
            0b00 => match shift {
                // LSL
                0 => (rm, carry_in),
                1..=31 => (rm << shift, (rm >> (32 - shift)) & 1 == 1),
                32 => (0, rm & 1 == 1),
                _ => (0, false),
            },
            0b01 => match shift {
                // LSR
                0 => {
                    if is_register {
                        (rm, carry_in)
                    } else {
                        (0, (rm >> 31) & 1 == 1)
                    }
                }
                1..=31 => (rm >> shift, (rm >> (shift - 1)) & 1 == 1),
                32 => (0, (rm >> 31) & 1 == 1),
                _ => (0, false),
            },
            0b10 => match shift {
                // ASR
                0 => {
                    if is_register {
                        (rm, carry_in)
                    } else {
                        (((rm as i32) >> 31) as u32, (rm >> 31) & 1 == 1)
                    }
                }
                1..=31 => (((rm as i32) >> shift) as u32, (rm >> (shift - 1)) & 1 == 1),
                _ => (((rm as i32) >> 31) as u32, (rm >> 31) & 1 == 1),
            },
            0b11 => match shift {
                // ROR
                0 => {
                    if is_register {
                        (rm, carry_in) // register ROR#0 = no shift
                    } else {
                        (((carry_in as u32) << 31) | (rm >> 1), rm & 1 == 1) // immediate ROR#0 = RRX
                    }
                } // RRX
                _ => {
                    let s = shift % 32;
                    if s == 0 {
                        (rm, (rm >> 31) & 1 == 1)
                    } else {
                        (rm.rotate_right(s), (rm >> (s - 1)) & 1 == 1)
                    }
                }
            },
            _ => (rm, carry_in),
        }
    }

    //decodes operand between loading as an immediate value or deducing from a register
    fn decode_op2(&mut self, instruction: u32) -> (u32, bool) {
        let op_flag = (instruction >> 25) & 1;
        let carry_in = (self.cpsr >> 29) & 1 == 1;

        if op_flag == 1 {
            let ror_shift = (instruction >> 8) & 0xF;
            let value = (instruction & 0xFF).rotate_right(ror_shift * 2);
            let carry = if ror_shift == 0 {
                (self.cpsr >> 29) & 1 == 1 // preserve carry if no rotation
            } else {
                (value >> 31) & 1 == 1 // carry = bit 31 of rotated result
            };
            (value, carry)
        } else {
            let shift_type = (instruction >> 5) & 0b11;
            let source_register = instruction & 0xF;
            let rm = self.read_register(source_register);

            let shift = if (instruction >> 4) & 1 == 0 {
                // shift by immediate
                (instruction >> 7) & 0x1F
            } else {
                // shift by register — bottom byte only
                self.registers[((instruction >> 8) & 0xF) as usize] & 0xFF
            };

            let is_register = (instruction >> 4) & 1 == 1;
            self.apply_shift(rm, shift_type, shift, carry_in, is_register)
        }
    }

    fn read_register(&self, index: u32) -> u32 {
        if index == 15 {
            (self.registers[15].wrapping_add(4)) & !3
        } else {
            self.registers[index as usize]
        }
    }
}
