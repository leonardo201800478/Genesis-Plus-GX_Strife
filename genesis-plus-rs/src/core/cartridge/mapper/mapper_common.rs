// genesis-plus-rs/src/core/cartridge/mapper/mapper_common.rs

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use crate::core::m68k::{M68KReadFunc, M68KWriteFunc};
use crate::core::z80::memory::Z80MemoryMap;
use crate::utils::crc32;

// SMS mapper constants re-export
pub use super::{
    MAPPER_NONE, MAPPER_TEREBI, MAPPER_RAM_2K, MAPPER_RAM_8K, MAPPER_RAM_8K_EXT1,
    MAPPER_SEGA, MAPPER_SEGA_X, MAPPER_93C46, MAPPER_CODIES, MAPPER_MULTI_16K,
    MAPPER_KOREA_16K_V1, MAPPER_KOREA_16K_V2, MAPPER_MULTI_2X16K_V1, MAPPER_MULTI_2X16K_V2,
    MAPPER_MULTI_16K_32K_V1, MAPPER_MULTI_16K_32K_V2, MAPPER_ZEMINA_16K_32K, MAPPER_HWASUNG,
    MAPPER_MSX_16K, MAPPER_KOREA_8K, MAPPER_MSX_8K, MAPPER_MSX_8K_NEMESIS, MAPPER_MULTI_8K,
    MAPPER_MULTI_4X8K, MAPPER_ZEMINA_4X8K, MAPPER_MULTI_32K, MAPPER_MULTI_32K_16K, MAPPER_HICOM,
};

/// Mapper type enumeration for all supported systems
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperType {
    /// Standard SEGA mapper (Mega Drive/Genesis)
    Standard,
    /// Realtec mapper (Earth Defense, Balloon Boy, etc.)
    Realtec,
    /// SF-001 mapper
    Sf001,
    /// SF-002 mapper
    Sf002,
    /// SF-004 mapper
    Sf004,
    /// T-5740 mapper
    T5740,
    /// Flash memory mapper
    Flash,
    /// Radica mapper
    Radica,
    /// Custom/unknown mapper
    Custom,
    
    // SMS/GG mappers
    /// SEGA SMS mapper (315-5124 / 315-5235)
    SmsSega,
    /// Codemasters SMS mapper
    SmsCodemasters,
    /// Korean SMS mappers
    SmsKorean,
    /// MSX SMS mappers
    SmsMsx,
    /// Multicart SMS mappers
    SmsMulti,
    /// Zemina SMS mappers
    SmsZemina,
    /// SMS EEPROM 93C46 mapper
    SmsEeprom,
    /// SMS Terebi Oekaki mapper
    SmsTerebi,
    /// SMS RAM mappers
    SmsRam,
    
    /// Unknown mapper type
    Unknown,
}

impl std::fmt::Display for MapperType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MapperType::Standard => write!(f, "Standard (SEGA)"),
            MapperType::Realtec => write!(f, "Realtec"),
            MapperType::Sf001 => write!(f, "SF-001"),
            MapperType::Sf002 => write!(f, "SF-002"),
            MapperType::Sf004 => write!(f, "SF-004"),
            MapperType::T5740 => write!(f, "T-5740"),
            MapperType::Flash => write!(f, "Flash"),
            MapperType::Radica => write!(f, "Radica"),
            MapperType::Custom => write!(f, "Custom"),
            MapperType::SmsSega => write!(f, "SMS SEGA"),
            MapperType::SmsCodemasters => write!(f, "SMS Codemasters"),
            MapperType::SmsKorean => write!(f, "SMS Korean"),
            MapperType::SmsMsx => write!(f, "SMS MSX"),
            MapperType::SmsMulti => write!(f, "SMS Multi"),
            MapperType::SmsZemina => write!(f, "SMS Zemina"),
            MapperType::SmsEeprom => write!(f, "SMS EEPROM"),
            MapperType::SmsTerebi => write!(f, "SMS Terebi Oekaki"),
            MapperType::SmsRam => write!(f, "SMS RAM"),
            MapperType::Unknown => write!(f, "Unknown"),
        }
    }
}

/// SMS system types for mapper detection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsSystemType {
    SG1000,
    SG1000II,
    SMS,
    SMS2,
    GG,
    GGMS,
    MarkIII,
    PBC,
}

/// SMS peripheral types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmsPeripheral {
    None,
    LightPhaser,
    Paddle,
    SportsPad,
    MasterTap,
    GraphicBoard,
}

impl From<u8> for SmsPeripheral {
    fn from(value: u8) -> Self {
        match value {
            1 => SmsPeripheral::LightPhaser,
            2 => SmsPeripheral::Paddle,
            3 => SmsPeripheral::SportsPad,
            4 => SmsPeripheral::MasterTap,
            5 => SmsPeripheral::GraphicBoard,
            _ => SmsPeripheral::None,
        }
    }
}

impl From<SmsPeripheral> for u8 {
    fn from(peripheral: SmsPeripheral) -> Self {
        match peripheral {
            SmsPeripheral::None => 0,
            SmsPeripheral::LightPhaser => 1,
            SmsPeripheral::Paddle => 2,
            SmsPeripheral::SportsPad => 3,
            SmsPeripheral::MasterTap => 4,
            SmsPeripheral::GraphicBoard => 5,
        }
    }
}

/// SMS mapper configuration
#[derive(Debug, Clone)]
pub struct SmsMapperConfig {
    pub system: SmsSystemType,
    pub peripheral: SmsPeripheral,
    pub has_fm: bool,
    pub has_3d_glasses: bool,
    pub mapper_id: u8,
    pub rom_pages: u16,
    pub fcr: [u8; 4], // Frame Control Registers
}

impl Default for SmsMapperConfig {
    fn default() -> Self {
        Self {
            system: SmsSystemType::SMS,
            peripheral: SmsPeripheral::None,
            has_fm: false,
            has_3d_glasses: false,
            mapper_id: MAPPER_NONE,
            rom_pages: 0,
            fcr: [0; 4],
        }
    }
}

/// Unified mapper configuration for all systems
#[derive(Debug, Clone)]
pub struct MapperConfig {
    pub mapper_type: MapperType,
    
    // Mega Drive/Genesis specific
    pub has_sram: bool,
    pub sram_start: u32,
    pub sram_end: u32,
    pub sram_custom: bool,
    pub bankshift: bool,
    pub special_hardware: u8,
    
    // SMS/GG specific
    pub sms_config: Option<SmsMapperConfig>,
    
    // Common
    pub rom_size: usize,
    pub rom_mask: u32,
}

impl Default for MapperConfig {
    fn default() -> Self {
        Self {
            mapper_type: MapperType::Standard,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: false,
            special_hardware: 0,
            sms_config: None,
            rom_size: 0,
            rom_mask: 0,
        }
    }
}

/// Information about a detected ROM/game
#[derive(Debug, Clone)]
pub struct RomInfo {
    pub crc: u32,
    pub name: String,
    pub system: SmsSystemType,
    pub mapper_id: u8,
    pub peripheral: SmsPeripheral,
    pub has_fm: bool,
    pub has_3d_glasses: bool,
    pub region: crate::core::system::Region,
}

/// Trait for SMS-specific mapper functionality
pub trait SmsMapper {
    /// Get SMS-specific configuration
    fn sms_config(&self) -> &SmsMapperConfig;
    
    /// Setup Z80 memory map for SMS
    fn setup_z80_memory(&mut self, memory_map: &mut Z80MemoryMap);
    
    /// Handle SMS I/O port writes
    fn write_io_port(&mut self, port: u8, value: u8);
    
    /// Handle SMS I/O port reads
    fn read_io_port(&self, port: u8) -> u8;
    
    /// Reset SMS mapper state
    fn sms_reset(&mut self);
    
    /// Get ROM size in pages
    fn rom_pages(&self) -> u16;
    
    /// Write to mapper registers
    fn write_mapper(&mut self, address: u16, value: u8);
    
    /// Read from mapper (for protected areas)
    fn read_mapper(&self, address: u16) -> u8;
}

/// Trait for all cartridge mappers (unified interface)
pub trait CartridgeMapper: Send + Sync {
    /// Initialize the mapper
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap);
    
    /// Reset the mapper
    fn reset(&mut self, hard_reset: bool);
    
    /// Get mapper type
    fn mapper_type(&self) -> MapperType;
    
    /// Get mapper configuration
    fn config(&self) -> &MapperConfig;
    
    /// Handle !TIME signal write ($A130xx) - Mega Drive specific
    fn handle_time_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap);
    
    /// Handle !TIME signal read ($A130xx) - Mega Drive specific
    fn handle_time_read(&self, address: u32) -> u32;
    
    /// Handle cartridge register write - Mega Drive specific
    fn handle_register_write(&mut self, address: u32, data: u32, memory_map: &mut MemoryMap);
    
    /// Handle cartridge register read - Mega Drive specific
    fn handle_register_read(&self, address: u32) -> u32;
    
    /// Save mapper state
    fn save_state(&self) -> Vec<u8>;
    
    /// Load mapper state
    fn load_state(&mut self, data: &[u8]) -> bool;
    
    /// Update memory mapping after bank change
    fn update_mapping(&mut self, memory_map: &mut MemoryMap);
    
    /// Check if this is an SMS/GG mapper
    fn is_sms_mapper(&self) -> bool;
    
    /// Get SMS mapper interface (if supported)
    fn as_sms_mapper(&mut self) -> Option<&mut dyn SmsMapper>;
    
    /// Get cartridge RAM size (for save states)
    fn ram_size(&self) -> usize;
    
    /// Read from cartridge RAM (for save states)
    fn read_ram(&self, address: usize) -> Option<u8>;
    
    /// Write to cartridge RAM (for save states)
    fn write_ram(&mut self, address: usize, value: u8) -> bool;
}

/// Base mapper implementation with common functionality for Mega Drive
pub struct BaseMapper {
    pub config: MapperConfig,
    pub rom_data: Vec<u8>,
    pub rom_mask: u32,
    pub current_bank: u32,
    pub regs: [u8; 4],
    pub sram: Option<Vec<u8>>,
}

impl BaseMapper {
    pub fn new() -> Self {
        Self {
            config: MapperConfig::default(),
            rom_data: Vec::new(),
            rom_mask: 0,
            current_bank: 0,
            regs: [0; 4],
            sram: None,
        }
    }
    
    pub fn setup_rom_mirroring(&mut self, rom_data: &[u8]) {
        let rom_size = rom_data.len() as u32;
        self.config.rom_size = rom_data.len();
        
        let mut size = 0x10000;
        while rom_size > size {
            size <<= 1;
        }
        
        self.rom_mask = if rom_size < size {
            size - 1
        } else {
            rom_size - 1
        };
        self.config.rom_mask = self.rom_mask;
        
        self.rom_data = rom_data.to_vec();
    }
    
    pub fn map_rom_bank(&self, bank: u32, memory_map: &mut MemoryMap, start_addr: u32, size: u32) {
        let bank_size = size;
        let rom_addr = (bank * bank_size) & self.rom_mask;
        
        if rom_addr < self.rom_data.len() as u32 {
            for i in 0..(size >> 16) {
                let map_addr = start_addr + (i << 16);
                let rom_offset = rom_addr + (i << 16);
                
                if rom_offset < self.rom_data.len() as u32 {
                    memory_map.map_rom(map_addr >> 16, &self.rom_data[rom_offset as usize..]);
                }
            }
        }
    }
    
    pub fn init_sram(&mut self, start_addr: u32, end_addr: u32) {
        self.config.has_sram = true;
        self.config.sram_start = start_addr;
        self.config.sram_end = end_addr;
        
        let sram_size = (end_addr - start_addr + 1) as usize;
        self.sram = Some(vec![0; sram_size]);
    }
}

impl Default for BaseMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// SMS base mapper implementation
pub struct SmsBaseMapper {
    pub config: MapperConfig,
    pub sms_config: SmsMapperConfig,
    pub rom_data: Vec<u8>,
    pub rom_pages: u16,
    pub fcr: [u8; 4],
    pub ram: Option<Vec<u8>>,
}

impl SmsBaseMapper {
    pub fn new() -> Self {
        Self {
            config: MapperConfig::default(),
            sms_config: SmsMapperConfig::default(),
            rom_data: Vec::new(),
            rom_pages: 0,
            fcr: [0; 4],
            ram: None,
        }
    }
    
    pub fn setup_rom(&mut self, rom_data: &[u8], mapper_id: u8) {
        self.rom_data = rom_data.to_vec();
        self.config.rom_size = rom_data.len();
        self.sms_config.mapper_id = mapper_id;
        
        // Calculate ROM pages based on mapper type
        self.rom_pages = self.calculate_rom_pages(rom_data.len(), mapper_id);
        self.sms_config.rom_pages = self.rom_pages;
        
        // Initialize FCR based on mapper type
        self.reset_fcr();
    }
    
    fn calculate_rom_pages(&self, rom_size: usize, mapper_id: u8) -> u16 {
        match mapper_id {
            m if m < MAPPER_SEGA => {
                // 1KB pages
                ((rom_size + (1 << 10) - 1) >> 10) as u16
            }
            m if (m & MAPPER_KOREA_8K) != 0 => {
                // 8KB pages
                ((rom_size + (1 << 13) - 1) >> 13) as u16
            }
            m if (m & MAPPER_MULTI_32K) != 0 => {
                // 32KB pages
                ((rom_size + (1 << 15) - 1) >> 15) as u16
            }
            _ => {
                // 16KB pages (default)
                ((rom_size + (1 << 14) - 1) >> 14) as u16
            }
        }
    }
    
    fn reset_fcr(&mut self) {
        match self.sms_config.mapper_id {
            MAPPER_SEGA | MAPPER_SEGA_X => {
                self.fcr = [0, 0, 1, 2];
            }
            MAPPER_ZEMINA_16K_32K => {
                self.fcr = [0, 0, 1, 1];
            }
            MAPPER_ZEMINA_4X8K => {
                self.fcr = [3, 2, 1, 0];
            }
            MAPPER_KOREA_8K | MAPPER_MSX_8K | MAPPER_MSX_8K_NEMESIS |
            MAPPER_MSX_16K | MAPPER_MULTI_4X8K | MAPPER_MULTI_8K => {
                self.fcr = [0; 4];
            }
            _ => {
                self.fcr = [0, 0, 1, 0];
            }
        }
        self.sms_config.fcr = self.fcr;
    }
    
    pub fn init_ram(&mut self, ram_size: usize) {
        self.ram = Some(vec![0; ram_size]);
    }
    
    pub fn mapper_8k_write(&mut self, offset: usize, data: u8, z80_memory: &mut Z80MemoryMap) {
        let page = (data as usize % self.rom_pages as usize) << 13;
        self.fcr[offset & 3] = data;
        
        match offset & 3 {
            0 => { // $8000-$9FFF
                for i in 0x20..0x28 {
                    let addr = page + ((i & 0x07) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            1 => { // $A000-$BFFF
                for i in 0x28..0x30 {
                    let addr = page + ((i & 0x07) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            2 => { // $4000-$5FFF
                for i in 0x10..0x18 {
                    let addr = page + ((i & 0x07) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            3 => { // $6000-$7FFF
                for i in 0x18..0x20 {
                    let addr = page + ((i & 0x07) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            _ => {}
        }
    }
    
    pub fn mapper_16k_write(&mut self, offset: usize, data: u8, z80_memory: &mut Z80MemoryMap) {
        let mut page = data as usize % self.rom_pages as usize;
        
        // Page increment for SEGA mapper
        if self.sms_config.mapper_id == MAPPER_SEGA && self.fcr[0] & 0x03 != 0 {
            page = (page + ((4 - (self.fcr[0] & 0x03)) << 3)) % self.rom_pages as usize;
        }
        
        self.fcr[offset] = data;
        
        match offset {
            0 => { // Control register
                // Handle RAM/ROM switching
                // Implementation depends on specific mapper
            }
            1 => { // $0000-$3FFF
                for i in 0x01..0x10 {
                    let addr = (page << 14) | ((i & 0x0F) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            2 => { // $4000-$7FFF
                for i in 0x10..0x20 {
                    let addr = (page << 14) | ((i & 0x0F) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            3 => { // $8000-$BFFF
                for i in 0x20..0x30 {
                    let addr = (page << 14) | ((i & 0x0F) << 10);
                    if addr < self.rom_data.len() {
                        z80_memory.map_read(i, &self.rom_data[addr..]);
                    }
                }
            }
            _ => {}
        }
    }
    
    pub fn mapper_32k_write(&mut self, data: u8, z80_memory: &mut Z80MemoryMap) {
        let page = (data as usize % self.rom_pages as usize) << 15;
        self.fcr[0] = data;
        
        // Map 32KB at $0000-$7FFF
        for i in 0x00..0x20 {
            let addr = page + (i << 10);
            if addr < self.rom_data.len() {
                z80_memory.map_read(i, &self.rom_data[addr..]);
            }
        }
        
        // Mirror lower 16KB at $8000-$BFFF
        for i in 0x20..0x30 {
            let addr = page + ((i & 0x0F) << 10);
            if addr < self.rom_data.len() {
                z80_memory.map_read(i, &self.rom_data[addr..]);
            }
        }
    }
}

impl Default for SmsBaseMapper {
    fn default() -> Self {
        Self::new()
    }
}

/// Common utilities for mapper detection and handling
pub mod utils {
    use super::*;
    use crate::core::system::Region;
    
    /// Detect if ROM is for SMS/GG system
    pub fn is_sms_rom(rom_info: &crate::core::cartridge::rom::RomInfo, rom_data: &[u8]) -> bool {
        // Check ROM size (SMS/GG ROMs are typically smaller)
        if rom_data.len() > 0x400000 { // 4MB
            return false;
        }
        
        // Check file extension hints
        let filename = rom_info.filename.to_lowercase();
        if filename.ends_with(".sms") || 
           filename.ends_with(".gg") || 
           filename.ends_with(".sg") ||
           filename.ends_with(".sc") {
            return true;
        }
        
        // Check for known SMS/GG header signatures
        if rom_data.len() > 0x7FF0 {
            // Check for "TMR SEGA" signature
            let header_pos = if rom_data.len() > 0x8000 { 0x7FF0 } else { 0x7FF0 - 0x200 };
            if rom_data.len() > header_pos + 8 {
                let signature = &rom_data[header_pos..header_pos + 8];
                if signature == b"TMR SEGA" {
                    return true;
                }
            }
        }
        
        false
    }
    
    /// Detect SMS mapper from ROM CRC
    pub fn detect_sms_mapper(crc: u32) -> Option<u8> {
        // This would be populated from the game database
        // For now, return None to use auto-detection
        None
    }
    
    /// Calculate ROM CRC32
    pub fn calculate_crc(rom_data: &[u8]) -> u32 {
        crc32::calculate(rom_data)
    }
    
    /// Convert region string to Region enum
    pub fn parse_region(region_str: &str) -> Region {
        match region_str.to_uppercase().as_str() {
            "JAPAN" | "JP" => Region::JapanNTSC,
            "USA" | "US" => Region::USA,
            "EUROPE" | "EU" | "PAL" => Region::Europe,
            "BRAZIL" | "BR" => Region::Brazil,
            "KOREA" | "KR" => Region::Korea,
            "ASIA" | "TW" => Region::Taiwan,
            _ => Region::USA,
        }
    }
    
    /// Detect SMS system type from filename
    pub fn detect_sms_system(filename: &str) -> SmsSystemType {
        let filename = filename.to_lowercase();
        
        if filename.ends_with(".gg") {
            SmsSystemType::GG
        } else if filename.ends_with(".sg") {
            SmsSystemType::SG1000
        } else if filename.ends_with(".sc") {
            SmsSystemType::SG1000II
        } else {
            // Default to Master System
            SmsSystemType::SMS
        }
    }
}

/// Helper functions for working with mappers
pub mod helpers {
    use super::*;
    
    /// Create a standard mapper for Mega Drive
    pub fn create_standard_mapper() -> BaseMapper {
        BaseMapper::new()
    }
    
    /// Create an SMS mapper based on mapper ID
    pub fn create_sms_mapper(mapper_id: u8) -> SmsBaseMapper {
        let mut mapper = SmsBaseMapper::new();
        mapper.sms_config.mapper_id = mapper_id;
        mapper
    }
    
    /// Check if mapper ID is for SMS
    pub fn is_sms_mapper_id(mapper_id: u8) -> bool {
        mapper_id <= MAPPER_HICOM
    }
    
    /// Get mapper type from mapper ID
    pub fn mapper_id_to_type(mapper_id: u8) -> MapperType {
        match mapper_id {
            MAPPER_SEGA | MAPPER_SEGA_X => MapperType::SmsSega,
            MAPPER_CODIES => MapperType::SmsCodemasters,
            MAPPER_KOREA_8K | MAPPER_KOREA_16K_V1 | MAPPER_KOREA_16K_V2 => MapperType::SmsKorean,
            MAPPER_MSX_8K | MAPPER_MSX_8K_NEMESIS | MAPPER_MSX_16K => MapperType::SmsMsx,
            MAPPER_MULTI_8K | MAPPER_MULTI_4X8K | MAPPER_MULTI_16K | 
            MAPPER_MULTI_2X16K_V1 | MAPPER_MULTI_2X16K_V2 |
            MAPPER_MULTI_16K_32K_V1 | MAPPER_MULTI_16K_32K_V2 |
            MAPPER_MULTI_32K | MAPPER_MULTI_32K_16K | MAPPER_HICOM => MapperType::SmsMulti,
            MAPPER_ZEMINA_4X8K | MAPPER_ZEMINA_16K_32K => MapperType::SmsZemina,
            MAPPER_93C46 => MapperType::SmsEeprom,
            MAPPER_TEREBI => MapperType::SmsTerebi,
            MAPPER_RAM_2K | MAPPER_RAM_8K | MAPPER_RAM_8K_EXT1 => MapperType::SmsRam,
            _ => MapperType::Unknown,
        }
    }
}