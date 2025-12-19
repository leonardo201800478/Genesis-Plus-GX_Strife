//! Mapeadores MSX

use super::*;

/// Mapeador MSX de 8KB
pub struct Msx8kMapper {
    banks: [u8; 4],
    rom: Vec<u8>,
    pages: u16,
    is_nemesis: bool,
}

impl Msx8kMapper {
    pub fn new(rom: Vec<u8>, is_nemesis: bool) -> Self {
        let pages = ((rom.len() + (1 << 13) - 1) >> 13) as u16;
        
        Self {
            banks: [0; 4],
            rom,
            pages,
            is_nemesis,
        }
    }
}

impl Mapper for Msx8kMapper {
    fn read(&self, address: u16) -> u8 {
        let bank_idx = match address {
            0x0000..=0x1FFF => 0,
            0x2000..=0x3FFF => 1,
            0x4000..=0x5FFF => 2,
            0x6000..=0x7FFF => 3,
            0x8000..=0x9FFF => 0,
            0xA000..=0xBFFF => 1,
            _ => return 0xFF,
        };
        
        // Nemesis: primeiro banco mapeado para última página
        let bank = if self.is_nemesis && bank_idx == 0 {
            (self.pages - 1) as u8
        } else {
            self.banks[bank_idx]
        };
        
        let bank_usize = bank as usize % self.pages as usize;
        let offset = (address & 0x1FFF) as usize;
        
        self.rom[(bank_usize << 13) | offset]
    }
    
    fn write(&mut self, address: u16, value: u8) {
        if address <= 0x0003 {
            let idx = address as usize;
            self.banks[idx] = value;
        }
    }
    
    fn reset(&mut self) {
        self.banks = [0; 4];
    }
}

/// Mapeador MSX de 16KB
pub struct Msx16kMapper {
    bank: u8,
    rom: Vec<u8>,
    pages: u16,
}

impl Msx16kMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            bank: 0,
            rom,
            pages,
        }
    }
}

impl Mapper for Msx16kMapper {
    fn read(&self, address: u16) -> u8 {
        let bank = self.bank as usize % self.pages as usize;
        
        match address {
            0x0000..=0x3FFF => {
                let offset = address as usize;
                self.rom[(bank << 14) | offset]
            }
            0x4000..=0x7FFF => {
                let offset = (address - 0x4000) as usize;
                self.rom[(bank << 14) | offset]
            }
            0x8000..=0xBFFF => {
                let offset = (address - 0x8000) as usize;
                self.rom[(bank << 14) | offset]
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        if (address & 0xC000) == 0x8000 {
            self.bank = value;
        }
    }
    
    fn reset(&mut self) {
        self.bank = 0;
    }
}
