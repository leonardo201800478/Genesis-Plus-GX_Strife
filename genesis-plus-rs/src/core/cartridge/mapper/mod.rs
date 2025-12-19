// genesis-plus-rs/src/core/cartridge/mapper/mod.rs

//! Cartridge mapper module
//!
//! This module implements various cartridge mappers and protection schemes
//! used in Genesis/Mega Drive games.

pub mod mapper_common;
pub mod mapper_database;
pub mod mapper_handlers;
pub mod mapper_realtec;
pub mod mapper_sf;
pub mod mapper_flash;
pub mod mapper_radica;
pub mod mapper_custom;

// SMS mappers
pub mod sms;
pub mod sms_common;
pub mod sms_sega;
pub mod sms_codemasters;
pub mod sms_korean;
pub mod sms_msx;
pub mod sms_multi;
pub mod sms_zemina;
pub mod sms_eeprom;
pub mod sms_terebi;
pub mod sms_ram;

// Re-export types and functions
pub use mapper_common::{CartridgeMapper, MapperType, MapperConfig};
pub use mapper_database::{detect_mapper, CartridgeDatabaseEntry};
pub use mapper_handlers::{setup_memory_map, handle_time_signal, handle_registers};
pub use mapper_realtec::RealtecMapper;
pub use mapper_sf::{Sf001Mapper, Sf002Mapper, Sf004Mapper, T5740Mapper};
pub use mapper_flash::FlashMapper;
pub use mapper_radica::RadicaMapper;
pub use mapper_custom::CustomMapper;

// Re-export SMS mapper types
pub use sms::{
    SmsCartridge, RomInfo, RomHardware, MemorySlot,
    MAPPER_NONE, MAPPER_TEREBI, MAPPER_RAM_2K, MAPPER_RAM_8K, MAPPER_RAM_8K_EXT1,
    MAPPER_SEGA, MAPPER_SEGA_X, MAPPER_93C46, MAPPER_CODIES, MAPPER_MULTI_16K,
    MAPPER_KOREA_16K_V1, MAPPER_KOREA_16K_V2, MAPPER_MULTI_2X16K_V1, MAPPER_MULTI_2X16K_V2,
    MAPPER_MULTI_16K_32K_V1, MAPPER_MULTI_16K_32K_V2, MAPPER_ZEMINA_16K_32K, MAPPER_HWASUNG,
    MAPPER_MSX_16K, MAPPER_KOREA_8K, MAPPER_MSX_8K, MAPPER_MSX_8K_NEMESIS, MAPPER_MULTI_8K,
    MAPPER_MULTI_4X8K, MAPPER_ZEMINA_4X8K, MAPPER_MULTI_32K, MAPPER_MULTI_32K_16K, MAPPER_HICOM,
};
pub use sms_sega::SegaMapper;
pub use sms_codemasters::CodemastersMapper;
pub use sms_korean::{Korean8kMapper, Korean16kV1Mapper, Korean16kV2Mapper};
pub use sms_msx::{Msx8kMapper, Msx16kMapper};
pub use sms_multi::{Multi16kMapper, Multi2x16kV1Mapper, Multi8kMapper};
pub use sms_zemina::{Zemina4x8kMapper, Zemina16k32kMapper};
pub use sms_eeprom::Eeprom93c46;
pub use sms_terebi::TerebiOekakiMapper;
pub use sms_ram::{Ram2kMapper, Ram8kMapper, Ram8kExtMapper};

use crate::core::cartridge::rom::RomInfo;
use crate::core::mem::MemoryMap;
use crate::core::m68k::M68K;
use crate::core::system::{System, Region};
use crate::core::z80::memory::Z80MemoryMap;
use log::{debug, info, warn};

/// SMS system types for mapper detection
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// SMS mapper configuration
#[derive(Debug, Clone)]
pub struct SmsMapperConfig {
    pub system: SmsSystemType,
    pub region: Region,
    pub has_fm: bool,
    pub has_3d_glasses: bool,
    pub peripheral: u8,
}

/// Initialize cartridge mapper based on ROM information
pub fn init_mapper(rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) -> Option<Box<dyn CartridgeMapper>> {
    // Check if this is a SMS/GG ROM
    if is_sms_rom(rom_info, rom_data) {
        // Handle SMS/GG mappers separately
        return init_sms_mapper(rom_info, rom_data, memory_map);
    }
    
    // Handle Mega Drive/Genesis mappers
    let mapper_type = detect_mapper(rom_info, rom_data);
    
    match mapper_type {
        MapperType::Realtec => {
            info!("Initializing Realtec mapper");
            let mut mapper = RealtecMapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Sf001 => {
            info!("Initializing SF-001 mapper");
            let mut mapper = Sf001Mapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Sf002 => {
            info!("Initializing SF-002 mapper");
            let mut mapper = Sf002Mapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Sf004 => {
            info!("Initializing SF-004 mapper");
            let mut mapper = Sf004Mapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::T5740 => {
            info!("Initializing T-5740 mapper");
            let mut mapper = T5740Mapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Flash => {
            info!("Initializing Flash mapper");
            let mut mapper = FlashMapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Radica => {
            info!("Initializing Radica mapper");
            let mut mapper = RadicaMapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Custom => {
            info!("Initializing custom mapper");
            let mut mapper = CustomMapper::new();
            mapper.init(rom_info, rom_data, memory_map);
            Some(Box::new(mapper))
        }
        MapperType::Standard => {
            debug!("Using standard cartridge mapping");
            setup_standard_mapping(rom_info, rom_data, memory_map);
            None
        }
        MapperType::Unknown => {
            warn!("Unknown mapper type, using standard mapping");
            setup_standard_mapping(rom_info, rom_data, memory_map);
            None
        }
    }
}

/// Initialize SMS/GG cartridge mapper
pub fn init_sms_mapper(
    rom_info: &RomInfo, 
    rom_data: &[u8], 
    memory_map: &mut MemoryMap
) -> Option<Box<dyn CartridgeMapper>> {
    info!("Initializing SMS/GG cartridge mapper");
    
    // Detect SMS system type from ROM info
    let system = detect_sms_system(rom_info);
    let region = detect_sms_region(rom_info);
    
    // Create SMS cartridge context
    let mut sms_cart = SmsCartridge::new();
    
    // Convert to SMS-friendly ROM info
    let sms_rom_info = convert_to_sms_rom_info(rom_info, rom_data);
    
    // Initialize SMS mapper
    sms_cart.init(&sms_rom_info, system, region);
    
    // For now, return None as SMS uses different memory handling
    // In practice, this would return a Box<dyn CartridgeMapper> that wraps SmsCartridge
    None
}

/// Setup standard cartridge mapping
fn setup_standard_mapping(rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
    // Calculate ROM size and mask
    let rom_size = rom_data.len();
    let mut size = 0x10000;
    
    while rom_size > size {
        size <<= 1;
    }
    
    // Handle special cases
    if rom_info.international.contains("SONIC & KNUCKLES") {
        // Sonic & Knuckles: disable ROM mirroring at $200000-$3fffff
        size = 0x400000;
    }
    
    let mask = if rom_size < size {
        // Pad ROM to next power of 2
        size - 1
    } else {
        rom_size - 1
    };
    
    // Setup default memory mapping
    for i in 0..0x40 {
        let offset = (i << 16) & mask;
        if offset < rom_data.len() {
            memory_map.map_rom(i as u32, &rom_data[offset..]);
        }
    }
}

/// Check if ROM is for SMS/GG system
fn is_sms_rom(rom_info: &RomInfo, rom_data: &[u8]) -> bool {
    // Check ROM size (SMS/GG ROMs are typically smaller)
    if rom_data.len() > 0x400000 { // 4MB
        return false;
    }
    
    // Check file extension hints
    let filename = rom_info.filename.to_lowercase();
    if filename.ends_with(".sms") || 
       filename.ends_with(".gg") || 
       filename.ends_with(".sg") {
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

/// Detect SMS system type from ROM info
fn detect_sms_system(rom_info: &RomInfo) -> SmsSystemType {
    let filename = rom_info.filename.to_lowercase();
    
    if filename.ends_with(".gg") {
        SmsSystemType::GG
    } else if filename.ends_with(".sg") {
        SmsSystemType::SG1000
    } else {
        // Default to Master System
        SmsSystemType::SMS
    }
}

/// Detect SMS region from ROM info
fn detect_sms_region(rom_info: &RomInfo) -> Region {
    // Parse region from ROM info
    if rom_info.international.contains("JAPAN") || 
       rom_info.domestic.contains("JAPAN") {
        Region::JapanNTSC
    } else if rom_info.international.contains("EUROPE") || 
              rom_info.domestic.contains("EUROPE") {
        Region::Europe
    } else {
        Region::USA
    }
}

/// Convert standard ROM info to SMS ROM info
fn convert_to_sms_rom_info(rom_info: &RomInfo, rom_data: &[u8]) -> crate::core::cartridge::mapper::sms::RomInfo {
    use crate::core::cartridge::mapper::sms::RomInfo as SmsRomInfo;
    use crate::core::system::System;
    
    let crc = crate::utils::crc32::calculate(rom_data);
    
    SmsRomInfo {
        crc,
        g_3d: false, // Will be auto-detected
        fm: false,    // Will be auto-detected
        peripheral: 0,
        mapper: 0,    // Will be auto-detected
        system: System::SMS,
        region: Region::USA,
    }
}

/// Reset mapper state
pub fn reset_mapper(mapper: Option<&mut Box<dyn CartridgeMapper>>, hard_reset: bool) {
    if let Some(mapper) = mapper {
        mapper.reset(hard_reset);
    }
}

/// Save mapper state
pub fn save_mapper_state(mapper: Option<&Box<dyn CartridgeMapper>>) -> Vec<u8> {
    if let Some(mapper) = mapper {
        mapper.save_state()
    } else {
        Vec::new()
    }
}

/// Load mapper state
pub fn load_mapper_state(mapper: Option<&mut Box<dyn CartridgeMapper>>, data: &[u8]) -> bool {
    if let Some(mapper) = mapper {
        mapper.load_state(data)
    } else {
        true
    }
}

/// Trait for SMS-specific mapper functionality
pub trait SmsMapper: CartridgeMapper {
    /// Get SMS-specific configuration
    fn sms_config(&self) -> &SmsMapperConfig;
    
    /// Setup Z80 memory map for SMS
    fn setup_z80_memory(&self, memory_map: &mut Z80MemoryMap);
    
    /// Handle SMS I/O port writes
    fn write_io_port(&mut self, port: u8, value: u8);
    
    /// Handle SMS I/O port reads
    fn read_io_port(&self, port: u8) -> u8;
}

/// SMS cartridge mapper implementation
pub struct SmsCartridgeMapper {
    cart: SmsCartridge,
    config: SmsMapperConfig,
}

impl SmsCartridgeMapper {
    pub fn new(cart: SmsCartridge, config: SmsMapperConfig) -> Self {
        Self { cart, config }
    }
}

impl CartridgeMapper for SmsCartridgeMapper {
    fn init(&mut self, rom_info: &RomInfo, rom_data: &[u8], memory_map: &mut MemoryMap) {
        // SMS uses Z80 memory map, not M68K
        // The actual initialization happens in SmsCartridge::init()
    }
    
    fn reset(&mut self, hard_reset: bool) {
        // Reset SMS cartridge state
    }
    
    fn save_state(&self) -> Vec<u8> {
        Vec::new() // TODO: Implement SMS state saving
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        true // TODO: Implement SMS state loading
    }
    
    fn get_type(&self) -> MapperType {
        MapperType::Custom
    }
}

impl SmsMapper for SmsCartridgeMapper {
    fn sms_config(&self) -> &SmsMapperConfig {
        &self.config
    }
    
    fn setup_z80_memory(&self, memory_map: &mut Z80MemoryMap) {
        // Setup Z80 memory mapping based on SMS cartridge state
    }
    
    fn write_io_port(&mut self, port: u8, value: u8) {
        // Handle SMS I/O port writes
        match port {
            0x00..=0x06 => { /* SMS VDP registers */ }
            0x07 => { /* SMS VDP data port */ }
            0x40..=0x7F => { /* SMS PSG */ }
            0x80..=0xBF => { /* SMS VDP */ }
            0xC0..=0xDF => { /* SMS I/O control */ }
            0xE0..=0xFF => { /* SMS memory control */ }
            _ => {}
        }
    }
    
    fn read_io_port(&self, port: u8) -> u8 {
        // Handle SMS I/O port reads
        match port {
            0x00..=0x06 => 0xFF, /* SMS VDP registers */
            0x07 => 0xFF, /* SMS VDP data port */
            0x40..=0x7F => 0xFF, /* SMS PSG */
            0x80..=0xBF => 0xFF, /* SMS VDP */
            0xC0..=0xDF => 0xFF, /* SMS I/O control */
            0xE0..=0xFF => 0xFF, /* SMS memory control */
            _ => 0xFF,
        }
    }
}