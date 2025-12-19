//! Mapeador SEGA padrão (315-5124 / 315-5235)

use super::*;

pub struct SegaMapper {
    fcr: [u8; 4],
    rom: Vec<u8>,
    pages: u16,
    has_ram: bool,
    ram_enabled: bool,
}

impl SegaMapper {
    pub fn new(rom: Vec<u8>) -> Self {
        let pages = ((rom.len() + (1 << 14) - 1) >> 14) as u16;
        
        Self {
            fcr: [0, 0, 1, 2],
            rom,
            pages,
            has_ram: false,
            ram_enabled: false,
        }
    }
    
    pub fn set_ram(&mut self, enabled: bool) {
        self.has_ram = true;
        self.ram_enabled = enabled;
    }
}

impl Mapper for SegaMapper {
    fn read(&self, address: u16) -> u8 {
        match address {
            0x0000..=0x3FFF => {
                let bank = self.fcr[1] as usize % self.pages as usize;
                let offset = address as usize;
                self.rom[(bank << 14) | offset]
            }
            0x4000..=0x7FFF => {
                let bank = self.fcr[2] as usize % self.pages as usize;
                let offset = (address - 0x4000) as usize;
                self.rom[(bank << 14) | offset]
            }
            0x8000..=0xBFFF => {
                if self.ram_enabled && (self.fcr[0] & 0x08) != 0 {
                    // Ler da RAM
                    0xFF // Placeholder
                } else {
                    let bank = self.fcr[3] as usize % self.pages as usize;
                    let offset = (address - 0x8000) as usize;
                    self.rom[(bank << 14) | offset]
                }
            }
            _ => 0xFF,
        }
    }
    
    fn write(&mut self, address: u16, value: u8) {
        match address {
            0xFFFC..=0xFFFF => {
                let offset = (address & 3) as usize;
                self.fcr[offset] = value;
                
                // Atualiza mapeamento
                if offset == 0 {
                    self.ram_enabled = (value & 0x08) != 0;
                }
            }
            _ => {
                // Escrita normal na memória
            }
        }
    }
    
    fn reset(&mut self) {
        self.fcr = [0, 0, 1, 2];
        self.ram_enabled = false;
    }
}