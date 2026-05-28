use crate::memory_bus::MemoryBus;

// 16 general registers (R15 = PC, R14 = LR, R13 = SP), CPSR, and per-mode
// banked R13/R14/SPSR. FIQ also banks R8-R12.
pub struct CPU {
    pub registers: [u32; 16],
    pub cpsr: u32,

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

    pub r8_fiq: u32,
    pub r9_fiq: u32,
    pub r10_fiq: u32,
    pub r11_fiq: u32,
    pub r12_fiq: u32,
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            registers: [0; 16],
            cpsr: 0,
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
            r8_fiq: 0,
            r9_fiq: 0,
            r10_fiq: 0,
            r11_fiq: 0,
            r12_fiq: 0,
        }
    }
}

include!("cpu/dispatch.rs");
include!("cpu/memory.rs");
include!("cpu/alu.rs");
include!("cpu/mul.rs");
include!("cpu/psr.rs");
include!("cpu/branch.rs");
include!("cpu/exceptions.rs");
include!("cpu/shifter.rs");
include!("cpu/flags.rs");
