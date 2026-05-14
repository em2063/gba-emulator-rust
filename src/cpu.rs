use crate::memory_bus::MemoryBus;

//struct to hold 16 registers: 0-12 general purpose, 13 stack pointer,
//14 link register, 15 program counter
//CPSR flags stored as a single 32-bit
//SPSR flags stored as 32-bit
//banked registers for each mode (required for exceptions)
pub struct CPU {
    pub registers: [u32; 16],
    pub cpsr: u32,
    pub spsr: u32,

    //banked R13/R14 for each mode
    pub r13_svc: u32,
    pub r14_svc: u32,
    pub spsr_svc: u32,
    pub r13_irq: u32,
    pub r14_irq: u32,
    pub spsr_irq: u32,
    pub r13_abt: u32,
    pub r14_abt: u32,
    pub spsr_abt: u32,
    pub r13_und: u32,
    pub r14_und: u32,
    pub spsr_und: u32,
    pub r13_fiq: u32,
    pub r14_fiq: u32,
    pub spsr_fiq: u32,
    pub r13_usr: u32,
    pub r14_usr: u32,

    // FIQ also banks R8-R12
    pub r8_fiq: u32,
    pub r9_fiq: u32,
    pub r10_fiq: u32,
    pub r11_fiq: u32,
    pub r12_fiq: u32,
}

impl CPU {
    //init cpu instance
    pub fn new() -> CPU {
        CPU {
            registers: [0; 16],
            cpsr: 0,
            spsr: 0,

            //banked R13/R14 for each mode
            r13_svc: 0,
            r14_svc: 0,
            spsr_svc: 0,
            r13_irq: 0,
            r14_irq: 0,
            spsr_irq: 0,
            r13_abt: 0,
            r14_abt: 0,
            spsr_abt: 0,
            r13_und: 0,
            r14_und: 0,
            spsr_und: 0,
            r13_fiq: 0,
            r14_fiq: 0,
            spsr_fiq: 0,
            r13_usr: 0,
            r14_usr: 0,

            // FIQ also banks R8-R12
            r8_fiq: 0,
            r9_fiq: 0,
            r10_fiq: 0,
            r11_fiq: 0,
            r12_fiq: 0,
        }
    }

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
            _ => todo!(),
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
                    0b110 | 0b111 => {}
                    0b010 | 0b011 => self.execute_ldr_str(bus, instruction),
                    0b000 | 0b001 => {
                        if (instruction >> 8) & 0xFFFFF == 0b00010010111111111111 {
                            self.decode_branch_exchange(instruction);
                        } else if (instruction >> 4) & 1 == 1
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

    fn decode_memory(&mut self, bus: &mut MemoryBus, instruction: u32) {
        let base = (instruction >> 16) & 0xF;
        let rn = self.registers[base as usize];

        let rlist = instruction & 0xFFFF;
        let p = (instruction >> 24) & 1; // pre-index
        let u = (instruction >> 23) & 1; // up (1) or down (0)
        let write_back = (instruction >> 21) & 1;
        let load = (instruction >> 20) & 1;

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
                        self.registers[15] = value & !1;
                        if value & 1 == 1 {
                            self.cpsr |= 1 << 5;
                        } else {
                            self.cpsr &= !(1 << 5);
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
                    _ => todo!(),
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
                    _ => todo!(),
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

        // Step 1: compute offset address (but DON'T apply post yet)
        let offset_addr = if u == 1 {
            rn.wrapping_add(offset)
        } else {
            rn.wrapping_sub(offset)
        };

        // Step 2: choose address depending on P
        let address = if p == 1 {
            offset_addr // pre-indexed
        } else {
            rn // post-indexed uses original rn
        };

        // Step 3: perform memory access
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
            _ => {
                println!(
                    "unimplemented ALU opcode: {:#06b}",
                    (instruction >> 21) & 0xF
                );
                todo!()
            }
        }
    }

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
            _ => {
                print!("Unimplemented MUL instruction: {:#034b}\n", instruction);
                todo!()
            }
        }
    }

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
            0b0011 => {
                print!("BLX - todo\n");
                todo!();
            }
            _ => todo!(),
        }
    }

    //switches between banked registers and system registers, used for exceptions and SWI
    pub fn switch_mode(&mut self, new_mode: u32) {
        if new_mode == 0b10010 {
            println!(
                "Switching to IRQ mode from PC={:#010x} CPSR={:#010x}",
                self.registers[15], self.cpsr
            );
        }
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

    pub fn execute_swi(&mut self, bus: &mut MemoryBus, instruction: u32) {
        let swi_num = (instruction >> 16) & 0xFF;
        if swi_num == 0x05 {
            bus.write_u16(0x04000208, 1); // enable IME
            return; // don't jump to BIOS
        }

        let saved_cpsr = self.cpsr; // must save before switch_mode modifies CPSR
        self.switch_mode(0b10011);
        // registers[15] is already pc+4 (incremented before dispatch in main.rs)
        self.registers[14] = self.registers[15];
        self.r14_svc = self.registers[15];
        self.spsr_svc = saved_cpsr;
        self.cpsr = (self.cpsr & !0x3F) | 0x13 | (1 << 7);
        self.cpsr &= !(1 << 5); // ARM mode
        self.registers[15] = 0x00000008;
    }

    pub fn trigger_irq(&mut self, bus: &mut MemoryBus) {
        let return_addr = self.registers[15];
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

    pub fn set_flags(&mut self, n: bool, z: bool, c: bool, v: bool) {
        if n {
            self.cpsr |= 1 << 31
        } else {
            self.cpsr &= !(1 << 31)
        }
        if z {
            self.cpsr |= 1 << 30
        } else {
            self.cpsr &= !(1 << 30)
        }
        if c {
            self.cpsr |= 1 << 29
        } else {
            self.cpsr &= !(1 << 29)
        }
        if v {
            self.cpsr |= 1 << 28
        } else {
            self.cpsr &= !(1 << 28)
        }
    }

    pub fn sub_flags(&self, rn: u32, op2: u32) -> (bool, bool, bool, bool) {
        let result = rn.wrapping_sub(op2);
        let rn_sign = (rn >> 31) & 1;
        let op2_sign = (op2 >> 31) & 1;
        let result_sign = (result >> 31) & 1;

        let n: bool = (result >> 31) == 1;
        let z: bool = result == 0;
        let c: bool = rn >= op2;
        let v: bool = (rn_sign == 0 && op2_sign == 1 && result_sign == 1)
            || (rn_sign == 1 && op2_sign == 0 && result_sign == 0);

        (n, z, c, v)
    }

    pub fn add_flags(&self, rn: u32, op2: u32) -> (bool, bool, bool, bool) {
        let result = rn.wrapping_add(op2);
        let n: bool = (result >> 31) == 1;
        let z: bool = result == 0;
        let c: bool = (rn as u64) + (op2 as u64) > 0xFFFFFFFF;

        let rn_sign = (rn >> 31) & 1;
        let op2_sign = (op2 >> 31) & 1;
        let result_sign = (result >> 31) & 1;

        let v = (rn_sign == 0 && op2_sign == 0 && result_sign == 1)
            || (rn_sign == 1 && op2_sign == 1 && result_sign == 0);

        (n, z, c, v)
    }

    pub fn logical_flags(&self, result: u32, carry: bool) -> (bool, bool, bool, bool) {
        let n = (result >> 31) == 1;
        let z = result == 0;
        let c = carry; // only affected by shift, handle later
        let v = false; // never affected by logical ops
        (n, z, c, v)
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
        let (op2, carry) = self.decode_op2(instruction);
        self.registers[dest_register as usize] = rn.wrapping_add(op2);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.add_flags(rn, op2);
            self.set_flags(n, z, c, v);
        }
    }

    //SUB rd, rn, op2
    fn execute_sub(&mut self, instruction: u32) {
        let dest_register = (instruction >> 12) & 0xF;
        let rn = self.read_register((instruction >> 16) & 0xF);

        let (op2, carry) = self.decode_op2(instruction);
        self.registers[dest_register as usize] = rn.wrapping_sub(op2);

        if dest_register == 15 && (instruction >> 20) & 1 == 1 {
            let spsr = self.get_spsr();
            println!(
                "SUBS PC: LR={:#010x} -> PC={:#010x}, SPSR={:#010x} (restoring mode {:#04x})",
                rn,
                rn.wrapping_sub(op2),
                spsr,
                spsr & 0x1F
            );
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
        let (op2, carry) = self.decode_op2(instruction);
        let (n, z, c, v) = self.sub_flags(rn, op2);
        self.set_flags(n, z, c, v);
    }

    fn execute_cmn(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let (op2, carry) = self.decode_op2(instruction);
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
        let (op2, carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        self.registers[rd as usize] = rn.wrapping_add(op2).wrapping_add(carry);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.adc_flags(rn, op2, carry);
            self.set_flags(n, z, c, v);
        }
    }

    pub fn adc_flags(&self, rn: u32, op2: u32, carry: u32) -> (bool, bool, bool, bool) {
        let result = rn.wrapping_add(op2).wrapping_add(carry);
        let n = (result >> 31) == 1;
        let z = result == 0;
        let c = (rn as u64) + (op2 as u64) + (carry as u64) > 0xFFFFFFFF;
        let rn_sign = (rn >> 31) & 1;
        let op2_sign = (op2 >> 31) & 1;
        let result_sign = (result >> 31) & 1;
        let v = (rn_sign == 0 && op2_sign == 0 && result_sign == 1)
            || (rn_sign == 1 && op2_sign == 1 && result_sign == 0);
        (n, z, c, v)
    }

    pub fn sbc_flags(&self, rn: u32, op2: u32, carry: u32) -> (bool, bool, bool, bool) {
        let result = rn.wrapping_sub(op2).wrapping_add(carry).wrapping_sub(1);
        let n = (result >> 31) == 1;
        let z = result == 0;
        let c = (rn as u64) >= (op2 as u64) + (1 - carry as u64);
        let rn_sign = (rn >> 31) & 1;
        let op2_sign = (op2 >> 31) & 1;
        let result_sign = (result >> 31) & 1;
        let v = (rn_sign == 1 && op2_sign == 0 && result_sign == 0)
            || (rn_sign == 0 && op2_sign == 1 && result_sign == 1);
        (n, z, c, v)
    }

    fn execute_subc(&mut self, instruction: u32) {
        let rn = self.read_register((instruction >> 16) & 0xF);
        let rd = (instruction >> 12) & 0xF;
        let (op2, carry) = self.decode_op2(instruction);
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
        let (op2, carry) = self.decode_op2(instruction);
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
        let (op2, carry) = self.decode_op2(instruction);
        let carry = (self.cpsr >> 29) & 1;
        self.registers[rd as usize] = op2.wrapping_sub(rn).wrapping_add(carry).wrapping_sub(1);

        if (instruction >> 20) & 1 == 1 {
            let (n, z, c, v) = self.sbc_flags(rn, op2, carry);
            self.set_flags(n, z, c, v);
        }
    }
}
