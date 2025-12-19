// genesis-plus-rs/src/core/cartridge/mapper/mapper_custom.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use super::mapper_common::{BaseMapper, CartridgeMapper, MapperType, MapperConfig};
use log::info;

/// Custom mapper implementation for various protection schemes
pub struct CustomMapper {
    base: BaseMapper,
    mapper_variant: CustomMapperVariant,
    regs_extended: [u8; 16],
    bank_mode: u8,
    sram_mapped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CustomMapperVariant {
    /// 32K bankswitch (Soul Edge VS Samurai Spirits, etc.)
    Bankswitch32K,
    /// 64K bankswitch (Chinese Fighter III)
    Bankswitch64K,
    /// Multi-game mapper
    MultiGame,
    /// WD1601 mapper (Canon - Legend of the New Gods)
    Wd1601,
    /// Super Mario World 64 mapper
    Smw64,
    /// Default custom mapper
    Default,
}

impl CustomMapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            mapper_variant: CustomMapperVariant::Default,
            regs_extended: [0; 16],
            bank_mode: 0,
            sram_mapped: false,
        }
    }
    
    fn determine_variant(&mut self, rom_info: &RomInfo) {
        // Determine mapper variant based on game
        if rom_info.checksum == 0x8180 {
            // Chinese Fighter III
            self.mapper_variant = CustomMapperVariant::Bankswitch64K;
        } else if rom_info.real_checksum == 0x5D8B || // Top Fighter
                  rom_info.real_checksum == 0x5D34 || // Soul Edge VS Samurai Spirits
                  rom_info.real_checksum == 0x1B40 || // Mulan
                  rom_info.real_checksum == 0x17E5    // Pocket Monsters II
        {
            self.mapper_variant = CustomMapperVariant::Bankswitch32K;
        } else if rom_info.product.contains("T-119186") {
            // Barkley Shut Up and Jam 2
            self.mapper_variant = CustomMapperVariant::MultiGame;
        } else if rom_info.real_checksum == 0xF894 {
            // Super Mario World 64
            self.mapper_variant = CustomMapperVariant::Smw64;
        } else {
            self.mapper_variant = CustomMapperVariant::Default;
        }
    }
}

impl CartridgeMapper for CustomMapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing custom mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Custom, rom_info);
        
        self.determine_variant(rom_info);
        
        // Setup initial mapping based on variant
        match self.mapper_variant {
            CustomMapperVariant::Bankswitch32K => {
                // Initial 32K mapping
                self.update_mapping(memory_map);
            }
            CustomMapperVariant::Bankswitch64K => {
                // Initial 64K mapping
                self.update_mapping(memory_map);
            }
            CustomMapperVariant::MultiGame => {
                // Multi-game mapper setup
                self.update_mapping(memory_map);
            }
            CustomMapperVariant::Wd1601 => {
                // WD1601 mapper setup
                self.sram_mapped = true;
                self.update_mapping(memory_map);
            }
            CustomMapperVariant::Smw64 => {
                // SMW64 special mapping
                // Lower 512KB mirrored in $000000-$0FFFFF
                for i in 0x00..0x10 {
                    let rom_offset = ((i & 7) << 16) & self.base.rom_mask;
                    if rom_offset < self.base.rom_data.len() as u32 {
                        memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                    }
                }
                
                // Custom hardware at $600000-$6FFFFF
                for i in 0x60..0x70 {
                    memory_map.map_custom(i);
                }
            }
            CustomMapperVariant::Default => {
                // Default mapping
                self.update_mapping(memory_map);
            }
        }
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.regs_extended = [0; 16];
            self.bank_mode = 0;
            self.sram_mapped = false;
            self.base.current_bank = 0;
            self.base.regs = [0; 4];
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Custom
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        match self.mapper_variant {
            CustomMapperVariant::Wd1601 => {
                // WD1601 mapper
                if (address & 0xFE) == 0x02 {
                    // Upper 2MB ROM mapped to $000000-$1FFFFF
                    for i in 0..0x20 {
                        let rom_offset = ((0x20 + i) << 16) & self.base.rom_mask;
                        if rom_offset < self.base.rom_data.len() as u32 {
                            memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                        }
                    }
                    
                    // SRAM mapped to $200000-$3FFFFF
                    self.sram_mapped = true;
                    self.update_mapping(memory_map);
                }
            }
            _ => {
                // Default handling for other variants
                if address < 0xA13060 {
                    // Multi-game mapper
                    let bank = address;
                    for i in 0..0x40 {
                        let rom_bank = (bank + i as u32) & 0x3F;
                        let rom_offset = (rom_bank << 16) & self.base.rom_mask;
                        if rom_offset < self.base.rom_data.len() as u32 {
                            memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                        }
                    }
                }
            }
        }
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        // Default return value
        0xFFFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        match self.mapper_variant {
            CustomMapperVariant::Bankswitch32K => {
                if (address >> 16) > 0x6F {
                    // ROM bankswitch
                    if data != 0 {
                        // Remap to unused ROM area
                        let bank = (data & 0x3F) as u32;
                        for i in 0..0x10 {
                            let rom_offset = ((i << 16) | (bank << 15)) & self.base.rom_mask;
                            if rom_offset < self.base.rom_data.len() as u32 {
                                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                            }
                        }
                    } else {
                        // Reset to default mapping
                        for i in 0..0x10 {
                            let rom_offset = (i << 16) & self.base.rom_mask;
                            if rom_offset < self.base.rom_data.len() as u32 {
                                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                            }
                        }
                    }
                } else {
                    // Register write with bitswapping
                    let reg_index = (address >> 1) & 3;
                    self.base.regs[reg_index as usize] = data as u8;
                    
                    // Perform bitswapping based on regs[1]
                    let temp = self.base.regs[0];
                    match self.base.regs[1] & 3 {
                        0 => self.base.regs[2] = temp << 1,
                        1 => self.base.regs[2] = temp >> 1,
                        2 => self.base.regs[2] = (temp >> 4) | ((temp & 0x0F) << 4),
                        _ => {
                            self.base.regs[2] = ((temp >> 7) & 0x01) |
                                                ((temp >> 5) & 0x02) |
                                                ((temp >> 3) & 0x04) |
                                                ((temp >> 1) & 0x08) |
                                                ((temp << 1) & 0x10) |
                                                ((temp << 3) & 0x20) |
                                                ((temp << 5) & 0x40) |
                                                ((temp << 7) & 0x80);
                        }
                    }
                }
            }
            
            CustomMapperVariant::Bankswitch64K => {
                if (address >> 16) > 0x5F {
                    // ROM bankswitch
                    if data != 0 {
                        let bank = (data & 0xF) as u32;
                        for i in 0..0x10 {
                            let rom_offset = (bank << 16) & self.base.rom_mask;
                            if rom_offset < self.base.rom_data.len() as u32 {
                                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                            }
                        }
                    } else {
                        // Reset to default mapping
                        for i in 0..0x10 {
                            let rom_offset = (i << 16) & self.base.rom_mask;
                            if rom_offset < self.base.rom_data.len() as u32 {
                                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                            }
                        }
                    }
                } else {
                    // Normal register write
                    let reg_index = (address >> 1) & 3;
                    self.base.regs[reg_index as usize] = data as u8;
                }
            }
            
            _ => {
                // Default register handling
                let reg_index = (address >> 1) & 3;
                if reg_index < 4 {
                    self.base.regs[reg_index as usize] = data as u8;
                }
            }
        }
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Return register value if address matches
        for i in 0..4 {
            if (address & self.base.config.sram_mask) == self.base.config.sram_start {
                return self.base.regs[i] as u32;
            }
        }
        
        // Default return
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.mapper_variant as u8);
        state.extend_from_slice(&self.regs_extended);
        state.push(self.bank_mode);
        state.push(self.sram_mapped as u8);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 1 + self.regs_extended.len() + 3 + self.base.regs.len() {
            return false;
        }
        
        let mut offset = 0;
        self.mapper_variant = match data[offset] {
            0 => CustomMapperVariant::Bankswitch32K,
            1 => CustomMapperVariant::Bankswitch64K,
            2 => CustomMapperVariant::MultiGame,
            3 => CustomMapperVariant::Wd1601,
            4 => CustomMapperVariant::Smw64,
            _ => CustomMapperVariant::Default,
        };
        offset += 1;
        
        self.regs_extended.copy_from_slice(&data[offset..offset + self.regs_extended.len()]);
        offset += self.regs_extended.len();
        
        self.bank_mode = data[offset];
        offset += 1;
        
        self.sram_mapped = data[offset] != 0;
        offset += 1;
        
        self.base.current_bank = data[offset] as u32;
        offset += 1;
        
        self.base.regs.copy_from_slice(&data[offset..offset + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        match self.mapper_variant {
            CustomMapperVariant::Wd1601 if self.sram_mapped => {
                // SRAM mapped to $200000-$3FFFFF
                for i in 0x20..0x40 {
                    memory_map.map_sram(i, None); // Would need SRAM reference
                }
            }
            _ => {
                // Default ROM mapping
                for i in 0..0x40 {
                    let rom_offset = (i << 16) & self.base.rom_mask;
                    if rom_offset < self.base.rom_data.len() as u32 {
                        memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                    }
                }
            }
        }
    }
}

impl Default for CustomMapper {
    fn default() -> Self {
        Self::new()
    }
}