// genesis-plus-rs/src/core/cartridge/mapper/mapper_radica.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use super::mapper_common::{BaseMapper, CartridgeMapper, MapperType, MapperConfig};
use log::info;

/// Radica mapper implementation
pub struct RadicaMapper {
    base: BaseMapper,
    mapper_64k: bool, // true for 64K banks, false for 128K banks
    current_bank_index: u8,
    sram_enabled: bool,
}

impl RadicaMapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            mapper_64k: true,
            current_bank_index: 0,
            sram_enabled: false,
        }
    }
}

impl CartridgeMapper for RadicaMapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing Radica mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Radica, rom_info);
        
        // Determine mapper type based on game
        if rom_info.real_checksum == 0x1F7F {
            // Sensible Soccer Plus edition uses 128K mapper with SRAM
            self.mapper_64k = false;
            self.sram_enabled = true;
        } else {
            // Most games use 64K mapper
            self.mapper_64k = true;
            self.sram_enabled = false;
        }
        
        // Setup initial mapping
        self.update_mapping(memory_map);
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.current_bank_index = 0;
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Radica
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // Radica mapper uses !TIME reads, not writes
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        // Bank index is encoded in address
        let index = if self.mapper_64k {
            // 64K banks: index from lower 6 bits
            (address >> 1) & 0x3F
        } else {
            // 128K banks: index from bits 1-6 (ignore bit 0)
            (address >> 1) & 0x3E
        };
        
        // Return value changes menu title
        // Real cartridge returns different values based on physical switches
        // We return the largest menu selection
        0x03
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // Handle !TIME reads to change banks
        let index = if self.mapper_64k {
            (address >> 1) & 0x3F
        } else {
            (address >> 1) & 0x3E
        };
        
        self.current_bank_index = index as u8;
        self.update_mapping(memory_map);
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.mapper_64k as u8);
        state.push(self.current_bank_index);
        state.push(self.sram_enabled as u8);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 4 + self.base.regs.len() {
            return false;
        }
        
        self.mapper_64k = data[0] != 0;
        self.current_bank_index = data[1];
        self.sram_enabled = data[2] != 0;
        self.base.current_bank = data[3] as u32;
        self.base.regs.copy_from_slice(&data[4..4 + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        if self.mapper_64k {
            // 64 x 64K banks
            let index = self.current_bank_index as u32;
            for i in 0..0x40 {
                let bank = index | (i as u32);
                let rom_offset = (bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        } else {
            // 32 x 128K banks with SRAM
            let index = self.current_bank_index as u32;
            
            // $000000-$1FFFFF area mapped to selected banks
            for i in 0..0x20 {
                let bank = index | (i as u32);
                let rom_offset = (bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
            
            if self.sram_enabled {
                // $200000-$3FFFFF area mapped to 8KB SRAM (mirrored)
                for i in 0x20..0x40 {
                    // In real implementation, this would map SRAM
                    memory_map.unmap(i);
                }
            } else {
                // No SRAM, map ROM continuation
                for i in 0x20..0x40 {
                    let bank = index | (i as u32);
                    let rom_offset = (bank << 16) & self.base.rom_mask;
                    if rom_offset < self.base.rom_data.len() as u32 {
                        memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                    }
                }
            }
        }
    }
}

impl Default for RadicaMapper {
    fn default() -> Self {
        Self::new()
    }
}
