// genesis-plus-rs/src/core/cartridge/eeprom/mod.rs

//! EEPROM support module
//!
//! This module implements various types of EEPROM memory used in Genesis/Mega Drive cartridges.
//! It supports Microwire (93C46), SPI (25xxx/95xxx), and I2C (24Cxx) EEPROMs.

pub mod eeprom_93c;
pub mod eeprom_i2c;
pub mod eeprom_spi;

// Re-export types
pub use eeprom_93c::{Eeprom93C, Eeprom93CState};
pub use eeprom_i2c::{EepromI2C, EepromI2CType, EepromI2CState};
pub use eeprom_spi::{EepromSPI, EepromSPIState};

use crate::core::cartridge::sram::BackupRam;
use log::{debug, info, warn};

/// EEPROM type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromType {
    /// No EEPROM
    None,
    /// Microwire 93C46 EEPROM
    Microwire93C46,
    /// SPI EEPROM (25xxx/95xxx series)
    Spi,
    /// I2C EEPROM (24Cxx series)
    I2C(EepromI2CType),
}

impl std::fmt::Display for EepromType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EepromType::None => write!(f, "None"),
            EepromType::Microwire93C46 => write!(f, "Microwire 93C46"),
            EepromType::Spi => write!(f, "SPI EEPROM"),
            EepromType::I2C(typ) => write!(f, "I2C EEPROM ({:?})", typ),
        }
    }
}

/// Trait for all EEPROM implementations
pub trait Eeprom {
    /// Initialize the EEPROM
    fn init(&mut self, sram: &mut BackupRam);
    
    /// Write data to the EEPROM control lines
    fn write(&mut self, data: u8, sram: &mut BackupRam);
    
    /// Read data from the EEPROM
    fn read(&self, address: u32) -> u32;
    
    /// Reset the EEPROM to its initial state
    fn reset(&mut self);
    
    /// Get the EEPROM type
    fn eeprom_type(&self) -> EepromType;
    
    /// Save EEPROM state
    fn save_state(&self) -> Vec<u8>;
    
    /// Load EEPROM state
    fn load_state(&mut self, data: &[u8]) -> bool;
}

/// Factory function to create an EEPROM of the specified type
pub fn create_eeprom(eeprom_type: EepromType) -> Box<dyn Eeprom> {
    match eeprom_type {
        EepromType::Microwire93C46 => {
            info!("Creating Microwire 93C46 EEPROM");
            Box::new(Eeprom93C::new())
        }
        EepromType::Spi => {
            info!("Creating SPI EEPROM");
            Box::new(EepromSPI::new())
        }
        EepromType::I2C(i2c_type) => {
            info!("Creating I2C EEPROM type: {:?}", i2c_type);
            Box::new(EepromI2C::new(i2c_type))
        }
        EepromType::None => {
            warn!("No EEPROM type specified");
            Box::new(NullEeprom)
        }
    }
}

/// Detect EEPROM type from ROM header
pub fn detect_eeprom_type(rom_data: &[u8], product_code: &str, checksum: u16) -> EepromType {
    // Check for specific product codes that use EEPROM
    let product_lower = product_code.to_lowercase();
    
    // Check for 93C46 games
    if detect_93c46_game(product_code) {
        return EepromType::Microwire93C46;
    }
    
    // Check for SPI EEPROM games
    if detect_spi_game(rom_data, product_code) {
        return EepromType::Spi;
    }
    
    // Check for I2C EEPROM games
    if let Some(i2c_type) = detect_i2c_game(product_code, checksum, rom_data) {
        return EepromType::I2C(i2c_type);
    }
    
    EepromType::None
}

fn detect_93c46_game(product_code: &str) -> bool {
    // Games known to use 93C46 EEPROM
    let known_games = [
        "T-081326", // NBA Jam
        "T-081586", // NFL Quarterback Club '96
        "T-081276", // NFL Quarterback Club
        "T-81406",  // NBA Jam TE
        "T-81476",  // Frank Thomas Big Hurt Baseball
        "T-81576",  // College Slam
    ];
    
    known_games.iter().any(|&code| product_code.contains(code))
}

fn detect_spi_game(rom_data: &[u8], product_code: &str) -> bool {
    // Check ROM header for SPI EEPROM indicator
    if rom_data.len() > 0x1B2 {
        let header_byte = rom_data[0x1B2];
        if header_byte == 0xE8 {
            return true;
        }
    }
    
    // Check specific product codes
    let known_games = [
        "T-12053", // Rockman Mega World
        "T-12046", // Megaman - The Wily Wars
    ];
    
    known_games.iter().any(|&code| product_code.contains(code))
}

fn detect_i2c_game(product_code: &str, checksum: u16, rom_data: &[u8]) -> Option<EepromI2CType> {
    // Database of I2C EEPROM games from the C code
    let games = [
        ("T-50176", 0, 0x0000, EepromI2CType::X24C01), // Rings of Power
        ("T-50396", 0, 0x0000, EepromI2CType::X24C01), // NHLPA Hockey 93
        ("T-50446", 0, 0x0000, EepromI2CType::X24C01), // John Madden Football 93
        ("T-50516", 0, 0x0000, EepromI2CType::X24C01), // John Madden Football 93 (Championship Ed.)
        ("T-50606", 0, 0x0000, EepromI2CType::X24C01), // Bill Walsh College Football
        (" T-12046", 0, 0x0000, EepromI2CType::X24C01), // Megaman - The Wily Wars
        (" T-12053", 0, 0x0000, EepromI2CType::X24C01), // Rockman Mega World
        ("MK-1215", 0, 0x0000, EepromI2CType::X24C01), // Evander 'Real Deal' Holyfield's Boxing
        ("MK-1228", 0, 0x0000, EepromI2CType::X24C01), // Greatest Heavyweights of the Ring (U)(E)
        ("G-5538", 0, 0x0000, EepromI2CType::X24C01), // Greatest Heavyweights of the Ring (J)
        ("PR-1993", 0, 0x0000, EepromI2CType::X24C01), // Greatest Heavyweights of the Ring (Prototype)
        (" G-4060", 0, 0x0000, EepromI2CType::X24C01), // Wonderboy in Monster World
        ("00001211", 0, 0x0000, EepromI2CType::X24C01), // Sports Talk Baseball
        ("00004076", 0, 0x0000, EepromI2CType::X24C01), // Honoo no Toukyuuji Dodge Danpei
        ("G-4524", 0, 0x0000, EepromI2CType::X24C01), // Ninja Burai Densetsu
        ("00054503", 0, 0x0000, EepromI2CType::X24C01), // Game Toshokan
        ("T-81033", 0, 0x0000, EepromI2CType::X24C02), // NBA Jam (J)
        ("T-081326", 0, 0x0000, EepromI2CType::X24C02), // NBA Jam (UE)
        ("T-081276", 0, 0x0000, EepromI2CType::C24C02), // NFL Quarterback Club
        ("T-81406", 0, 0x0000, EepromI2CType::C24C04), // NBA Jam TE
        ("T-081586", 0, 0x0000, EepromI2CType::C24C16), // NFL Quarterback Club '96
        ("T-81476", 0, 0x0000, EepromI2CType::C24C65), // Frank Thomas Big Hurt Baseball
        ("T-81576", 0, 0x0000, EepromI2CType::C24C65), // College Slam
        ("T-120106", 0, 0x0000, EepromI2CType::C24C08), // Brian Lara Cricket
        ("T-120096", 0, 0x0000, EepromI2CType::C24C16), // Micro Machines 2 - Turbo Tournament
        ("T-120146", 0, 0x0000, EepromI2CType::C24C65), // Brian Lara Cricket 96 / Shane Warne Cricket
    ];
    
    for (id, sp, chk, eeprom_type) in games.iter() {
        if product_code.contains(id.trim()) {
            // Check checksum if specified
            if *chk == 0 || *chk == checksum {
                // Check special pattern if specified
                if *sp == 0 || check_special_pattern(rom_data, *sp) {
                    return Some(*eeprom_type);
                }
            }
        }
    }
    
    None
}

fn check_special_pattern(rom_data: &[u8], pattern: u32) -> bool {
    if rom_data.len() >= 4 {
        let value = u32::from_le_bytes([
            rom_data[0],
            rom_data[1],
            rom_data[2],
            rom_data[3],
        ]);
        value == pattern
    } else {
        false
    }
}

/// Null EEPROM implementation (no EEPROM present)
struct NullEeprom;

impl Eeprom for NullEeprom {
    fn init(&mut self, _sram: &mut BackupRam) {}
    
    fn write(&mut self, _data: u8, _sram: &mut BackupRam) {}
    
    fn read(&self, _address: u32) -> u32 {
        0
    }
    
    fn reset(&mut self) {}
    
    fn eeprom_type(&self) -> EepromType {
        EepromType::None
    }
    
    fn save_state(&self) -> Vec<u8> {
        Vec::new()
    }
    
    fn load_state(&mut self, _data: &[u8]) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_eeprom_type() {
        // Test product code detection
        let product_code = "T-081326";
        let rom_data = vec![0u8; 0x200];
        let checksum = 0;
        
        let eeprom_type = detect_eeprom_type(&rom_data, product_code, checksum);
        assert!(matches!(eeprom_type, EepromType::I2C(EepromI2CType::X24C02)));
    }
    
    #[test]
    fn test_create_eeprom() {
        let eeprom = create_eeprom(EepromType::Microwire93C46);
        assert_eq!(eeprom.eeprom_type(), EepromType::Microwire93C46);
        
        let eeprom = create_eeprom(EepromType::Spi);
        assert_eq!(eeprom.eeprom_type(), EepromType::Spi);
        
        let eeprom = create_eeprom(EepromType::I2C(EepromI2CType::X24C01));
        assert_eq!(eeprom.eeprom_type(), EepromType::I2C(EepromI2CType::X24C01));
    }
}