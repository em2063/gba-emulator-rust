pub struct Timer {
    pub counter: u16,
    pub reload: u16,
    enabled: bool,
    pub irq_enabled: bool,
    pub cascade: bool,
    prescaler: u32,
    cycle_count: u32,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            counter: 0,
            reload: 0,
            enabled: false,
            irq_enabled: false,
            cascade: false,
            prescaler: 1,
            cycle_count: 0,
        }
    }

    pub fn tick(&mut self, cycles: u32) -> bool {
        if !self.enabled {
            return false;
        }
        self.cycle_count += cycles;
        if self.cycle_count >= self.prescaler {
            self.cycle_count -= self.prescaler;
            self.counter = self.counter.wrapping_add(1);
            if self.counter == 0 {
                self.counter = self.reload;
                return true;
            }
        }
        false
    }

    pub fn update_control(&mut self, value: u16) {
        let was_enabled = self.enabled;

        self.prescaler = match value & 0b11 {
            0 => 1,
            1 => 64,
            2 => 256,
            3 => 1024,
            _ => 1,
        };
        self.cascade = (value >> 2) & 1 == 1;
        self.irq_enabled = (value >> 6) & 1 == 1;
        self.enabled = (value >> 7) & 1 == 1;

        //if transitioning from stopped to running, reload counter
        if !was_enabled && self.enabled {
            self.counter = self.reload;
            self.cycle_count = 0;
        }
    }
}
