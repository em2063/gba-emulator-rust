use std::result;

use crate::cpu::CPU;
use crate::memory_bus::MemoryBus;

//Implements THUMB instructions within CPU
impl CPU {
    //execute 16-bit thumb instructions
    pub fn execute_thumb_instruction(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let bits_13_15 = (instruction >> 13) & 0b111;
        match bits_13_15 {
            0b000 => {
                if (instruction >> 11) & 0b11 == 0b11 {
                    self.execute_add_subtract(instruction)
                } else {
                    self.execute_thumb_move_shifted(instruction)
                }
            }
            0b001 => self.execute_mov_cmp_add_sub(instruction),
            _ => {
                let bits_15_10 = (instruction >> 10) & 0b111111;
                match bits_15_10 {
                    0b010000 => self.decode_thumb_alu(instruction),
                    _ => todo!(),
                }
            }
        }
    }

    //THUMB.1 move shifted register
    fn execute_thumb_move_shifted(&mut self, instruction: u16) {
        print!("THUMB.1");
        let offset = (instruction >> 6) & 0b11111;
        let rs = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;
        let opcode = (instruction >> 11) & 0b11;

        let rm = self.registers[rs as usize];
        let carry_in = (self.cpsr >> 29) & 1 == 1;

        let (result, carry) = self.apply_shift(rm, opcode as u32, offset as u32, carry_in, false);

        self.registers[rd as usize] = result;
        let (n, z, c, v) = self.logical_flags(result, carry);
        self.set_flags(n, z, c, v);
    }

    //THUMB.2 add/subtract
    fn execute_add_subtract(&mut self, instruction: u16) {
        print!("THUMB.2");

        let opcode = (instruction >> 9) & 0b11;
        let rs = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;

        let value_rn_nn = (instruction >> 6) & 0b111;
        let rn = self.registers[value_rn_nn as usize];
        match opcode {
            0x0 => {
                let rsv = self.registers[rs as usize];
                self.registers[rd as usize] = rsv.wrapping_add(rn);

                let (n, z, c, v) = self.add_flags(rsv, rn);
                self.set_flags(n, z, c, v);
            }
            0x1 => {
                let rsv = self.registers[rs as usize];
                self.registers[rd as usize] = rsv.wrapping_sub(rn);

                let (n, z, c, v) = self.sub_flags(rsv, rn);
                self.set_flags(n, z, c, v);
            }
            0x2 => {
                let rsv = self.registers[rs as usize];
                self.registers[rd as usize] = rsv.wrapping_add(value_rn_nn as u32);

                let (n, z, c, v) = self.add_flags(rsv, value_rn_nn as u32);
                self.set_flags(n, z, c, v);
            }
            0x3 => {
                let rsv = self.registers[rs as usize];
                self.registers[rd as usize] = rsv.wrapping_sub(value_rn_nn as u32);

                let (n, z, c, v) = self.sub_flags(rsv, value_rn_nn as u32);
                self.set_flags(n, z, c, v);
            }
            _ => todo!(),
        }
    }

    //THUMB.3 mov/compare/add/sub (imm)
    fn execute_mov_cmp_add_sub(&mut self, instruction: u16) {
        let opcode = (instruction >> 11) & 0b11;
        let rd = (instruction >> 8) & 0b111;
        let rd_val = self.registers[rd as usize];
        let nn = instruction & 0xFF;

        match opcode {
            0b0 => {
                self.registers[rd as usize] = nn as u32;

                let c = (self.cpsr >> 29) & 1 == 1;
                let (n, z, _, _) = self.logical_flags(nn as u32, c);
                let v = (self.cpsr >> 28) & 1 == 1;
                self.set_flags(n, z, c, v);
            }
            0b1 => {
                let (n, z, c, v) = self.sub_flags(rd_val, nn as u32);
                self.set_flags(n, z, c, v);
            }
            0b10 => {
                self.registers[rd as usize] = rd_val.wrapping_add(nn as u32);

                let (n, z, c, v) = self.add_flags(rd_val, nn as u32);
                self.set_flags(n, z, c, v);
            }
            0b11 => {
                self.registers[rd as usize] = rd_val.wrapping_sub(nn as u32);

                let (n, z, c, v) = self.sub_flags(rd_val, nn as u32);
                self.set_flags(n, z, c, v);
            }
            _ => todo!(),
        }
    }

    //THUMB.4 ALU
    fn decode_thumb_alu(&mut self, instruction: u16) {
        let opcode = (instruction >> 6) & 0xF;
        let rs = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;
        let rd_val = self.registers[rd as usize];
        let rs_val = self.registers[rs as usize];

        match opcode {
            //AND
            0x0 => {
                let result = rd_val & rs_val;
                self.registers[rd as usize] = rd_val & rs_val;

                let c = (self.cpsr >> 29) & 1 == 1;
                let (n, z, c, v) = self.logical_flags(result as u32, c);
                self.set_flags(n, z, c, v);
            }
            //EOR
            0x1 => {
                let result = rd_val ^ rs_val;
                self.registers[rd as usize] = result;

                let c = (self.cpsr >> 29) & 1 == 1;
                let (n, z, c, v) = self.logical_flags(result as u32, c);
                self.set_flags(n, z, c, v);
            }
            //LSL
            0x2 => {
                let c = (self.cpsr >> 29) & 1 == 1;
                let (result, carry) = self.apply_shift(rd_val, 0b00, rs_val, c, true);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, carry);
                self.set_flags(n, z, c, v);
            }
            //LSR
            0x3 => {
                let c = (self.cpsr >> 29) & 1 == 1;
                let (result, carry) = self.apply_shift(rd_val, 0b01, rs_val, c, true);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, carry);
                self.set_flags(n, z, c, v);
            }
            //ASR
            0x4 => {
                let c = (self.cpsr >> 29) & 1 == 1;
                let (result, carry) = self.apply_shift(rd_val, 0b10, rs_val, c, true);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, carry);
                self.set_flags(n, z, c, v);
            }
            //ADC
            0x5 => {
                let cy = (self.cpsr >> 29) & 1;
                let result = rd_val.wrapping_add(rs_val).wrapping_add(cy);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.adc_flags(rd_val, rs_val, cy);
                self.set_flags(n, z, c, v);
            }
            //SBC
            0x6 => {
                let cy = (self.cpsr >> 29) & 1;
                let result = rd_val.wrapping_sub(rs_val).wrapping_sub(1 - cy);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.sbc_flags(rd_val, rs_val, cy);
                self.set_flags(n, z, c, v);
            }
            //ROR
            0x7 => {
                let c = (self.cpsr >> 29) & 1 == 1;
                let (result, carry) = self.apply_shift(rd_val, 0b11, rs_val, c, true);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, carry);
                self.set_flags(n, z, c, v);
            }
            //TST
            0x8 => {
                let cy = (self.cpsr >> 29) & 1 == 1;
                let result = rd_val & rs_val;

                let (n, z, c, v) = self.logical_flags(result, cy);
                self.set_flags(n, z, c, v);
            }
            //NEG
            0x9 => {
                let result = 0u32.wrapping_sub(rs_val);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.sub_flags(0, rs_val);
                self.set_flags(n, z, c, v);
            }
            //CMP
            0xA => {
                let result = rd_val.wrapping_sub(rs_val);

                let (n, z, c, v) = self.sub_flags(rd_val, rs_val);
                self.set_flags(n, z, c, v);
            }
            //CMN
            0xB => {
                let result = rd_val.wrapping_add(rs_val);

                let (n, z, c, v) = self.add_flags(rd_val, rs_val);
                self.set_flags(n, z, c, v);
            }
            //ORR
            0xC => {
                let cy = (self.cpsr >> 29) & 1 == 1;
                let result = rd_val | rs_val;
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, cy);
                self.set_flags(n, z, c, v);
            }
            //MUL
            0xD => {
                let result = rd_val * rs_val;
                self.registers[rd as usize] = result;

                let c = (self.cpsr >> 29) & 1 == 1;
                let v = (self.cpsr >> 28) & 1 == 1;
                let n = (result >> 31) == 1;
                let z = result == 0;
                self.set_flags(n, z, c, v);
            }
            //BIC
            0xE => {
                let cy = (self.cpsr >> 29) & 1 == 1;
                let result = rd_val & (!rs_val);
                self.registers[rd as usize] = result;

                let (n, z, c, v) = self.logical_flags(result, cy);
                self.set_flags(n, z, c, v);
            }
            //MVN
            0xF => {
                let cy = (self.cpsr >> 29) & 1 == 1;
                let result = !rs_val;
                self.registers[rd as usize] = result as u32;

                let (n, z, c, v) = self.logical_flags(result as u32, cy);
                self.set_flags(n, z, c, v);
            }
            _ => todo!(),
        }
    }
}
