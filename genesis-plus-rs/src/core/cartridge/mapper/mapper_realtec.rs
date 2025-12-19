// genesis-plus-rs/src/core/cartridge/mapper/mapper_realtec.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use super::mapper_common::{BaseMapper, CartridgeMapper, MapperType, MapperConfig};
use log::info;

/// Realtec mapper implementation
pub struct RealtecMapper {
    base: BaseMapper,
    boot_rom_mapped: bool,
    rom_access_enabled: bool,
    fixed_bank_size: u8,
    fixed_bank_selection: u8,
}

impl RealtecMapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            boot_rom_mapped: true,
            rom_access_enabled: false,
            fixed_bank_size: 0,
            fixed_bank_selection: 0,
        }
    }
}

impl CartridgeMapper for RealtecMapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing Realtec mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Realtec, rom_info);
        
        // Copy 8KB Boot ROM after cartridge ROM area
        if rom_data.len() >= 0x7E000 + 0x2000 {
            let boot_rom = &rom_data[0x7E000..0x7E000 + 0x2000];
            // This would need to be stored separately
        }
        
        // Boot ROM (8KB mirrored) is mapped to $000000-$3FFFFF on reset
        for i in 0..0x40 {
            // In real implementation, this would map to boot ROM
            memory_map.map_rom(i, rom_data);
        }
        
        self.boot_rom_mapped = true;
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.boot_rom_mapped = true;
            self.rom_access_enabled = false;
            self.fixed_bank_size = 0;
            self.fixed_bank_selection = 0;
            self.base.regs = [0; 4];
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Realtec
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // Realtec mapper doesn't use !TIME signal
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        0xFFFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        match address {
            0x400000 => {
                // ROM access enable
                if data & 0x01 != 0 && !self.rom_access_enabled {
                    // Once ROM access is enabled, ROM mapping cannot be modified until next reset
                    for i in 0..0x40 {
                        // 0x000000-0x07ffff mapped area is mirrored in 4MB cartridge range
                        let mut base = (i & 7) as u32;
                        
                        // Adjust 64k mapped area ROM base address according to fixed ROM bank configuration
                        base = (base & !(self.fixed_bank_size as u32)) | 
                               (self.fixed_bank_selection as u32 & self.fixed_bank_size as u32);
                        
                        // Map ROM
                        let rom_offset = (base << 16) & self.base.rom_mask;
                        if rom_offset < self.base.rom_data.len() as u32 {
                            memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                        }
                    }
                    
                    self.rom_access_enabled = true;
                    self.boot_rom_mapped = false;
                }
            }
            
            0x402000 => {
                // Fixed ROM bank size
                // Bits 0-1 control which address pins are forced
                self.fixed_bank_size = (data & 3) as u8;
            }
            
            0x404000 => {
                // Fixed ROM bank selection (4 x 128KB banks)
                self.fixed_bank_selection = (data & 3) as u8;
            }
            
            _ => {
                // Unknown register
            }
        }
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.boot_rom_mapped as u8);
        state.push(self.rom_access_enabled as u8);
        state.push(self.fixed_bank_size);
        state.push(self.fixed_bank_selection);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 4 + self.base.regs.len() {
            return false;
        }
        
        self.boot_rom_mapped = data[0] != 0;
        self.rom_access_enabled = data[1] != 0;
        self.fixed_bank_size = data[2];
        self.fixed_bank_selection = data[3];
        self.base.regs.copy_from_slice(&data[4..4 + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        // Update mapping based on current state
        if self.boot_rom_mapped {
            // Boot ROM mapped
            for i in 0..0x40 {
                // Map boot ROM
                // In real implementation, this would map to boot ROM area
                memory_map.map_rom(i, &self.base.rom_data);
            }
        } else if self.rom_access_enabled {
            // ROM access enabled with current banking
            for i in 0..0x40 {
                let mut base = (i & 7) as u32;
                base = (base & !(self.fixed_bank_size as u32)) | 
                       (self.fixed_bank_selection as u32 & self.fixed_bank_size as u32);
                
                let rom_offset = (base << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
    }
}

impl Default for RealtecMapper {
    fn default() -> Self {
        Self::new()
    }
}