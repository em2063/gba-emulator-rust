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
            0b011 => self.execute_ldr_str_with_immediate_offset(bus, instruction),
            _ => {
                let bits_15_12 = (instruction >> 12) & 0xF;
                match bits_15_12 {
                    0b1101 => {
                        self.execute_conditional_branch(instruction);
                    }
                    0b1100 => self.execute_multiple_ldr_str(bus, instruction),
                    0b1000 => self.execute_ldr_str_halfword(bus, instruction),
                    0b1010 => self.execute_get_relative_address(instruction),
                    0b1001 => self.execute_ldr_str_sp_relative(bus, instruction),
                    0b0101 => {
                        if ((instruction >> 9) & 1 == 0) {
                            self.execute_ldr_str_with_register_offset(bus, instruction)
                        } else {
                            self.execute_ldr_str_sign_extended(bus, instruction);
                        }
                    }
                    0b1011 => self.execute_push_pop_registers(bus, instruction),
                    _ => {
                        let bits_15_11 = (instruction >> 11) & 0b11111;
                        match bits_15_11 {
                            0b11110 | 0b11111 | 0b11101 => {
                                self.excecute_bl_with_long_offset(instruction)
                            }
                            0b11100 => self.execute_unconditional_branch(instruction),
                            0b1001 => self.execute_load_pc_relative(bus, instruction),
                            _ => {
                                let bits_15_10 = (instruction >> 10) & 0b111111;
                                match bits_15_10 {
                                    0b010000 => self.decode_thumb_alu(instruction),
                                    0b010001 => self.execute_hi_register_ops(instruction),
                                    _ => {
                                        let bits_15_8 = (instruction >> 8) & 0xFF;
                                        match bits_15_8 {
                                            0b10110000 => {
                                                self.execute_offset_stack_pointer(instruction)
                                            }
                                            0b11011111 => self.execute_swi_thumb(bus),
                                            _ => {
                                                println!(
                                                    "unimplemented instruction: {:#034b} at PC: {:#010x}",
                                                    instruction, self.registers[15]
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    //THUMB.1 move shifted register
    fn execute_thumb_move_shifted(&mut self, instruction: u16) {
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
                let result = rd_val.wrapping_mul(rs_val);
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

    //THUMB.5 HI registers operations/BX
    fn execute_hi_register_ops(&mut self, instruction: u16) {
        let opcode = (instruction >> 8) & 0b11;
        let msbs = (instruction >> 6) & 1;
        let msbd = (instruction >> 7) & 1;
        let rs = ((instruction >> 3) & 0b111) + (msbs * 8);
        let rd = (instruction & 0b111) + (msbd * 8);

        let source_reg = self.registers[rs as usize];
        let dest_reg = self.registers[rd as usize];

        match opcode {
            0 => self.registers[rd as usize] = dest_reg.wrapping_add(source_reg),
            1 => {
                let (n, z, c, v) = self.sub_flags(dest_reg, source_reg);
                self.set_flags(n, z, c, v);
            }
            2 => {
                if rd == 15 {
                    if source_reg & 1 == 1 {
                        self.cpsr |= 1 << 5;
                    } else {
                        self.cpsr &= !(1 << 5);
                    }
                    self.registers[15] = source_reg & !1;
                } else {
                    self.registers[rd as usize] = source_reg;
                }
            }
            3 => {
                if source_reg & 1 == 1 {
                    self.cpsr |= 1 << 5;
                } else {
                    self.cpsr &= !(1 << 5);
                }
                self.registers[15] = source_reg & !1;
            }
            _ => {}
        }
    }

    //THUMB.6 load PC-relative (for loading immediates from literal pool)
    fn execute_load_pc_relative(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let rd = (instruction >> 8) & 0b111;
        let nn = (instruction & 0xFF) as u32 * 4;
        let address = (self.registers[15].wrapping_add(2) & !2).wrapping_add(nn);

        self.registers[rd as usize] = bus.read_u32(address);
    }

    //THUMB.7 load/store with register offset
    fn execute_ldr_str_with_register_offset(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let ro = (instruction >> 6) & 0b111;
        let rb = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;
        let opcode = (instruction >> 10) & 0b11;

        let rb_register = self.registers[rb as usize];
        let rd_register = self.registers[rd as usize];
        let ro_register = self.registers[ro as usize];
        let address = rb_register.wrapping_add(ro_register);
        match opcode {
            0 => {
                bus.write_u32(address, rd_register);
            }
            1 => {
                bus.write_u8(address, rd_register as u8);
            }
            2 => {
                self.registers[rd as usize] = bus.read_u32(address);
            }
            3 => {
                self.registers[rd as usize] = bus.read_u8(address) as u32;
            }
            _ => {}
        }
    }

    //THUMB.8 load/store sign-extended byte/halfword
    fn execute_ldr_str_sign_extended(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let ro = (instruction >> 6) & 0b111;
        let rb = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;
        let opcode = (instruction >> 10) & 0b11;

        let rb_register = self.registers[rb as usize];
        let rd_register = self.registers[rd as usize];
        let ro_register = self.registers[ro as usize];
        let address = rb_register.wrapping_add(ro_register);
        match opcode {
            0 => {
                bus.write_u32(address, rd_register);
            }
            1 => {
                let byte = bus.read_u8(address);
                let sign_extended = (byte as i8) as i32 as u32;
                self.registers[rd as usize] = sign_extended;
            }
            2 => {
                let halfword = bus.read_u16(address);
                self.registers[rd as usize] = halfword as u32;
            }
            3 => {
                let halfword = bus.read_u16(address);
                let sign_extended = (halfword as i16) as i32 as u32;
                self.registers[rd as usize] = sign_extended;
            }
            _ => {}
        }
    }

    //THUMB.9 load/store with immediate offset
    fn execute_ldr_str_with_immediate_offset(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let rb = (instruction >> 3) & 0b111;
        let rd = instruction & 0b111;
        let nn = (instruction >> 6) & 0b11111;

        let rb_register = self.registers[rb as usize];
        let rd_register = self.registers[rd as usize];

        let opcode = (instruction >> 11) & 0b11;
        let is_byte = opcode >= 2;
        let scaled_nn = if is_byte { nn as u32 } else { nn as u32 * 4 };

        let address = rb_register.wrapping_add(scaled_nn as u32);
        match opcode {
            0 => {
                bus.write_u32(address, rd_register);
            }
            1 => {
                self.registers[rd as usize] = bus.read_u32(address);
            }
            2 => {
                bus.write_u8(address, rd_register as u8);
            }
            3 => {
                self.registers[rd as usize] = bus.read_u8(address) as u32;
            }
            _ => {}
        }
    }

    //THUMB.10 load/store halfword
    fn execute_ldr_str_halfword(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let offset = (instruction >> 6) & 0b11111;
        let base = (instruction >> 3) & 0b111;
        let rb = self.registers[base as usize];

        let source_dest = instruction & 0b111;

        let address = rb.wrapping_add((offset as u32) << 1);
        if (instruction >> 11) == 1 {
            self.registers[source_dest as usize] = bus.read_u16(address as u32) as u32;
        } else {
            bus.write_u16(address, self.registers[source_dest as usize] as u32);
        }
    }

    //THUMB.11 load/store SP-relative
    fn execute_ldr_str_sp_relative(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let rd = (instruction >> 8) & 0b111;
        let offset = (instruction & 0xFF) << 2;

        let address = self.registers[13].wrapping_add(offset as u32);
        if instruction >> 11 == 0 {
            bus.write_u32(address, self.registers[rd as usize] as u32);
        } else {
            self.registers[rd as usize] = bus.read_u32(address as u32) as u32;
        }
    }

    //THUMB.12 get relative address
    fn execute_get_relative_address(&mut self, instruction: u16) {
        let dest = (instruction >> 8) & 0b111;
        let offset = (instruction & 0xFF) << 2;

        if (instruction >> 11) == 0 {
            let pc_aligned = (self.registers[15].wrapping_add(2)) & !2;
            self.registers[dest as usize] = pc_aligned.wrapping_add(offset as u32);
        } else {
            self.registers[dest as usize] = self.registers[13].wrapping_add(offset as u32);
        }
    }

    //THUMB.13 add offset to stack pointer
    fn execute_offset_stack_pointer(&mut self, instruction: u16) {
        let nn = (instruction & 0b1111111) * 4;
        let opcode = (instruction >> 7) & 1;

        match opcode {
            0 => self.registers[13] = self.registers[13].wrapping_add(nn as u32),
            1 => self.registers[13] = self.registers[13].wrapping_sub(nn as u32),
            _ => {}
        }
    }

    //THUMB.14 push/pop registers
    fn execute_push_pop_registers(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let rlist = instruction & 0xFF;
        let pc_lr_bit = (instruction >> 8) & 1;
        let opcode = (instruction >> 11) & 1;

        if opcode == 0 {
            //push
            if pc_lr_bit == 1 {
                self.registers[13] = self.registers[13].wrapping_sub(4);
                bus.write_u32(self.registers[13], self.registers[14]);
            }
            for i in (0..8usize).rev() {
                if (rlist >> i) & 1 == 1 {
                    self.registers[13] = self.registers[13].wrapping_sub(4);
                    bus.write_u32(self.registers[13], self.registers[i]);
                }
            }
        } else {
            //pop
            for i in 0..8usize {
                if (rlist >> i) & 1 == 1 {
                    self.registers[i] = bus.read_u32(self.registers[13]);
                    self.registers[13] = self.registers[13].wrapping_add(4);
                }
            }
            if pc_lr_bit == 1 {
                let value = bus.read_u32(self.registers[13]);
                self.registers[13] = self.registers[13].wrapping_add(4);
                if value & 1 == 1 {
                    self.cpsr |= 1 << 5;
                } else {
                    self.cpsr &= !(1 << 5);
                }
                self.registers[15] = value & !1;
            }
        }
    }

    //THUMB.15 multiple load/store
    fn execute_multiple_ldr_str(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let base = (instruction >> 8) & 0b111;
        let rb = self.registers[base as usize];
        let mut address = rb;

        let rlist = instruction & 0xFF;
        let count = (0..8).filter(|i| (rlist >> i) & 1 == 1).count() as u16;
        let opcode = (instruction >> 11) & 1;

        for i in 0..8usize {
            if (rlist >> i) & 1 == 1 {
                if opcode == 1 {
                    self.registers[i] = bus.read_u32(address);
                } else {
                    bus.write_u32(address, self.registers[i]);
                }
                address = address.wrapping_add(4);
            }
        }

        let base_idx = ((instruction >> 8) & 0b111) as usize;
        self.registers[base_idx] = rb.wrapping_add(count as u32 * 4)
    }

    //THUMB.16 jumps and calls (conditional branching)
    fn execute_conditional_branch(&mut self, instruction: u16) {
        let opcode = ((instruction >> 8) & 0xF) as u32;
        if self.check_condition(opcode) {
            let offset = (instruction & 0xFF) as i8 as i32;
            self.registers[15] = (self.registers[15] as i32 + 2 + (offset << 1)) as u32;
        }
    }

    //THUMB.18 unconditional branch (B)
    fn execute_unconditional_branch(&mut self, instruction: u16) {
        let offset = ((instruction & 0x7FF) as i16) << 5 >> 5;
        self.registers[15] = (self.registers[15] as i32 + 2 + ((offset as i32) << 1)) as u32;
    }

    //THUMB.19 long branch with link
    //This may be used to call (or jump) to a subroutine, return address is saved in LR (R14).
    //Unlike all other THUMB mode instructions,
    //this instruction occupies 32bit of memory which are split into two 16bit THUMB opcodes
    fn excecute_bl_with_long_offset(&mut self, instruction: u16) {
        //Instruction 1
        let opcode = (instruction >> 11) & 0b11111;
        match opcode {
            0b11110 => {
                let nn = (instruction & 0x7FF) as u32;
                let nn_signed = ((nn << 21) as i32 >> 21) as u32;
                self.registers[14] = self.registers[15]
                    .wrapping_add(2)
                    .wrapping_add((nn_signed as u32) << 12);
            }
            0b11111 => {
                let nn = (instruction & 0x7FF) as u32;
                let temp = self.registers[15] | 1;
                self.registers[15] = self.registers[14].wrapping_add(nn << 1);
                self.registers[14] = temp;
            }
            0b11101 => {
                let nn = (instruction & 0x7FF) as u32;
                let temp = self.registers[15].wrapping_add(2) | 1;
                self.registers[15] = self.registers[14].wrapping_add(nn << 1) & !1; //clear bit 0
                self.registers[14] = temp;
                self.cpsr &= !(1 << 5); //clear T flag — switch to ARM mode
            }
            _ => todo!(),
        }
    }
    //swi
    fn execute_swi_thumb(&mut self, bus: &mut MemoryBus) {
        let saved_cpsr = self.cpsr; // must save before switch_mode modifies CPSR
        self.switch_mode(0b10011);
        // registers[15] is already pc+2 (incremented before dispatch in main.rs)
        self.registers[14] = self.registers[15];
        self.r14_svc = self.registers[15];
        self.spsr_svc = saved_cpsr;
        self.cpsr = (self.cpsr & !0x3F) | 0x13 | (1 << 7);
        self.cpsr &= !(1 << 5); // ARM mode
        self.registers[15] = 0x00000008;
    }
}
