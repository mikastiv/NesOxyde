use crate::cpu::Interface;
use crate::joypad::{Button, JoyPort};

pub struct TestBus {
    ram: [u8; 0x800],
    program: Vec<u8>,
}

impl Interface for TestBus {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x7FF) as usize],
            _ => self.program[(addr - 0x2000) as usize],
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x1FFF => self.ram[(addr & 0x7FF) as usize] = data,
            _ => self.program[(addr - 0x2000) as usize] = data,
        }
    }

    fn poll_nmi(&mut self) -> bool {
        false
    }

    fn tick(&mut self, cycles: u64) {}
    
    fn update_joypad(&mut self, button: Button, pressed: bool, port: JoyPort) {}
}

impl TestBus {
    pub fn new(program: Vec<u8>) -> Self {
        Self {
            ram: [0; 0x800],
            program,
        }
    }

    pub fn set_ram(&mut self, addr: u16, data: u8) {
        self.ram[(addr & 0x7FF) as usize] = data;
    }
}
