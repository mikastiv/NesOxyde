use self::addr_modes::AddrMode;
use self::instructions::OPTABLE;

mod addr_modes;
mod instructions;

const STACK_PAGE: u16 = 0x0100;

pub trait Interface {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
}

bitflags! {
    struct Flags: u8 {
        const N = 0b10000000;
        const V = 0b01000000;
        const U = 0b00100000;
        const B = 0b00010000;
        const D = 0b00001000;
        const I = 0b00000100;
        const Z = 0b00000010;
        const C = 0b00000001;
    }
}

pub struct Cpu {
    a: u8,
    x: u8,
    y: u8,
    s: u8,
    p: Flags,
    pc: u16,

    bus: Box<dyn Interface>,
    ins_cycles: u32,
}

impl Cpu {
    pub fn new(bus: Box<dyn Interface>) -> Self {
        Self {
            a: 0,
            x: 0,
            y: 0,
            s: 0xFD,
            p: Flags::U,
            pc: 0,

            bus,

            ins_cycles: 0,
        }
    }

    pub fn reset(&mut self) {
        self.a = 0;
        self.x = 0;
        self.y = 0;
        self.s = 0xFD;
        self.p = Flags::U;
        self.pc = self.mem_read_word(0xFFFC);
    }

    pub fn execute(&mut self) -> u32 {
        let opcode = self.read_byte();

        let ins = *OPTABLE.get(&opcode).unwrap();
        self.ins_cycles = ins.cycles;
        (ins.cpu_fn)(self, ins.mode);

        self.ins_cycles
    }

    fn mem_read(&mut self, addr: u16) -> u8 {
        self.bus.read(addr)
    }

    fn mem_read_word(&mut self, addr: u16) -> u16 {
        let lo = self.mem_read(addr);
        let hi = self.mem_read(addr.wrapping_add(1));
        u16::from_le_bytes([lo, hi])
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.write(addr, data);
    }

    fn read_byte(&mut self) -> u8 {
        let b = self.mem_read(self.pc);
        self.increment_pc();
        b
    }

    fn read_word(&mut self) -> u16 {
        let lo = self.read_byte();
        let hi = self.read_byte();
        u16::from_le_bytes([lo, hi])
    }

    fn push_byte(&mut self, data: u8) {
        self.mem_write(STACK_PAGE + self.s as u16, data);
        self.s = self.s.wrapping_sub(1);
    }

    fn push_word(&mut self, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = data as u8;
        self.push_byte(hi);
        self.push_byte(lo);
    }

    fn pop_byte(&mut self) -> u8 {
        self.s = self.s.wrapping_add(1);
        self.mem_read(STACK_PAGE + self.s as u16)
    }

    fn pop_word(&mut self) -> u16 {
        let lo = self.pop_byte();
        let hi = self.pop_byte();
        u16::from_le_bytes([lo, hi])
    }

    fn operand_addr(&mut self, mode: AddrMode) -> u16 {
        match mode {
            AddrMode::None | AddrMode::IMP => panic!("Not supported"),
            AddrMode::IMM | AddrMode::REL => self.pc,
            AddrMode::ZP0 => self.read_byte() as u16,
            AddrMode::ZPX => {
                let base = self.read_byte();
                base.wrapping_add(self.x) as u16
            }
            AddrMode::ZPY => {
                let base = self.read_byte();
                base.wrapping_add(self.y) as u16
            }
            AddrMode::ABS | AddrMode::IND => self.read_word(),
            AddrMode::ABX => {
                let base = self.read_word();
                let addr = base.wrapping_add(self.x as u16);

                if Self::page_crossed(base, addr) {
                    self.ins_cycles += 1;
                }

                addr
            }
            AddrMode::ABXW => {
                let base = self.read_word();
                base.wrapping_add(self.x as u16)
            }
            AddrMode::ABY => {
                let base = self.read_word();
                let addr = base.wrapping_add(self.y as u16);

                if Self::page_crossed(base, addr) {
                    self.ins_cycles += 1;
                }

                addr
            }
            AddrMode::ABYW => {
                let base = self.read_word();
                base.wrapping_add(self.y as u16)
            }
            AddrMode::IZX => {
                let base = self.read_byte();
                let ptr = base.wrapping_add(self.x);
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                u16::from_le_bytes([lo, hi])
            }
            AddrMode::IZY => {
                let ptr = self.read_byte();
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                let addr = u16::from_le_bytes([lo, hi]).wrapping_add(self.y as u16);

                if Self::page_crossed(u16::from_le_bytes([lo, hi]), addr) {
                    self.ins_cycles += 1;
                }

                addr
            }
            AddrMode::IZYW => {
                let ptr = self.read_byte();
                let lo = self.mem_read(ptr as u16);
                let hi = self.mem_read(ptr.wrapping_add(1) as u16);
                u16::from_le_bytes([lo, hi]).wrapping_add(self.y as u16)
            }
        }
    }

    fn fetch_operand(&mut self, addr: u16, mode: AddrMode) -> u8 {
        match mode {
            AddrMode::None | AddrMode::IMP | AddrMode::IND => panic!("Not supported"),
            AddrMode::IMM | AddrMode::REL => self.read_byte(),
            _ => self.mem_read(addr),
        }
    }

    fn branch(&mut self, mode: AddrMode, cond: bool) {
        let addr = self.operand_addr(mode);
        let offset = self.fetch_operand(addr, mode) as i8;

        if cond {
            self.ins_cycles += 1;
            let new_addr = self.pc.wrapping_add(offset as u16);

            if Self::page_crossed(self.pc, new_addr) {
                self.ins_cycles += 1;
            }

            self.pc = new_addr;
        }
    }

    fn increment_pc(&mut self) {
        self.pc = self.pc.wrapping_add(1);
    }

    fn set_z_n(&mut self, v: u8) {
        self.p.set(Flags::Z, v == 0);
        self.p.set(Flags::N, v & 0x80 != 0);
    }

    fn set_a(&mut self, v: u8) {
        self.a = v;
        self.set_z_n(v);
    }

    fn set_x(&mut self, v: u8) {
        self.x = v;
        self.set_z_n(v);
    }

    fn set_y(&mut self, v: u8) {
        self.y = v;
        self.set_z_n(v);
    }

    fn set_p(&mut self, v: u8) {
        self.p.bits = (v | Flags::U.bits) & !Flags::B.bits;
    }

    fn page_crossed(old: u16, new: u16) -> bool {
        old & 0xFF00 != new & 0xFF00
    }

    fn wrap(old: u16, new: u16) -> u16 {
        (old & 0xFF00) | (new & 0x00FF)
    }

    fn lda(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_a(v);
    }

    fn ldx(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_x(v);
    }

    fn ldy(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_y(v);
    }

    fn sta(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        self.mem_write(addr, self.a);
    }

    fn stx(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        self.mem_write(addr, self.x);
    }

    fn sty(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        self.mem_write(addr, self.y);
    }

    fn tax(&mut self, _mode: AddrMode) {
        self.set_x(self.a);
    }

    fn tay(&mut self, _mode: AddrMode) {
        self.set_y(self.a);
    }

    fn tsx(&mut self, _mode: AddrMode) {
        self.set_x(self.s);
    }

    fn txa(&mut self, _mode: AddrMode) {
        self.set_a(self.x);
    }

    fn txs(&mut self, _mode: AddrMode) {
        self.s = self.x;
    }

    fn tya(&mut self, _mode: AddrMode) {
        self.set_a(self.y);
    }

    fn clc(&mut self, _mode: AddrMode) {
        self.p.remove(Flags::C);
    }

    fn cld(&mut self, _mode: AddrMode) {
        self.p.remove(Flags::D);
    }

    fn cli(&mut self, _mode: AddrMode) {
        self.p.remove(Flags::I);
    }

    fn clv(&mut self, _mode: AddrMode) {
        self.p.remove(Flags::V);
    }

    fn sec(&mut self, _mode: AddrMode) {
        self.p.insert(Flags::C);
    }

    fn sed(&mut self, _mode: AddrMode) {
        self.p.insert(Flags::D);
    }

    fn sei(&mut self, _mode: AddrMode) {
        self.p.insert(Flags::I);
    }

    fn inc(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode).wrapping_add(1);
        self.set_z_n(v);
        self.mem_write(addr, v);
    }

    fn inx(&mut self, mode: AddrMode) {
        self.set_x(self.x.wrapping_add(1));
    }

    fn iny(&mut self, mode: AddrMode) {
        self.set_y(self.y.wrapping_add(1));
    }

    fn dec(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode).wrapping_sub(1);
        self.set_z_n(v);
        self.mem_write(addr, v);
    }

    fn dex(&mut self, mode: AddrMode) {
        self.set_x(self.x.wrapping_sub(1));
    }

    fn dey(&mut self, mode: AddrMode) {
        self.set_y(self.y.wrapping_sub(1));
    }

    fn cmp(&mut self, v1: u8, v2: u8) {
        let result = v1.wrapping_sub(v2);
        self.p.set(Flags::C, v1 >= v2);
        self.set_z_n(result);
    }

    fn cpa(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.cmp(self.a, v);
    }

    fn cpx(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.cmp(self.x, v);
    }

    fn cpy(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.cmp(self.y, v);
    }

    fn bcs(&mut self, mode: AddrMode) {
        self.branch(mode, self.p.contains(Flags::C));
    }

    fn bcc(&mut self, mode: AddrMode) {
        self.branch(mode, !self.p.contains(Flags::C));
    }

    fn beq(&mut self, mode: AddrMode) {
        self.branch(mode, self.p.contains(Flags::Z));
    }

    fn bne(&mut self, mode: AddrMode) {
        self.branch(mode, !self.p.contains(Flags::Z));
    }

    fn bmi(&mut self, mode: AddrMode) {
        self.branch(mode, self.p.contains(Flags::N));
    }

    fn bpl(&mut self, mode: AddrMode) {
        self.branch(mode, !self.p.contains(Flags::N));
    }

    fn bvs(&mut self, mode: AddrMode) {
        self.branch(mode, self.p.contains(Flags::V));
    }

    fn bvc(&mut self, mode: AddrMode) {
        self.branch(mode, !self.p.contains(Flags::V));
    }

    fn jmp_abs(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        self.pc = addr;
    }

    fn jmp_ind(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let lo = self.mem_read(addr);
        let hi = self.mem_read(Self::wrap(addr, addr.wrapping_add(1)));
        self.pc = u16::from_le_bytes([lo, hi]);
    }

    fn pha(&mut self, _mode: AddrMode) {
        self.push_byte(self.a);
    }

    fn php(&mut self, _mode: AddrMode) {
        self.push_byte(self.p.bits | Flags::B.bits);
    }

    fn pla(&mut self, _mode: AddrMode) {
        let v = self.pop_byte();
        self.set_a(v);
    }

    fn plp(&mut self, _mode: AddrMode) {
        let v = self.pop_byte();
        self.set_p(v);
    }

    fn jsr(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        self.push_word(self.pc.wrapping_sub(1));
        self.pc = addr;
    }

    fn rts(&mut self, _mode: AddrMode) {
        let addr = self.pop_word();
        self.pc = addr.wrapping_add(1);
    }

    fn rti(&mut self, _mode: AddrMode) {
        let v = self.pop_byte();
        let addr = self.pop_word();
        self.set_p(v);
        self.pc = addr;
    }

    fn nop(&mut self, mode: AddrMode) {
        match mode {
            AddrMode::IMP => {}
            _ => {
                let addr = self.operand_addr(mode);
                self.fetch_operand(addr, mode);
            }
        }
    }

    fn bit(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);

        self.p.set(Flags::Z, self.a & v == 0);
        self.p.set(Flags::V, v & 0x40 != 0);
        self.p.set(Flags::N, v & 0x80 != 0);
    }

    fn and(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_a(self.a & v);
    }

    fn eor(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_a(self.a ^ v);
    }

    fn ora(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.set_a(self.a | v);
    }

    fn asl(&mut self, v: u8) -> u8 {
        self.p.set(Flags::C, v & 0x80 != 0);
        let result = v << 1;
        self.set_z_n(result);
        result
    }

    fn asl_acc(&mut self, _mode: AddrMode) {
        let v = self.asl(self.a);
        self.set_a(v);
    }

    fn asl_mem(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        let result = self.asl(v);
        self.mem_write(addr, result);
    }

    fn lsr(&mut self, v: u8) -> u8 {
        self.p.set(Flags::C, v & 0x01 != 0);
        let result = v >> 1;
        self.set_z_n(result);
        result
    }

    fn lsr_acc(&mut self, _mode: AddrMode) {
        let v = self.lsr(self.a);
        self.set_a(v);
    }

    fn lsr_mem(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        let result = self.lsr(v);
        self.mem_write(addr, result);
    }

    fn rol(&mut self, v: u8) -> u8 {
        let c = self.p.contains(Flags::C) as u8;
        self.p.set(Flags::C, v & 0x80 != 0);

        let result = (v << 1) | c;
        self.set_z_n(result);
        result
    }

    fn rol_acc(&mut self, _mode: AddrMode) {
        let v = self.rol(self.a);
        self.set_a(v);
    }

    fn rol_mem(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        let result = self.rol(v);
        self.mem_write(addr, result);
    }

    fn ror(&mut self, v: u8) -> u8 {
        let c = self.p.contains(Flags::C) as u8;
        self.p.set(Flags::C, v & 0x01 != 0);

        let result = (c << 7) | (v >> 1);
        self.set_z_n(result);
        result
    }

    fn ror_acc(&mut self, _mode: AddrMode) {
        let v = self.ror(self.a);
        self.set_a(v);
    }

    fn ror_mem(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        let result = self.ror(v);
        self.mem_write(addr, result);
    }

    fn add(&mut self, v: u8) {
        let c = self.p.contains(Flags::C) as u16;
        let sum = self.a as u16 + v as u16 + c;
        let result = sum as u8;

        self.p
            .set(Flags::V, (v ^ result) & (result ^ self.a) & 0x80 != 0);
        self.p.set(Flags::C, sum > 0xFF);
        self.set_a(result);
    }

    fn sub(&mut self, v: u8) {
        self.add(((v as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    fn adc(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.add(v);
    }

    fn sbc(&mut self, mode: AddrMode) {
        let addr = self.operand_addr(mode);
        let v = self.fetch_operand(addr, mode);
        self.sub(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::bus::TestBus;

    fn get_test_cpu(program: Vec<u8>, ram: Vec<u8>) -> Cpu {
        let mut bus = Box::new(TestBus::new(program));
        for (addr, data) in ram.iter().enumerate() {
            bus.set_ram(addr as u16, *data);
        }
        let mut cpu = Cpu::new(bus);
        cpu.pc = 0x2000;
        cpu
    }

    fn get_test_cpu_from_bus(bus: TestBus) -> Cpu {
        let mut cpu = Cpu::new(Box::new(bus));
        cpu.pc = 0x2000;
        cpu
    }

    #[test]
    fn test_a9() {
        let mut cpu = get_test_cpu(vec![0xA9, 0x05], vec![0]);
        cpu.execute();

        assert_eq!(cpu.a, 0x05);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA9, 0x00], vec![0]);
        cpu.execute();

        assert_eq!(cpu.a, 0x00);
        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA9, 0x80], vec![0]);
        cpu.execute();

        assert_eq!(cpu.a, 0x80);
        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
    }

    #[test]
    fn test_a5() {
        let mut cpu = get_test_cpu(vec![0xA5, 0x02], vec![0x00, 0x00, 0x23]);
        cpu.execute();

        assert_eq!(cpu.a, 0x23);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA5, 0x00], vec![0x00]);
        cpu.execute();

        assert_eq!(cpu.a, 0x00);
        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA5, 0x00], vec![0x85]);
        cpu.execute();

        assert_eq!(cpu.a, 0x85);
        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_b5() {
        let mut bus = TestBus::new(vec![0xB5, 0x00]);
        bus.set_ram(0xFF, 0x50);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.x = 0xFF;
        cpu.execute();

        assert_eq!(cpu.a, 0x50);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xB5, 0x01], vec![0x50]);

        cpu.x = 0xFF;
        cpu.execute();

        assert_eq!(cpu.a, 0x50);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_ad() {
        let mut bus = TestBus::new(vec![0xAD, 0x05, 0x02]);
        bus.set_ram(0x0205, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_bd() {
        let mut bus = TestBus::new(vec![0xBD, 0x05, 0x02]);
        bus.set_ram(0x020A, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.x = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 4);

        let mut bus = TestBus::new(vec![0xBD, 0xFF, 0x05]);
        bus.set_ram(0x0604, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.x = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_b9() {
        let mut bus = TestBus::new(vec![0xB9, 0x05, 0x02]);
        bus.set_ram(0x020A, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 4);

        let mut bus = TestBus::new(vec![0xB9, 0xFF, 0x05]);
        bus.set_ram(0x0604, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_a1() {
        let mut bus = TestBus::new(vec![0xA1, 0x05]);
        bus.set_ram(0x0A, 0x00);
        bus.set_ram(0x0B, 0x02);
        bus.set_ram(0x0200, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.x = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 6);
    }

    #[test]
    fn test_b1() {
        let mut bus = TestBus::new(vec![0xB1, 0x05]);
        bus.set_ram(0x05, 0x00);
        bus.set_ram(0x06, 0x02);
        bus.set_ram(0x0205, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 5;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 5);

        let mut bus = TestBus::new(vec![0xB1, 0x05]);
        bus.set_ram(0x05, 0xFF);
        bus.set_ram(0x06, 0x02);
        bus.set_ram(0x0300, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 1;
        cpu.execute();

        assert_eq!(cpu.a, 0xFE);
        assert_eq!(cpu.ins_cycles, 6);
    }

    #[test]
    fn test_a2() {
        let mut cpu = get_test_cpu(vec![0xA2, 0x05], vec![0]);
        cpu.execute();

        assert_eq!(cpu.x, 0x05);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA2, 0x00], vec![0]);
        cpu.execute();

        assert_eq!(cpu.x, 0x00);
        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA2, 0x80], vec![0]);
        cpu.execute();

        assert_eq!(cpu.x, 0x80);
        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
    }

    #[test]
    fn test_a6() {
        let mut cpu = get_test_cpu(vec![0xA6, 0x02], vec![0x00, 0x00, 0x23]);
        cpu.execute();

        assert_eq!(cpu.x, 0x23);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA6, 0x00], vec![0x00]);
        cpu.execute();

        assert_eq!(cpu.x, 0x00);
        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA6, 0x00], vec![0x85]);
        cpu.execute();

        assert_eq!(cpu.x, 0x85);
        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_b6() {
        let mut bus = TestBus::new(vec![0xB6, 0x00]);
        bus.set_ram(0xFF, 0x50);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 0xFF;
        cpu.execute();

        assert_eq!(cpu.x, 0x50);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xB6, 0x01], vec![0x50]);

        cpu.y = 0xFF;
        cpu.execute();

        assert_eq!(cpu.x, 0x50);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_ae() {
        let mut bus = TestBus::new(vec![0xAE, 0x05, 0x02]);
        bus.set_ram(0x0205, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.execute();

        assert_eq!(cpu.x, 0xFE);
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_be() {
        let mut bus = TestBus::new(vec![0xBE, 0x05, 0x02]);
        bus.set_ram(0x020A, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 5;
        cpu.execute();

        assert_eq!(cpu.x, 0xFE);
        assert_eq!(cpu.ins_cycles, 4);

        let mut bus = TestBus::new(vec![0xBE, 0xFF, 0x05]);
        bus.set_ram(0x0604, 0xFE);
        let mut cpu = get_test_cpu_from_bus(bus);

        cpu.y = 5;
        cpu.execute();

        assert_eq!(cpu.x, 0xFE);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_a0() {
        let mut cpu = get_test_cpu(vec![0xA0, 0x05], vec![0]);
        cpu.execute();

        assert_eq!(cpu.y, 0x05);
        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA0, 0x00], vec![0]);
        cpu.execute();

        assert_eq!(cpu.y, 0x00);
        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));

        let mut cpu = get_test_cpu(vec![0xA0, 0x80], vec![0]);
        cpu.execute();

        assert_eq!(cpu.y, 0x80);
        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
    }

    #[test]
    fn test_85() {
        let mut cpu = get_test_cpu(vec![0x85, 0x03], vec![]);
        cpu.a = 0xDE;
        cpu.execute();

        assert_eq!(cpu.mem_read(0x03), 0xDE);
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_9d() {
        let mut cpu = get_test_cpu(vec![0x9D, 0x03, 0x04], vec![]);
        cpu.a = 0xDE;
        cpu.x = 0x0A;
        cpu.execute();

        assert_eq!(cpu.mem_read(0x040D), 0xDE);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_86() {
        let mut cpu = get_test_cpu(vec![0x86, 0x03], vec![]);
        cpu.x = 0xDE;
        cpu.execute();

        assert_eq!(cpu.mem_read(0x03), 0xDE);
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_84() {
        let mut cpu = get_test_cpu(vec![0x84, 0x03], vec![]);
        cpu.y = 0xDE;
        cpu.execute();

        assert_eq!(cpu.mem_read(0x03), 0xDE);
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_aa() {
        let mut cpu = get_test_cpu(vec![0xAA], vec![]);
        cpu.a = 0x20;
        cpu.execute();

        assert_eq!(cpu.x, cpu.a);
        assert_eq!(cpu.x, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_a8() {
        let mut cpu = get_test_cpu(vec![0xA8], vec![]);
        cpu.a = 0x20;
        cpu.execute();

        assert_eq!(cpu.y, cpu.a);
        assert_eq!(cpu.y, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_ba() {
        let mut cpu = get_test_cpu(vec![0xBA], vec![]);
        cpu.s = 0x20;
        cpu.execute();

        assert_eq!(cpu.x, cpu.s);
        assert_eq!(cpu.x, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_8a() {
        let mut cpu = get_test_cpu(vec![0x8A], vec![]);
        cpu.x = 0x20;
        cpu.execute();

        assert_eq!(cpu.a, cpu.x);
        assert_eq!(cpu.a, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_9a() {
        let mut cpu = get_test_cpu(vec![0x9A], vec![]);
        cpu.x = 0x20;
        cpu.execute();

        assert_eq!(cpu.s, cpu.x);
        assert_eq!(cpu.s, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_98() {
        let mut cpu = get_test_cpu(vec![0x98], vec![]);
        cpu.y = 0x20;
        cpu.execute();

        assert_eq!(cpu.a, cpu.y);
        assert_eq!(cpu.a, 0x20);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_18() {
        let mut cpu = get_test_cpu(vec![0x18], vec![]);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_d8() {
        let mut cpu = get_test_cpu(vec![0xD8], vec![]);
        cpu.p.insert(Flags::D);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::D));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_58() {
        let mut cpu = get_test_cpu(vec![0x58], vec![]);
        cpu.p.insert(Flags::I);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::I));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_b8() {
        let mut cpu = get_test_cpu(vec![0xB8], vec![]);
        cpu.p.insert(Flags::V);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::V));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_38() {
        let mut cpu = get_test_cpu(vec![0x38], vec![]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::C));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_f8() {
        let mut cpu = get_test_cpu(vec![0xF8], vec![]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::D));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_78() {
        let mut cpu = get_test_cpu(vec![0x78], vec![]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::I));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_e6() {
        let mut cpu = get_test_cpu(vec![0xE6, 0x01], vec![0x00, 0xFE]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.mem_read(0x01), 0xFF);
        assert_eq!(cpu.ins_cycles, 5);

        let mut cpu = get_test_cpu(vec![0xE6, 0x01], vec![0x00, 0xFF]);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.mem_read(0x01), 0x00);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_e8() {
        let mut cpu = get_test_cpu(vec![0xE8], vec![]);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.x, 0x01);
        assert_eq!(cpu.ins_cycles, 2);

        let mut cpu = get_test_cpu(vec![0xE8], vec![]);
        cpu.x = 0xFF;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.x, 0x00);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_c8() {
        let mut cpu = get_test_cpu(vec![0xC8], vec![]);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.y, 0x01);
        assert_eq!(cpu.ins_cycles, 2);

        let mut cpu = get_test_cpu(vec![0xC8], vec![]);
        cpu.y = 0xFF;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.y, 0x00);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_c6() {
        let mut cpu = get_test_cpu(vec![0xC6, 0x01], vec![0x00, 0x00]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.mem_read(0x01), 0xFF);
        assert_eq!(cpu.ins_cycles, 5);

        let mut cpu = get_test_cpu(vec![0xC6, 0x01], vec![0x00, 0x01]);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.mem_read(0x01), 0x00);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_ca() {
        let mut cpu = get_test_cpu(vec![0xCA], vec![]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.x, 0xFF);
        assert_eq!(cpu.ins_cycles, 2);

        let mut cpu = get_test_cpu(vec![0xCA], vec![]);
        cpu.x = 0x01;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.x, 0x00);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_88() {
        let mut cpu = get_test_cpu(vec![0x88], vec![]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.y, 0xFF);
        assert_eq!(cpu.ins_cycles, 2);

        let mut cpu = get_test_cpu(vec![0x88], vec![]);
        cpu.y = 0x01;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.y, 0x00);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_c9() {
        let mut cpu = get_test_cpu(vec![0xC9, 0x05], vec![]);
        cpu.a = 0x05;
        cpu.execute();

        assert!(cpu.p.contains(Flags::C));
        assert!(cpu.p.contains(Flags::Z));
        assert!(!cpu.p.contains(Flags::N));

        let mut cpu = get_test_cpu(vec![0xC9, 0x0A], vec![]);
        cpu.a = 0x05;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert!(!cpu.p.contains(Flags::Z));
        assert!(cpu.p.contains(Flags::N));
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_e4() {
        let mut bus = TestBus::new(vec![0xE4, 0x05]);
        bus.set_ram(0x05, 0x0A);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.x = 0x05;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert!(!cpu.p.contains(Flags::Z));
        assert!(cpu.p.contains(Flags::N));
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_cc() {
        let mut bus = TestBus::new(vec![0xCC, 0x05, 0x03]);
        bus.set_ram(0x0305, 0x0A);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.y = 0x05;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert!(!cpu.p.contains(Flags::Z));
        assert!(cpu.p.contains(Flags::N));
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_90() {
        let mut cpu = get_test_cpu(vec![0x90, 0x05], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0x90, !0x05 + 1], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 - 0x05);
        assert_eq!(cpu.ins_cycles, 4);

        let mut cpu = get_test_cpu(vec![0x90, !0x05 + 1], vec![]);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_b0() {
        let mut cpu = get_test_cpu(vec![0xB0, 0x05], vec![]);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0xB0, !0x05 + 1], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_f0() {
        let mut cpu = get_test_cpu(vec![0xF0, 0x05], vec![]);
        cpu.p.insert(Flags::Z);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0xF0, !0x05 + 1], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_d0() {
        let mut cpu = get_test_cpu(vec![0xD0, 0x05], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0xD0, !0x05 + 1], vec![]);
        cpu.p.insert(Flags::Z);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_30() {
        let mut cpu = get_test_cpu(vec![0x30, 0x05], vec![]);
        cpu.p.insert(Flags::N);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0x30, !0x05 + 1], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_10() {
        let mut cpu = get_test_cpu(vec![0x10, 0x05], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0x10, !0x05 + 1], vec![]);
        cpu.p.insert(Flags::N);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_50() {
        let mut cpu = get_test_cpu(vec![0x50, 0x05], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0x50, !0x05 + 1], vec![]);
        cpu.p.insert(Flags::V);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_70() {
        let mut cpu = get_test_cpu(vec![0x70, 0x05], vec![]);
        cpu.p.insert(Flags::V);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002 + 0x05);
        assert_eq!(cpu.ins_cycles, 3);

        let mut cpu = get_test_cpu(vec![0x70, !0x05 + 1], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2002);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_4c() {
        let mut cpu = get_test_cpu(vec![0x4C, 0x34, 0x02], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x0234);
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_6c() {
        let mut bus = TestBus::new(vec![0x6C, 0x05, 0x03]);
        bus.set_ram(0x0305, 0x0A);
        bus.set_ram(0x0306, 0x06);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert_eq!(cpu.pc, 0x060A);

        // wrap bug test
        let mut bus = TestBus::new(vec![0x6C, 0xFF, 0x10]);
        bus.set_ram(0x10FF, 0x0A);
        bus.set_ram(0x1000, 0x06);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert_eq!(cpu.pc, 0x060A);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_48() {
        let mut cpu = get_test_cpu(vec![0x48], vec![]);
        cpu.a = 0x93;
        cpu.execute();

        assert_eq!(
            cpu.mem_read(STACK_PAGE + cpu.s.wrapping_add(1) as u16),
            0x93
        );
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_08() {
        let mut cpu = get_test_cpu(vec![0x08], vec![]);
        cpu.p.insert(Flags::N);
        cpu.p.insert(Flags::V);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert_eq!(
            cpu.mem_read(STACK_PAGE + cpu.s.wrapping_add(1) as u16),
            Flags::N.bits | Flags::V.bits | Flags::U.bits | Flags::B.bits | Flags::C.bits
        );
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_68() {
        let mut bus = TestBus::new(vec![0x68]);
        bus.set_ram(STACK_PAGE + 0xA5, 0x0A);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.s = 0xA4;
        cpu.execute();

        assert_eq!(cpu.a, 0x0A);
        assert_eq!(cpu.ins_cycles, 4);

        let mut cpu = get_test_cpu(vec![0x68], vec![]);
        cpu.execute();

        assert_eq!(cpu.a, 0x00);
        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_28() {
        let mut bus = TestBus::new(vec![0x28]);
        bus.set_ram(
            STACK_PAGE + 0xA5,
            Flags::N.bits | Flags::B.bits | Flags::I.bits,
        );
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.s = 0xA4;
        cpu.execute();

        assert_eq!(cpu.p, Flags::N | Flags::U | Flags::I);
        assert_eq!(cpu.ins_cycles, 4);
    }

    #[test]
    fn test_20() {
        let mut cpu = get_test_cpu(vec![0x20, 0x63, 0x05], vec![]);
        cpu.execute();

        assert_eq!(cpu.mem_read_word(STACK_PAGE + cpu.s as u16 + 1), 0x2002);
        assert_eq!(cpu.pc, 0x0563);
        assert_eq!(cpu.ins_cycles, 6);
    }

    #[test]
    fn test_60() {
        let mut bus = TestBus::new(vec![0x60]);
        bus.set_ram(STACK_PAGE + 0xFE, 0xEF);
        bus.set_ram(STACK_PAGE + 0xFF, 0xBE);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert_eq!(cpu.pc, 0xBEEF + 1);
        assert_eq!(cpu.ins_cycles, 6);
    }

    #[test]
    fn test_40() {
        let mut bus = TestBus::new(vec![0x40]);
        bus.set_ram(STACK_PAGE + 0xFE, Flags::V.bits | Flags::C.bits);
        bus.set_ram(STACK_PAGE + 0xFF, 0xEF);
        bus.set_ram(STACK_PAGE, 0xBE);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert_eq!(cpu.pc, 0xBEEF);
        assert_eq!(cpu.p, Flags::V | Flags::U | Flags::C);
        assert_eq!(cpu.ins_cycles, 6);
    }

    #[test]
    fn test_ea() {
        let mut cpu = get_test_cpu(vec![0xEA], vec![]);
        cpu.execute();

        assert_eq!(cpu.pc, 0x2001);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_24() {
        let mut bus = TestBus::new(vec![0x24, 0xFE]);
        bus.set_ram(0xFE, 0b0010_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.a = 0b1101_1001;
        cpu.execute();

        assert!(cpu.p.contains(Flags::Z));
        assert_eq!(cpu.ins_cycles, 3);

        let mut bus = TestBus::new(vec![0x24, 0xFE]);
        bus.set_ram(0xFE, 0b1100_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.a = 0b1101_1001;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::Z));
        assert!(cpu.p.contains(Flags::V));
        assert!(cpu.p.contains(Flags::N));
        assert_eq!(cpu.ins_cycles, 3);
    }

    #[test]
    fn test_29() {
        let mut cpu = get_test_cpu(vec![0x29, 0x8E], vec![]);
        cpu.a = 0x3C;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.a, 0x3C & 0x8E);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_49() {
        let mut cpu = get_test_cpu(vec![0x49, 0x8E], vec![]);
        cpu.a = 0x3C;
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.a, 0x3C ^ 0x8E);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_09() {
        let mut cpu = get_test_cpu(vec![0x09, 0x8E], vec![]);
        cpu.a = 0x3C;
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.a, 0x3C | 0x8E);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_0a() {
        let mut cpu = get_test_cpu(vec![0x0A], vec![]);
        cpu.a = 0xC1;
        cpu.execute();

        assert!(cpu.p.contains(Flags::C));
        assert_eq!(cpu.a, 0xC1 << 1);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_06() {
        let mut cpu = get_test_cpu(vec![0x06, 0x00], vec![0x67]);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert_eq!(cpu.mem_read(0x00), 0x67 << 1);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_4a() {
        let mut cpu = get_test_cpu(vec![0x4A], vec![]);
        cpu.a = 0xC0;
        cpu.execute();

        assert!(!cpu.p.contains(Flags::C));
        assert_eq!(cpu.a, 0xC1 >> 1);
        assert_eq!(cpu.ins_cycles, 2);
    }

    #[test]
    fn test_46() {
        let mut cpu = get_test_cpu(vec![0x46, 0x00], vec![0x67]);
        cpu.execute();

        assert!(cpu.p.contains(Flags::C));
        assert_eq!(cpu.mem_read(0x00), 0x67 >> 1);
        assert_eq!(cpu.ins_cycles, 5);
    }

    #[test]
    fn test_2a() {
        let mut cpu = get_test_cpu(vec![0x2A], vec![]);
        cpu.a = 0b0100_0010;
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::Z | Flags::C));
        assert_eq!(cpu.a, 0b1000_0101);
    }

    #[test]
    fn test_2e() {
        let mut bus = TestBus::new(vec![0x2E, 0xFE, 0x10]);
        bus.set_ram(0x10FE, 0b1001_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert!(cpu.p.contains(Flags::C));
        assert!(!cpu.p.contains(Flags::N | Flags::Z));
        assert_eq!(cpu.mem_read(0x10FE), 0b0010_1101);

        let mut bus = TestBus::new(vec![0x2E, 0xFE, 0x10]);
        bus.set_ram(0x10FE, 0b0001_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N | Flags::Z | Flags::C));
        assert_eq!(cpu.mem_read(0x10FE), 0b0010_1100);
    }

    #[test]
    fn test_6a() {
        let mut cpu = get_test_cpu(vec![0x6A], vec![]);
        cpu.a = 0b0100_0011;
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N | Flags::C));
        assert!(!cpu.p.contains(Flags::Z));
        assert_eq!(cpu.a, 0b1010_0001);
    }

    #[test]
    fn test_6e() {
        let mut bus = TestBus::new(vec![0x6E, 0xFE, 0x10]);
        bus.set_ram(0x10FE, 0b1001_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.p.insert(Flags::C);
        cpu.execute();

        assert!(cpu.p.contains(Flags::N));
        assert!(!cpu.p.contains(Flags::C | Flags::Z));
        assert_eq!(cpu.mem_read(0x10FE), 0b1100_1011);

        let mut bus = TestBus::new(vec![0x6E, 0xFE, 0x10]);
        bus.set_ram(0x10FE, 0b0001_0110);
        let mut cpu = get_test_cpu_from_bus(bus);
        cpu.execute();

        assert!(!cpu.p.contains(Flags::N | Flags::Z | Flags::C));
        assert_eq!(cpu.mem_read(0x10FE), 0b0000_1011);
    }
}
