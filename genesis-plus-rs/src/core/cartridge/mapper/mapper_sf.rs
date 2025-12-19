// genesis-plus-rs/src/core/cartridge/mapper/mapper_sf.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use super::mapper_common::{BaseMapper, CartridgeMapper, MapperType, MapperConfig};
use log::info;

/// SF-001 mapper implementation
pub struct Sf001Mapper {
    base: BaseMapper,
    mode_register: u8,
    rom_enabled: bool,
    sram_enabled: bool,
    hardware_locked: bool,
}

impl Sf001Mapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            mode_register: 0,
            rom_enabled: true,
            sram_enabled: false,
            hardware_locked: false,
        }
    }
}

impl CartridgeMapper for Sf001Mapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing SF-001 mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Sf001, rom_info);
        
        // Setup default mapping
        self.update_mapping(memory_map);
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.mode_register = 0;
            self.rom_enabled = true;
            self.sram_enabled = false;
            self.hardware_locked = false;
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Sf001
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // SF-001 doesn't use !TIME signal
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        0xFFFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        if self.hardware_locked {
            return;
        }
        
        match (address >> 8) & 0xF {
            0xE => {
                self.mode_register = data as u8;
                
                // Bit 6: enable/disable cartridge access
                if data & 0x40 != 0 {
                    self.rom_enabled = false;
                }
                // Bit 7: enable/disable SRAM & ROM bankswitching
                else if data & 0x80 != 0 {
                    self.sram_enabled = true;
                    self.rom_enabled = true;
                } else {
                    self.rom_enabled = true;
                    self.sram_enabled = false;
                }
                
                // Bit 5: lock bankswitch hardware when set
                if data & 0x20 != 0 {
                    self.hardware_locked = true;
                }
                
                self.update_mapping(memory_map);
            }
            _ => {}
        }
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.mode_register);
        state.push(self.rom_enabled as u8);
        state.push(self.sram_enabled as u8);
        state.push(self.hardware_locked as u8);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 5 + self.base.regs.len() {
            return false;
        }
        
        self.mode_register = data[0];
        self.rom_enabled = data[1] != 0;
        self.sram_enabled = data[2] != 0;
        self.hardware_locked = data[3] != 0;
        self.base.current_bank = data[4] as u32;
        self.base.regs.copy_from_slice(&data[5..5 + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        if !self.rom_enabled {
            // ROM disabled
            for i in 0..0x40 {
                memory_map.unmap(i);
            }
        } else if self.sram_enabled {
            // SRAM enabled mode
            // 256K ROM bank #15 mapped to $000000-$03FFFF
            for i in 0..0x04 {
                let bank = 0x38 + i; // Bank #15
                let rom_offset = (bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
            
            // Remaining areas would need SRAM mapping
            // Simplified implementation
        } else {
            // Normal mode: 256K ROM banks #1 to #16 mapped to $000000-$3FFFFF
            for i in 0..0x40 {
                let rom_offset = (i << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
    }
}

impl Default for Sf001Mapper {
    fn default() -> Self {
        Self::new()
    }
}

/// SF-002 mapper implementation
pub struct Sf002Mapper {
    base: BaseMapper,
    bank_remap_enabled: bool,
}

impl Sf002Mapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            bank_remap_enabled: false,
        }
    }
}

impl CartridgeMapper for Sf002Mapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing SF-002 mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Sf002, rom_info);
        
        // Setup default mapping
        self.update_mapping(memory_map);
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.bank_remap_enabled = false;
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Sf002
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // SF-002 doesn't use !TIME signal
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        0xFFFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        if data & 0x80 != 0 {
            self.bank_remap_enabled = true;
        } else {
            self.bank_remap_enabled = false;
        }
        
        self.update_mapping(memory_map);
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.bank_remap_enabled as u8);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 2 + self.base.regs.len() {
            return false;
        }
        
        self.bank_remap_enabled = data[0] != 0;
        self.base.current_bank = data[1] as u32;
        self.base.regs.copy_from_slice(&data[2..2 + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        if self.bank_remap_enabled {
            // $000000-$1BFFFF mapped to $200000-$3BFFFF
            for i in 0x20..0x3C {
                let src_bank = (i - 0x20) as u32;
                let rom_offset = (src_bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        } else {
            // Normal mapping
            for i in 0x20..0x3C {
                let rom_offset = (i << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
        
        // Always map lower area
        for i in 0..0x20 {
            let rom_offset = (i << 16) & self.base.rom_mask;
            if rom_offset < self.base.rom_data.len() as u32 {
                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
            }
        }
    }
}

impl Default for Sf002Mapper {
    fn default() -> Self {
        Self::new()
    }
}

/// SF-004 mapper implementation
pub struct Sf004Mapper {
    base: BaseMapper,
    sram_enabled: bool,
    rom_access_enabled: bool,
    first_page_mirroring: bool,
    hardware_locked: bool,
    first_page_bank: u8,
}

impl Sf004Mapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            sram_enabled: false,
            rom_access_enabled: true,
            first_page_mirroring: true,
            hardware_locked: false,
            first_page_bank: 0,
        }
    }
}

impl CartridgeMapper for Sf004Mapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing SF-004 mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::Sf004, rom_info);
        
        // First 256K ROM bank is mirrored into $000000-$1FFFFF on reset
        for i in 0..0x20 {
            let rom_offset = ((i & 0x03) << 16) & self.base.rom_mask;
            if rom_offset < self.base.rom_data.len() as u32 {
                memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
            }
        }
        
        self.first_page_mirroring = true;
        self.first_page_bank = 0;
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.sram_enabled = false;
            self.rom_access_enabled = true;
            self.first_page_mirroring = true;
            self.hardware_locked = false;
            self.first_page_bank = 0;
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::Sf004
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        // SF-004 doesn't use !TIME signal for writes
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        // Return first page 256K bank index
        (self.first_page_bank as u32) << 4
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        if self.hardware_locked {
            return;
        }
        
        match (address >> 8) & 0xF {
            0xD => {
                // Bit 7: enable/disable static RAM access
                self.sram_enabled = (data & 0x80) != 0;
                self.update_mapping(memory_map);
            }
            
            0xE => {
                // Bit 5: enable/disable cartridge ROM access
                self.rom_access_enabled = (data & 0x20) == 0;
                
                // Bit 6: enable/disable first page mirroring
                self.first_page_mirroring = (data & 0x40) != 0;
                
                // Bit 7: lock ROM bankswitching hardware when cleared
                if (data & 0x80) == 0 {
                    self.hardware_locked = true;
                }
                
                self.update_mapping(memory_map);
            }
            
            0xF => {
                // Bits 6-4: select first page ROM bank (8 x 256K ROM banks)
                self.first_page_bank = ((data >> 4) & 7) as u8;
                self.update_mapping(memory_map);
            }
            
            _ => {}
        }
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.sram_enabled as u8);
        state.push(self.rom_access_enabled as u8);
        state.push(self.first_page_mirroring as u8);
        state.push(self.hardware_locked as u8);
        state.push(self.first_page_bank);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 6 + self.base.regs.len() {
            return false;
        }
        
        self.sram_enabled = data[0] != 0;
        self.rom_access_enabled = data[1] != 0;
        self.first_page_mirroring = data[2] != 0;
        self.hardware_locked = data[3] != 0;
        self.first_page_bank = data[4];
        self.base.current_bank = data[5] as u32;
        self.base.regs.copy_from_slice(&data[6..6 + self.base.regs.len()]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        if !self.rom_access_enabled {
            // ROM access disabled
            for i in 0..0x20 {
                memory_map.unmap(i);
            }
        } else if self.first_page_mirroring {
            // First page mirroring enabled
            let base = (self.first_page_bank as u32) << 2;
            for i in 0..0x20 {
                let bank = base + ((i & 0x03) as u32);
                let rom_offset = (bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        } else {
            // 5 x 256K ROM banks mapped to $000000-$13FFFF
            let base = (self.first_page_bank as u32) << 2;
            for i in 0..0x14 {
                let bank = (base + i as u32) & 0x1F;
                let rom_offset = (bank << 16) & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
            
            // $140000-$1FFFFF unmapped
            for i in 0x14..0x20 {
                memory_map.unmap(i);
            }
        }
    }
}

impl Default for Sf004Mapper {
    fn default() -> Self {
        Self::new()
    }
}

/// T-5740 mapper implementation
pub struct T5740Mapper {
    base: BaseMapper,
    mode_register: u8,
    page_registers: [u8; 3], // Pages 5, 6, 7
}

impl T5740Mapper {
    pub fn new() -> Self {
        Self {
            base: BaseMapper::new(),
            mode_register: 0,
            page_registers: [0; 3],
        }
    }
}

impl CartridgeMapper for T5740Mapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        info!("Initializing T-5740 mapper");
        
        self.base.setup_rom_mirroring(rom_data);
        self.base.config = super::mapper_database::get_mapper_config(MapperType::T5740, rom_info);
        
        // Setup default mapping
        self.update_mapping(memory_map);
    }
    
    fn reset(&mut self, hard_reset: bool) {
        if hard_reset {
            self.mode_register = 0;
            self.page_registers = [0; 3];
            self.base.current_bank = 0;
        }
    }
    
    fn mapper_type(&self) -> MapperType {
        MapperType::T5740
    }
    
    fn config(&self) -> &MapperConfig {
        &self.base.config
    }
    
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap) {
        match address & 0xFF {
            0x01 => {
                // Mode register
                self.mode_register = data as u8;
            }
            
            0x03 => {
                // Page #5 register
                self.page_registers[0] = data as u8;
                self.update_mapping(memory_map);
            }
            
            0x05 => {
                // Page #6 register
                self.page_registers[1] = data as u8;
                self.update_mapping(memory_map);
            }
            
            0x07 => {
                // Page #7 register
                self.page_registers[2] = data as u8;
                self.update_mapping(memory_map);
            }
            
            0x09 => {
                // Serial EEPROM SPI board support
                // Would call eeprom_spi_write(data)
            }
            
            _ => {
                // Unknown register
            }
        }
    }
    
    fn handle_time_read(&self, address: u32) -> u32 {
        // Handle special mirroring for $181xx area
        if (address & 0xFF00) == 0x8100 {
            // Return mirrored data from first 32K of each 512K page
            let offset = address & 0x7FFF;
            if offset < self.base.rom_data.len() as u32 {
                return self.base.rom_data[offset as usize] as u32;
            }
        }
        
        0xFF
    }
    
    fn handle_register_write(&mut self, address: u32, data: u32, _memory_map: &mut MemoryMap) {
        // T-5740 uses !TIME for register writes
    }
    
    fn handle_register_read(&self, address: u32) -> u32 {
        // Default register read
        0xFF
    }
    
    fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        state.push(self.mode_register);
        state.extend_from_slice(&self.page_registers);
        state.push(self.base.current_bank as u8);
        state.extend_from_slice(&self.base.regs);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 1 + self.page_registers.len() + 1 + self.base.regs.len() {
            return false;
        }
        
        self.mode_register = data[0];
        self.page_registers.copy_from_slice(&data[1..1 + self.page_registers.len()]);
        self.base.current_bank = data[1 + self.page_registers.len()] as u32;
        self.base.regs.copy_from_slice(&data[2 + self.page_registers.len()..]);
        
        true
    }
    
    fn update_mapping(&mut self, memory_map: &mut MemoryMap) {
        // Map pages based on page registers
        // Page 5: $280000-$2FFFFF
        if self.page_registers[0] != 0 {
            let base = (self.page_registers[0] as u32 & 0x0F) << 19;
            for i in 0x28..0x30 {
                let bank_offset = base + ((i & 0x07) << 16);
                let rom_offset = bank_offset & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
        
        // Page 6: $300000-$37FFFF
        if self.page_registers[1] != 0 {
            let base = (self.page_registers[1] as u32 & 0x0F) << 19;
            for i in 0x30..0x38 {
                let bank_offset = base + ((i & 0x07) << 16);
                let rom_offset = bank_offset & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
        
        // Page 7: $380000-$3FFFFF
        if self.page_registers[2] != 0 {
            let base = (self.page_registers[2] as u32 & 0x0F) << 19;
            for i in 0x38..0x40 {
                let bank_offset = base + ((i & 0x07) << 16);
                let rom_offset = bank_offset & self.base.rom_mask;
                if rom_offset < self.base.rom_data.len() as u32 {
                    memory_map.map_rom(i, &self.base.rom_data[rom_offset as usize..]);
                }
            }
        }
    }
}

impl Default for T5740Mapper {
    fn default() -> Self {
        Self::new()
    }
}
