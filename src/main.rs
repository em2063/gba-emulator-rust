mod cpu;
mod memory_bus;

fn main() {
    let rom: Vec<u8> = std::fs::read("arm.gba").unwrap();
    let mut bus = memory_bus::MemoryBus::new(rom);
    let mut cpu = cpu::CPU::new();
    cpu.registers[13] = 0x03008000; //top of iwram
    cpu.registers[15] = 0x08000000;

    let mut count = 0;
    loop {
        count += 1;
        if count > 500_000 {
            println!("Max instructions reached");
            println!("R12: {:#010x}", cpu.registers[12]);
            break;
        }

        let pc = cpu.registers[15];
        let instruction = bus.read_u32(cpu.registers[15]);
        println!("PC: {:#010x} instruction: {:#010x}", pc, instruction);
        if instruction == 0xEAFFFFFE {
            println!("halted at PC: {:#010x}", pc);
            println!("Registers at halt:");
            for i in 0..16 {
                println!("R{}: {:#010x}", i, cpu.registers[i]);
            }
            println!("CPSR: {:#010x}", cpu.cpsr);
            break;
        }
        cpu.registers[15] = cpu.registers[15].wrapping_add(4);
        cpu.execute_instruction(&mut bus, instruction);
    }
}
