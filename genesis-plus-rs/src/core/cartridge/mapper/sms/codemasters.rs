//! Mapeador Codemasters

use super::*;

pub struct CodemastersMapper {
    banks: [u8; 3],
    rom: Vec<u8>,
    pages: u16,
    ram_enabled: bool,
    ram: [u8; 0x2000],
}

impl CodemastersMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            banks: [0, 0, 0],
            rom,
            pages,
            ram_enabled: false,
            ram: [0; 0x2000],
        }
    }
}

impl Mapper for CodemastersMapper {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                let bank = self.banks[0] as usize % self.pages as usize;
                let offset = address as usize;
                self.rom[(bank << 14) | offset]
            }
            0x4000..=0x7FFF => {
                let bank = self.banks[1] as usize % self.pages as usize;
                let offset = (address - 0x4000) as usize;
                self.rom[(bank << 14) | offset]
            }
            0x8000..=0xBFFF => {
                if self.ram_enabled && address >= 0xA000 {
                    // Ler da RAM
                    let ram_addr = (address - 0xA000) as usize;
                    self.ram[ram_addr & 0x1FFF]
                } else {
                    let bank = self.banks[2] as usize % self.pages as usize;
                    let offset = (address - 0x8000) as usize;
                    self.rom[(bank << 14) | offset]
                }
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x0000 => {
                self.banks[0] = value;
            }
            0x4000 => {
                self.banks[1] = value;
                self.ram_enabled = (value & 0x80) != 0;
            }
            0x8000 => {
                self.banks[2] = value;
            }
            0xA000..=0xBFFF if self.ram_enabled => {
                let ram_addr = (address - 0xA000) as usize;
                self.ram[ram_addr & 0x1FFF] = value;
            }
            _ => {}
        }
    }
    
    fn reset(&mut self) {
        self.banks = [0, 0, 0];
        self.ram_enabled = false;
        self.ram = [0; 0x2000];
    }
}