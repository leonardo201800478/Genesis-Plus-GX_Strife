//! Mapeadores coreanos

use super::*;

/// Mapeador coreano de 8KB
pub struct Korean8kMapper {
    banks: [u8; 4],
    rom: Vec<u8>,
    pages: u16,
    protected: [bool; 4],
}

impl Korean8kMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 13) - 1) >> 13) as u16;
        
        Self {
            banks: [0; 4],
            rom,
            pages,
            protected: [false; 4],
        }
    }
}

impl Mapper for Korean8kMapper {
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
        let mut data = self.rom[(bank << 13) | offset];
        
        // Bitswap se protegido
        if self.protected[bank_idx] {
            data = ((data >> 7) & 0x01) |
                   ((data >> 5) & 0x02) |
                   ((data >> 3) & 0x04) |
                   ((data >> 1) & 0x08) |
                   ((data << 1) & 0x10) |
                   ((data << 3) & 0x20) |
                   ((data << 5) & 0x40) |
                   ((data << 7) & 0x80);
        }
        
        data
    }
    
    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x4000 => self.banks[2] = value,
            0x6000 => self.banks[3] = value,
            0x8000 => {
                self.banks[0] = value;
                self.protected[0] = (value & 0x80) != 0;
            }
            0xA000 => {
                self.banks[1] = value;
                self.protected[1] = (value & 0x80) != 0;
            }
            0xFFFE => {
                self.banks[2] = (value << 1) & 0xFF;
                self.banks[3] = (1 + (value << 1)) & 0xFF;
            }
            0xFFFF => {
                self.banks[0] = (value << 1) & 0xFF;
                self.banks[1] = (1 + (value << 1)) & 0xFF;
            }
            _ => {}
        }
    }
    
    fn reset(&mut self) {
        self.banks = [0; 4];
        self.protected = [false; 4];
    }
}

/// Mapeador coreano de 16KB (Versão 1)
pub struct Korean16kV1Mapper {
    bank: u8,
    rom: Vec<u8>,
    pages: u16,
}

impl Korean16kV1Mapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            bank: 0,
            rom,
            pages,
        }
    }
}

impl Mapper for Korean16kV1Mapper {
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
        if address == 0xA000 {
            self.bank = value;
        }
    }
    
    fn reset(&mut self) {
        self.bank = 0;
    }
}

/// Mapeador coreano de 16KB (Versão 2)
pub struct Korean16kV2Mapper {
    banks: [u8; 2],
    rom: Vec<u8>,
    pages: u16,
}

impl Korean16kV2Mapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            banks: [0, 0],
            rom,
            pages,
        }
    }
}

impl Mapper for Korean16kV2Mapper {
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
                let bank = self.banks[1] as usize % self.pages as usize;
                let offset = (address - 0x8000) as usize;
                self.rom[(bank << 14) | offset]
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        match address {
            0x4000 => self.banks[0] = value,
            0x8000 => self.banks[1] = value,
            0xFFFC..=0xFFFF => {
                // Compatibilidade com mapper SEGA
                let offset = (address & 3) as usize;
                if offset == 2 {
                    self.banks[0] = value;
                } else if offset == 3 {
                    self.banks[1] = value;
                }
            }
            _ => {}
        }
    }
    
    fn reset(&mut self) {
        self.banks = [0, 0];
    }
}
