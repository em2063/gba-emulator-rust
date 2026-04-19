use crate::cpu::CPU;
use crate::memory_bus::MemoryBus;

//Implements THUMB instructions within CPU
impl CPU {
    //execute 16-bit thumb instructions
    pub fn execute_thumb_instruction(&mut self, bus: &mut MemoryBus, instruction: u16) {
        let bits_13_15 = (instruction >> 13) & 0b111;
        match bits_13_15 {
            0b000 => self.execute_thumb_move_shifted(instruction),
            _ => todo!(),
        }
    }

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
}
