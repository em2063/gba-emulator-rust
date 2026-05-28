impl CPU {
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
}
