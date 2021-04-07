use core::panic;
use std::fmt::Display;
use std::io;
use std::path::Path;

use self::mapper::{Mapper, Mapper0};
use self::rom::{INesHeader, Rom};

mod mapper;
mod rom;

pub enum MirrorMode {
    Vertical,
    Horizontal,
}

pub struct Cartridge {
    header: INesHeader,
    mapper: Box<dyn Mapper>,
}

impl Cartridge {
    pub fn new<P: AsRef<Path> + Display>(romfile: P) -> io::Result<Self> {
        let (rom, header) = Rom::new(romfile)?;
        let mapper = match header.mapper_id() {
            0 => Mapper0::new(rom),
            _ => panic!("Unimplemented mapper: {}", header.mapper_id()),
        };

        Ok(Self {
            header,
            mapper: Box::new(mapper),
        })
    }

    pub fn read_prg(&mut self, addr: u16) -> u8 {
        self.mapper.read_prg(addr)
    }

    pub fn write_prg(&mut self, addr: u16, data: u8) {
        self.mapper.write_prg(addr, data);
    }

    pub fn read_chr(&mut self, addr: u16) -> u8 {
        self.mapper.read_chr(addr)
    }

    pub fn write_chr(&mut self, addr: u16, data: u8) {
        self.mapper.write_chr(addr, data);
    }
}
