// genesis-plus-rs/src/core/cartridge/mapper/mapper_database.rs

use crate::core::cartridge::rom::RomInfo;
use super::mapper_common::{MapperType, MapperConfig};

/// Cartridge database entry
#[derive(Debug, Clone)]
pub struct CartridgeDatabaseEntry {
    pub product_id: &'static str,
    pub checksum: u16,
    pub real_checksum: u16,
    pub mapper_type: MapperType,
    pub special_pattern: u32,
    pub config: MapperConfig,
}

/// Database of known cartridges and their mappers
static CARTRIDGE_DATABASE: &[CartridgeDatabaseEntry] = &[
    // Realtec mapper games
    CartridgeDatabaseEntry {
        product_id: "",
        checksum: 0x0000,
        real_checksum: 0x06AB,
        mapper_type: MapperType::Realtec,
        special_pattern: 0,
        config: MapperConfig {
            mapper_type: MapperType::Realtec,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: false,
            special_hardware: 0,
        },
    },
    CartridgeDatabaseEntry {
        product_id: "",
        checksum: 0xFFFF,
        real_checksum: 0xF863,
        mapper_type: MapperType::Realtec,
        special_pattern: 0,
        config: MapperConfig {
            mapper_type: MapperType::Realtec,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: false,
            special_hardware: 0,
        },
    },
    // SF mapper games
    CartridgeDatabaseEntry {
        product_id: "T-5740",
        checksum: 0,
        real_checksum: 0,
        mapper_type: MapperType::T5740,
        special_pattern: 0,
        config: MapperConfig {
            mapper_type: MapperType::T5740,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: true,
            special_hardware: 0,
        },
    },
    // Flash mapper games
    CartridgeDatabaseEntry {
        product_id: "00000000-42",
        checksum: 0,
        real_checksum: 0,
        mapper_type: MapperType::Flash,
        special_pattern: 0,
        config: MapperConfig {
            mapper_type: MapperType::Flash,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: false,
            special_hardware: 0,
        },
    },
    // Radica mapper games
    CartridgeDatabaseEntry {
        product_id: "",
        checksum: 0x0000,
        real_checksum: 0x2326,
        mapper_type: MapperType::Radica,
        special_pattern: 0,
        config: MapperConfig {
            mapper_type: MapperType::Radica,
            has_sram: false,
            sram_start: 0,
            sram_end: 0,
            sram_custom: false,
            bankshift: false,
            special_hardware: 0,
        },
    },
    // Add more entries as needed...
];

/// Detect mapper type from ROM information
pub fn detect_mapper(rom_info: &RomInfo, rom_data: &[u8]) -> MapperType {
    // Check product code and checksum against database
    for entry in CARTRIDGE_DATABASE {
        if entry.product_id.is_empty() || rom_info.product.contains(entry.product_id) {
            if (entry.checksum == 0 || entry.checksum == rom_info.checksum) &&
               (entry.real_checksum == 0 || entry.real_checksum == rom_info.real_checksum) {
                return entry.mapper_type;
            }
        }
    }
    
    // Check for specific ROM headers
    if rom_data.len() > 0x1C8 {
        // Check for SVP chip
        if rom_data[0x1C8] == b'S' && rom_data[0x1C9] == b'V' {
            return MapperType::Standard; // SVP uses standard mapping
        }
    }
    
    // Check for specific game titles
    if rom_info.international.contains("SONIC & KNUCKLES") {
        return MapperType::Standard;
    }
    
    if rom_info.console_type.contains("SEGA SSF") {
        return MapperType::Custom;
    }
    
    if rom_info.console_type.contains("SEGA SSF2") && rom_data.len() <= 0x800000 {
        return MapperType::Custom;
    }
    
    if rom_info.console_type.contains("SEGA MEGASD") && rom_data.len() <= 0x400000 {
        return MapperType::Custom;
    }
    
    if rom_info.rom_type.contains("SF") {
        if rom_info.product.contains("001") {
            return MapperType::Sf001;
        } else if rom_info.product.contains("002") {
            return MapperType::Sf002;
        } else if rom_info.product.contains("004") {
            return MapperType::Sf004;
        }
    }
    
    // Check for large ROMs
    if rom_data.len() > 0x400000 {
        return MapperType::Custom;
    }
    
    MapperType::Standard
}

/// Get mapper configuration for detected mapper
pub fn get_mapper_config(mapper_type: MapperType, rom_info: &RomInfo) -> MapperConfig {
    for entry in CARTRIDGE_DATABASE {
        if entry.mapper_type == mapper_type {
            return entry.config.clone();
        }
    }
    
    // Default configuration
    MapperConfig {
        mapper_type,
        has_sram: false,
        sram_start: 0,
        sram_end: 0,
        sram_custom: false,
        bankshift: false,
        special_hardware: 0,
    }
}