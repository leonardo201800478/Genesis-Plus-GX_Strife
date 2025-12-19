// genesis-plus-rs/src/core/cartridge/mapper/mapper_flash.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use super::mapper_common::{BaseMapper, CartridgeMapper, MapperType, MapperConfig};
use log::info;

/// Flash mapper implementation
pub struct FlashMapper {
    base: BaseMapper,
    flash_type: FlashType,
    write_enable: bool,
    current_command: u32,
    sector_protect: [bool; 128], // Up to 128 sectors
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum FlashType {
    M29W320EB,
    S29GL064N04,
    Unknown,
}

impl FlashMapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            flash_type: FlashType::Unknown,
            write_enable: false,
            current_command: 0,
            sector_protect: [false; 128],
        }
    }
    
    fn flash_write(&mut self, address: u32, data: u32) {
        // Handle flash memory commands
        match self.current_command {
            0xAAAA => {
                if data == 0x5555 {
                    self.current_command = 0x5555;
                }
            }
            0x5555 => {
                if data == 0xAAAA {
                    // Enter command mode
                    self.current_command = 0;
                } else if data == 0x9090 {
                    // Enter autoselect mode
                    self.current_command = 0x9090;
                } else if data == 0x8080 {
                    // Enter erase mode
                    self.current_command = 0x8080;
                } else if data == 0xF0F0 {
                    // Reset
                    self.current_command = 0;
                    self.write_enable = false;
                }
            }
            0x8080 => {
                if data == 0x1010 {
                    // Chip erase
                    // In real implementation, would erase entire flash
                    self.current_command = 0;
                } else if data == 0x3030 {
                    // Sector erase
                    self.current_command = 0x3030;
                }
            }
            0x3030 => {
                // Sector erase address
                // In real implementation, would erase sector
                self.current_command = 0;
            }
            _ => {
                // Normal write if write enable
                if self.write_enable {
                    let offset = address & self.base.rom_mask;
                    if offset < self.base.rom_data.len() as u32 {
                        // In real flash, this would write to flash memory
                        // For emulation, we might want to track writes
                    }
                }
                self.current_command = 0;
            }
        }
    }
    
    fn flash_read(&self, address: u32) -> u32 {
        match self.current_command {
            0x9090 => {
                // Autoselect mode
                match address & 0xFF {
                    0x00 => {
                        // Manufacturer ID
                        match self.flash_type {
                            FlashType::M29W320EB => 0x0020,
                            FlashType::S29GL064N04 => 0x0001,
                            _ => 0xFFFF,
                        }
                    }
                    0x02 => {
                        // Device ID
                        match self.flash_type {
                            FlashType::M29W320EB => 0x22CB,
                            FlashType::S29GL064N04 => 0x227E,
                            _ => 0xFFFF,
                        }
                    }
                    _ => 0xFFFF,
                }
            }
            _ => {
                // Normal read
                let offset = address & self.base.rom_mask;
                if offset < self.base.rom_data.len() as u32 {
                    let addr = offset as usize;
                    if addr + 1 < self.base.rom_data.len() {
                        (self.base.rom_data[addr] as u32) |
                        ((self.base.rom_data[addr + 1] as u32) << 8)
                    } else {
                        0xFFFF
                    }
                } else {
                    0xFFFF
                }
            }
        }
    }
}

impl CartridgeMapper for FlashMapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing Flash mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Flash, rom_info);
        
        // Determine flash type based on game
        if rom_info.product.contains("00000000-42") {
            // Escape 2042
            self.flash_type = FlashType::M29W320EB;
        } else if rom_info.product.contains("00000000-00") {
            // Life on Mars or similar
            self.flash_type = FlashType::S29GL064N04;
        }
        
        // Setup memory mapping
        self.update_mapping(memory_map);
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.write_enable = false;
            self.current_command = 0;
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Flash
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // Flash mapper doesn't use !TIME signal
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        0xFFFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // Handle flash writes
        self.flash_write(address, data);
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Handle flash reads
        self.flash_read(address)
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.flash_type as u8);
        state.push(self.write_enable as u8);
        state.extend_from_slice(&self.current_command.to_le_bytes());
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        // Save sector protect status (simplified)
        for i in 0..16 {
            let mut byte = 0u8;
            for j in 0..8 {
                if self.sector_protect[i * 8 + j] {
                    byte |= 1 << j;
                }
            }
            state.push(byte);
        }
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 1 + 1 + 4 + 1 + self.base.regs.len() + 16 {
            return false;
        }
        
        let mut offset = 0;
        self.flash_type = match data[offset] {
            0 => FlashType::M29W320EB,
            1 => FlashType::S29GL064N04,
            _ => FlashType::Unknown,
        };
        offset += 1;
        
        self.write_enable = data[offset] != 0;
        offset += 1;
        
        self.current_command = u32::from_le_bytes([
            data[offset], data[offset + 1], data[offset + 2], data[offset + 3]
        ]);
        offset += 4;
        
        self.base.current_bank = data[offset] as u32;
        offset += 1;
        
        let regs_len = self.base.regs.len();
        self.base.regs.copy_from_slice(&data[offset..offset + regs_len]);
        offset += regs_len;
        
        // Load sector protect status
        for i in 0..16 {
            let byte = data[offset + i];
            for j in 0..8 {
                self.sector_protect[i * 8 + j] = (byte >> j) & 1 != 0;
            }
        }
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        // Flash mapper typically maps ROM normally
        // Special handling would be in the read/write handlers
        for i in 0..0x40 {
            let rom_offset = (i << 16) & self.base.rom_mask;
            if rom_offset < self.base.rom_data.len() as u32 {
                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
            }
        }
    }
}

impl Default for FlashMapper {
    fn default() -> Self {
        Self::new()
    }
}