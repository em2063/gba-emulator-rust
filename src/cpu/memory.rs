impl CPU {
    fn decode_memory(&mut self, bus: &mut MemoryBus, instruction: u32) {
        let base = (instruction >> 16) & 0xF;
        let rn = self.registers[base as usize];

        let rlist = instruction & 0xFFFF;
        let p = (instruction >> 24) & 1; // pre-index
        let u = (instruction >> 23) & 1; // up (1) or down (0)
        let s = (instruction >> 22) & 1; // S bit: exception return / user-bank
        let write_back = (instruction >> 21) & 1;
        let load = (instruction >> 20) & 1;

        let pc_in_list = (rlist >> 15) & 1 == 1;
        // LDM with S=1 and PC in list -> exception return: restore CPSR from SPSR after PC load.
        let exception_return = load == 1 && s == 1 && pc_in_list;

        //ARM always stores the lowest-numbered register at the lowest address.
        //compute the lowest address, then walk upward through the list.
        let count = (0..16).filter(|i| (rlist >> i) & 1 == 1).count() as u32;
        let mut address = if u == 1 {
            if p == 1 { rn.wrapping_add(4) } else { rn } //IB / IA
        } else {
            if p == 1 {
                rn.wrapping_sub(count * 4)
            }
            //DB (STMDB / LDMDB)
            else {
                rn.wrapping_sub(count * 4).wrapping_add(4)
            } //DA
        };

        for i in 0..16usize {
            if (rlist >> i) & 1 == 1 {
                if load == 1 {
                    let value = bus.read_u32(address);
                    if i == 15 {
                        if exception_return {
                            // Restore CPSR from SPSR before writing PC, then preserve
                            // whatever T bit the SPSR specified (don't strip from value).
                            let spsr = self.get_spsr();
                            self.switch_mode(spsr & 0x1F);
                            self.cpsr = spsr;
                            // In ARM mode, PC must be 4-aligned; in Thumb, 2-aligned.
                            let mask = if (self.cpsr >> 5) & 1 == 1 {
                                !1u32
                            } else {
                                !3u32
                            };
                            self.registers[15] = value & mask;
                        } else {
                            // Plain LDM with PC: BX-style mode switch via bit 0
                            self.registers[15] = value & !1;
                            if value & 1 == 1 {
                                self.cpsr |= 1 << 5;
                            } else {
                                self.cpsr &= !(1 << 5);
                            }
                        }
                    } else {
                        self.registers[i] = value;
                    }
                } else {
                    bus.write_u32(address, self.registers[i]);
                }
                address = address.wrapping_add(4);
            }
        }

        if write_back == 1 {
            self.registers[base as usize] = if u == 1 {
                rn.wrapping_add(count * 4)
            } else {
                rn.wrapping_sub(count * 4)
            };
        }
    }

    fn execute_halfword_transfer(&mut self, bus: &mut MemoryBus, instruction: u32) {
        let p = (instruction >> 24) & 1; //pre/post
        let u = (instruction >> 23) & 1; //up/down
        let i = (instruction >> 22) & 1; //imm offset flag (0 = reg offset, 1 = imm offset)
        let w = (instruction >> 21) & 1; //write-back bit (0 = no, 1 = write add into base)
        let l = (instruction >> 20) & 1; //ldr/str bit (0 = store to mem, 1 = load from mem)

        let rn = (instruction >> 16) & 0xF; //base register
        let rd = (instruction >> 12) & 0xF; //source/dest register

        let offset = if i == 0 {
            self.registers[(instruction & 0xF) as usize]
        } else {
            ((instruction >> 8) & 0xF) << 4 | (instruction & 0xF)
        };

        let address = if u == 1 {
            if p == 1 {
                self.registers[rn as usize].wrapping_add(offset)
            } else {
                self.registers[rn as usize]
            }
        } else {
            if p == 1 {
                self.registers[rn as usize].wrapping_sub(offset)
            } else {
                self.registers[rn as usize]
            }
        };

        let opcode = (instruction >> 5) & 0b11;
        if l == 0 {
            match opcode {
                1 => bus.write_u16(address, self.registers[rd as usize]),
                2 => {
                    self.registers[rd as usize] = bus.read_u32(address);
                    self.registers[rd.wrapping_add(1) as usize] =
                        bus.read_u32(address.wrapping_add(4));
                }
                3 => {
                    bus.write_u32(address, self.registers[rd as usize]);
                    bus.write_u32(
                        address.wrapping_add(4),
                        self.registers[rd.wrapping_add(1) as usize],
                    );
                }
                _ => {}
            }
        } else {
            match opcode {
                1 => self.registers[rd as usize] = bus.read_u16(address) as u32,
                2 => self.registers[rd as usize] = bus.read_u8(address) as i8 as i32 as u32, // sign extend
                3 => self.registers[rd as usize] = bus.read_u16(address) as i16 as i32 as u32, // sign extend
                _ => {}
            }
        }

        if w == 1 || p == 0 {
            let wb = if u == 1 {
                self.registers[rn as usize].wrapping_add(offset)
            } else {
                self.registers[rn as usize].wrapping_sub(offset)
            };
            self.registers[rn as usize] = wb;
        }
    }

    fn execute_ldr_str(&mut self, bus: &mut MemoryBus, instruction: u32) {
        let is_register = (instruction >> 25) & 1;
        let offset;
        if is_register == 0 {
            offset = instruction & 0xFFF;
        } else {
            let register_flag = (instruction >> 4) & 1;
            if register_flag == 0b0 {
                let shift = (instruction >> 7) & 0x1F;
                let source_register = instruction & 0xF;

                let rm = self.read_register(source_register);
                offset = match (instruction >> 5) & 0b11 {
                    0b0 => rm << shift,
                    0b1 => rm >> shift,
                    0b10 => ((rm as i32) >> shift) as u32,
                    0b11 => rm.rotate_right(shift),
                    _ => unreachable!(),
                };
            } else {
                let register = self.registers[((instruction >> 8) & 0xF) as usize];
                let shift = register & 0xFF;

                let source_register = instruction & 0xF;

                let rm = self.read_register(source_register);
                offset = match (instruction >> 5) & 0b11 {
                    0b0 => rm,
                    0b1 => rm.wrapping_shr(shift),
                    0b10 => ((rm as i32).wrapping_shr(shift)) as u32,
                    0b11 => rm.rotate_right(shift),
                    _ => unreachable!(),
                };
            }
        }
        let u = (instruction >> 23) & 1;
        let p = (instruction >> 24) & 1;
        let w = (instruction >> 21) & 1;
        let l = (instruction >> 20) & 1;
        let b = (instruction >> 22) & 1; // byte or word

        let base_idx = ((instruction >> 16) & 0xF) as usize;
        let rd_idx = ((instruction >> 12) & 0xF) as usize;

        let rn = self.read_register(base_idx as u32);

        //Step 1: compute offset address (but DON'T apply post yet)
        let offset_addr = if u == 1 {
            rn.wrapping_add(offset)
        } else {
            rn.wrapping_sub(offset)
        };

        //Step 2: choose address depending on P
        let address = if p == 1 {
            offset_addr // pre-indexed
        } else {
            rn // post-indexed uses original rn
        };

        //Step 3: perform memory access
        if l == 1 {
            // LDR
            if b == 1 {
                self.registers[rd_idx] = bus.read_u8(address) as u32;
            } else {
                let value = bus.read_u32(address);
                if rd_idx == 15 {
                    self.registers[15] = value & !1;
                    if value & 1 == 1 {
                        self.cpsr |= 1 << 5;
                    } else {
                        self.cpsr &= !(1 << 5);
                    }
                } else {
                    self.registers[rd_idx] = value;
                }
            }
        } else {
            // STR
            let value = self.registers[rd_idx];
            if b == 1 {
                bus.write_u8(address, value as u8);
            } else {
                bus.write_u32(address, value);
            }
        }

        // Step 4: write-back
        if p == 0 {
            // post-index: ALWAYS write-back
            self.registers[base_idx] = offset_addr;
        } else if w == 1 {
            // pre-index: optional write-back
            self.registers[base_idx] = offset_addr;
        }
    }
}
