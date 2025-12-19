//! Mapeadores multicart

use super::*;

/// Mapeador Multi 16KB
pub struct Multi16kMapper {
    banks: [u8; 3],
    rom: Vec<u8>,
    pages: u16,
}

impl Multi16kMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            banks: [0; 3],
            rom,
            pages,
        }
    }
}

impl Mapper for Multi16kMapper {
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
                let bank = ((self.banks[0] & 0x30) | self.banks[2]) as usize % self.pages as usize;
                let offset = (address - 0x8000) as usize;
                self.rom[(bank << 14) | offset]
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x3FFE => self.banks[0] = value,
            0x7FFF => self.banks[1] = value,
            0xBFFF => self.banks[2] = ((self.banks[0] & 0x30) | value) & 0x3F,
            _ => {}
        }
    }
    
    fn reset(&mut self) {
        self.banks = [0; 3];
    }
}

/// Mapeador Multi 2x16KB (Versão 1)
pub struct Multi2x16kV1Mapper {
    config: u8,
    banks: [u8; 2],
    rom: Vec<u8>,
    pages: u16,
}

impl Multi2x16kV1Mapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            config: 0,
            banks: [0, 0],
            rom,
            pages,
        }
    }
}

impl Mapper for Multi2x16kV1Mapper {
    fn read(&self, address: u16) -> u8 {
        if self.config != 0x01 {
            if (0x8000..=0xBFFF).contains(&address) {
                return 0xFF; // Área não mapeada
            }
        }
        
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
                // Mirror baseado na configuração
                if address < 0xA000 {
                    // $8000-$9FFF mirror de $6000-$7FFF
                    let offset = (address - 0x6000) as usize;
                    self.rom[(self.banks[1] as usize % self.pages as usize) << 14 | offset]
                } else {
                    // $A000-$BFFF mirror de $4000-$5FFF
                    let offset = (address - 0xA000) as usize;
                    self.rom[(self.banks[1] as usize % self.pages as usize) << 14 | offset]
                }
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        if address == 0xFFFE {
            self.config = (value >> 5) & 0x03;
            
            if self.config & 0x02 != 0 {
                let base = value & 0x1E;
                self.banks[0] = base;
                self.banks[1] = base + 1;
            } else {
                self.banks[0] = 0x00;
                self.banks[1] = value & 0x1F;
            }
        }
    }
    
    fn reset(&mut self) {
        self.config = 0;
        self.banks = [0, 0];
    }
}

/// Mapeador Multi 8KB
pub struct Multi8kMapper {
    banks: [u8; 4],
    rom: Vec<u8>,
    pages: u16,
}

impl Multi8kMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 13) - 1) >> 13) as u16;
        
        Self {
            banks: [0; 4],
            rom,
            pages,
        }
    }
}

impl Mapper for Multi8kMapper {
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
        
        let bank = self.banks[bank_idx] as usize % self.pages as usize;
        let offset = (address & 0x1FFF) as usize;
        
        self.rom[(bank << 13) | offset]
    }
    
    fn write(&mut self, address: u16, value: u8) {
        let bank_idx = match address & 0xFF00 {
            0x0000 => 0,
            0x0100 => 2,
            0x0200 => 1,
            0x0300 => 3,
            _ => return,
        };
        
        self.banks[bank_idx] = value;
    }
    
    fn reset(&mut self) {
        self.banks = [0; 4];
    }
}

// ... continuar com outros mappers multi