// genesis-plus-rs/src/core/cartridge/eeprom/eeprom_i2c.rs

use crate::core::cartridge::sram::BackupRam;
use log::{debug, info};

/// I2C EEPROM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromI2CState {
    StandBy,
    WaitStop,
    GetDeviceAdr,
    GetWordAdr7Bits,
    GetWordAdrHigh,
    GetWordAdrLow,
    WriteData,
    ReadData,
}

/// I2C EEPROM type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromI2CType {
    X24C01,
    X24C02,
    C24C01,
    C24C02,
    C24C04,
    C24C08,
    C24C16,
    C24C32,
    C24C64,
    C24C65,
    C24C128,
    C24C256,
    C24C512,
}

/// I2C EEPROM specifications
#[derive(Debug, Clone, Copy)]
struct EepromI2CSpec {
    address_bits: u8,
    size_mask: u16,
    pagewrite_mask: u16,
}

impl EepromI2CSpec {
    fn from_type(eeprom_type: EepromI2CType) -> Self {
        match eeprom_type {
            EepromI2CType::X24C01 => Self { address_bits: 7, size_mask: 0x7F, pagewrite_mask: 0x03 },
            EepromI2CType::X24C02 => Self { address_bits: 8, size_mask: 0xFF, pagewrite_mask: 0x03 },
            EepromI2CType::C24C01 => Self { address_bits: 8, size_mask: 0x7F, pagewrite_mask: 0x07 },
            EepromI2CType::C24C02 => Self { address_bits: 8, size_mask: 0xFF, pagewrite_mask: 0x07 },
            EepromI2CType::C24C04 => Self { address_bits: 8, size_mask: 0x1FF, pagewrite_mask: 0x0F },
            EepromI2CType::C24C08 => Self { address_bits: 8, size_mask: 0x3FF, pagewrite_mask: 0x0F },
            EepromI2CType::C24C16 => Self { address_bits: 8, size_mask: 0x7FF, pagewrite_mask: 0x0F },
            EepromI2CType::C24C32 => Self { address_bits: 16, size_mask: 0xFFF, pagewrite_mask: 0x1F },
            EepromI2CType::C24C64 => Self { address_bits: 16, size_mask: 0x1FFF, pagewrite_mask: 0x1F },
            EepromI2CType::C24C65 => Self { address_bits: 16, size_mask: 0x1FFF, pagewrite_mask: 0x3F },
            EepromI2CType::C24C128 => Self { address_bits: 16, size_mask: 0x3FFF, pagewrite_mask: 0x3F },
            EepromI2CType::C24C256 => Self { address_bits: 16, size_mask: 0x7FFF, pagewrite_mask: 0x3F },
            EepromI2CType::C24C512 => Self { address_bits: 16, size_mask: 0xFFFF, pagewrite_mask: 0x7F },
        }
    }
}

/// I2C EEPROM structure
#[derive(Debug, Clone)]
pub struct EepromI2C {
    sda: u8,                // Current SDA line state
    scl: u8,                // Current SCL line state
    old_sda: u8,            // Previous SDA line state
    old_scl: u8,            // Previous SCL line state
    cycles: u8,             // Operation internal cycle (0-9)
    rw: bool,               // Operation type (true: READ, false: WRITE)
    device_address: u16,    // Device address
    word_address: u16,      // Memory address
    buffer: u8,             // Write buffer
    state: EepromI2CState,  // Current operation state
    spec: EepromI2CSpec,    // EEPROM characteristics
    
    // I/O bit positions
    scl_in_bit: u8,         // SCL (write) bit position
    sda_in_bit: u8,         // SDA (write) bit position
    sda_out_bit: u8,        // SDA (read) bit position
}

impl EepromI2C {
    pub fn new(eeprom_type: EepromI2CType) -> Self {
        Self {
            sda: 1,
            scl: 1,
            old_sda: 1,
            old_scl: 1,
            cycles: 0,
            rw: false,
            device_address: 0,
            word_address: 0,
            buffer: 0,
            state: EepromI2CState::StandBy,
            spec: EepromI2CSpec::from_type(eeprom_type),
            scl_in_bit: 1,
            sda_in_bit: 0,
            sda_out_bit: 0,
        }
    }
    
    /// Initialize the EEPROM
    pub fn init(&mut self, sram: &mut BackupRam) {
        info!("Initializing I2C EEPROM type: {:?}", self.spec_from_type());
        
        // Reset I2C EEPROM state
        self.sda = 1;
        self.scl = 1;
        self.old_sda = 1;
        self.old_scl = 1;
        self.cycles = 0;
        self.rw = false;
        self.device_address = 0;
        self.word_address = 0;
        self.buffer = 0;
        self.state = EepromI2CState::StandBy;
        
        // Enable backup RAM
        sram.custom = 1;
        sram.on = true;
    }
    
    /// Helper to get EEPROM type from spec
    fn spec_from_type(&self) -> EepromI2CType {
        match (self.spec.address_bits, self.spec.size_mask, self.spec.pagewrite_mask) {
            (7, 0x7F, 0x03) => EepromI2CType::X24C01,
            (8, 0xFF, 0x03) => EepromI2CType::X24C02,
            (8, 0x7F, 0x07) => EepromI2CType::C24C01,
            (8, 0xFF, 0x07) => EepromI2CType::C24C02,
            (8, 0x1FF, 0x0F) => EepromI2CType::C24C04,
            (8, 0x3FF, 0x0F) => EepromI2CType::C24C08,
            (8, 0x7FF, 0x0F) => EepromI2CType::C24C16,
            (16, 0xFFF, 0x1F) => EepromI2CType::C24C32,
            (16, 0x1FFF, 0x1F) => EepromI2CType::C24C64,
            (16, 0x1FFF, 0x3F) => EepromI2CType::C24C65,
            (16, 0x3FFF, 0x3F) => EepromI2CType::C24C128,
            (16, 0x7FFF, 0x3F) => EepromI2CType::C24C256,
            (16, 0xFFFF, 0x7F) => EepromI2CType::C24C512,
            _ => EepromI2CType::X24C01, // Default
        }
    }
    
    /// Update EEPROM state based on SCL/SDA changes
    fn update(&mut self, sram: &mut BackupRam) {
        // EEPROM current state
        match self.state {
            EepromI2CState::StandBy => {
                self.detect_start();
            }
            
            EepromI2CState::WaitStop => {
                self.detect_stop();
            }
            
            EepromI2CState::GetWordAdr7Bits => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Next sequence
                        self.cycles = 1;
                        self.state = if self.rw {
                            EepromI2CState::ReadData
                        } else {
                            EepromI2CState::WriteData
                        };
                        
                        // Clear write buffer
                        self.buffer = 0;
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles < 8 {
                        // Latch Word Address bits 6-0
                        self.word_address |= (self.sda as u16) << (7 - self.cycles);
                    } else if self.cycles == 8 {
                        // Latch R/W bit
                        self.rw = self.sda != 0;
                    }
                }
            }
            
            EepromI2CState::GetDeviceAdr => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Shift Device Address bits
                        self.device_address <<= self.spec.address_bits as u16;
                        
                        // Next sequence
                        self.cycles = 1;
                        if self.rw {
                            self.state = EepromI2CState::ReadData;
                        } else {
                            self.word_address = 0;
                            self.state = if self.spec.address_bits == 16 {
                                EepromI2CState::GetWordAdrHigh
                            } else {
                                EepromI2CState::GetWordAdrLow
                            };
                        }
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles > 4 && self.cycles < 8 {
                        // Latch Device Address bits
                        self.device_address |= (self.sda as u16) << (7 - self.cycles);
                    } else if self.cycles == 8 {
                        // Latch R/W bit
                        self.rw = self.sda != 0;
                    }
                }
            }
            
            EepromI2CState::GetWordAdrHigh => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Next sequence
                        self.cycles = 1;
                        self.state = EepromI2CState::GetWordAdrLow;
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles < 9 {
                        if self.spec.size_mask < (1 << (16 - self.cycles)) {
                            // Ignored bit: Device Address bits should be right-shifted
                            self.device_address >>= 1;
                        } else {
                            // Latch Word Address high bits
                            self.word_address |= (self.sda as u16) << (16 - self.cycles);
                        }
                    }
                }
            }
            
            EepromI2CState::GetWordAdrLow => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Next sequence
                        self.cycles = 1;
                        self.state = EepromI2CState::WriteData;
                        
                        // Clear write buffer
                        self.buffer = 0;
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles < 9 {
                        if self.spec.size_mask < (1 << (8 - self.cycles)) {
                            // Ignored bit: Device Address bits should be right-shifted
                            self.device_address >>= 1;
                        } else {
                            // Latch Word Address low bits
                            self.word_address |= (self.sda as u16) << (8 - self.cycles);
                        }
                    }
                }
            }
            
            EepromI2CState::ReadData => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Next read sequence
                        self.cycles = 1;
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles == 9 {
                        // Check if ACK is received
                        if self.sda != 0 {
                            // End of read sequence
                            self.state = EepromI2CState::WaitStop;
                        } else {
                            // Increment Word Address (roll up at maximum array size)
                            self.word_address = (self.word_address + 1) & self.spec.size_mask;
                        }
                    }
                }
            }
            
            EepromI2CState::WriteData => {
                self.detect_start();
                self.detect_stop();
                
                // Look for SCL HIGH to LOW transition
                if self.old_scl != 0 && self.scl == 0 {
                    if self.cycles < 9 {
                        self.cycles += 1;
                    } else {
                        // Next write sequence
                        self.cycles = 1;
                    }
                }
                
                // Look for SCL LOW to HIGH transition
                else if self.old_scl == 0 && self.scl != 0 {
                    if self.cycles < 9 {
                        // Latch DATA bits 7-0 to write buffer
                        self.buffer |= self.sda << (8 - self.cycles);
                    } else {
                        // Write back to memory array (max 64kB)
                        let address = (self.device_address | self.word_address) & 0xFFFF;
                        if (address as usize) < sram.sram.len() {
                            sram.sram[address as usize] = self.buffer;
                        }
                        
                        // Clear write buffer
                        self.buffer = 0;
                        
                        // Increment Word Address (roll over at maximum page size)
                        self.word_address = (self.word_address & !self.spec.pagewrite_mask) |
                                          ((self.word_address + 1) & self.spec.pagewrite_mask);
                    }
                }
            }
        }
        
        // Save SCL & SDA previous state
        self.old_scl = self.scl;
        self.old_sda = self.sda;
    }
    
    /// Detect START condition (SDA HIGH to LOW while SCL is HIGH)
    fn detect_start(&mut self) {
        if self.old_scl != 0 && self.scl != 0 {
            if self.old_sda != 0 && self.sda == 0 {
                // Initialize cycle counter
                self.cycles = 0;
                
                // Initialize sequence
                if self.spec.address_bits == 7 {
                    // Get Word Address
                    self.word_address = 0;
                    self.state = EepromI2CState::GetWordAdr7Bits;
                } else {
                    // Get Device Address
                    self.device_address = 0;
                    self.state = EepromI2CState::GetDeviceAdr;
                }
            }
        }
    }
    
    /// Detect STOP condition (SDA LOW to HIGH while SCL is HIGH)
    fn detect_stop(&mut self) {
        if self.old_scl != 0 && self.scl != 0 {
            if self.old_sda == 0 && self.sda != 0 {
                self.state = EepromI2CState::StandBy;
            }
        }
    }
    
    /// Get output data from EEPROM
    fn out(&self, sram: &BackupRam) -> u8 {
        // Check EEPROM state
        if self.state == EepromI2CState::ReadData {
            // READ cycle
            if self.cycles < 9 {
                // Return memory array (max 64kB) DATA bits
                let address = (self.device_address | self.word_address) & 0xFFFF;
                if (address as usize) < sram.sram.len() {
                    return (sram.sram[address as usize] >> (8 - self.cycles)) & 1;
                }
            }
        } else if self.cycles == 9 {
            // ACK cycle
            return 0;
        }
        
        // Return latched /SDA input by default
        self.sda
    }
    
    /// Process write to EEPROM control lines
    pub fn write(&mut self, sda: u8, scl: u8, sram: &mut BackupRam) {
        self.sda = sda;
        self.scl = scl;
        self.update(sram);
    }
    
    /// Read from EEPROM
    pub fn read(&self, sram: &BackupRam) -> u8 {
        self.out(sram)
    }
    
    /// Save state
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(32);
        
        state.push(self.sda);
        state.push(self.scl);
        state.push(self.old_sda);
        state.push(self.old_scl);
        state.push(self.cycles);
        state.push(self.rw as u8);
        state.extend_from_slice(&self.device_address.to_le_bytes());
        state.extend_from_slice(&self.word_address.to_le_bytes());
        state.push(self.buffer);
        state.push(self.state as u8);
        state.push(self.spec.address_bits);
        state.extend_from_slice(&self.spec.size_mask.to_le_bytes());
        state.extend_from_slice(&self.spec.pagewrite_mask.to_le_bytes());
        state.push(self.scl_in_bit);
        state.push(self.sda_in_bit);
        state.push(self.sda_out_bit);
        
        state
    }
    
    /// Load state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 32 {
            return false;
        }
        
        self.sda = data[0];
        self.scl = data[1];
        self.old_sda = data[2];
        self.old_scl = data[3];
        self.cycles = data[4];
        self.rw = data[5] != 0;
        self.device_address = u16::from_le_bytes([data[6], data[7]]);
        self.word_address = u16::from_le_bytes([data[8], data[9]]);
        self.buffer = data[10];
        self.state = match data[11] {
            0 => EepromI2CState::StandBy,
            1 => EepromI2CState::WaitStop,
            2 => EepromI2CState::GetDeviceAdr,
            3 => EepromI2CState::GetWordAdr7Bits,
            4 => EepromI2CState::GetWordAdrHigh,
            5 => EepromI2CState::GetWordAdrLow,
            6 => EepromI2CState::WriteData,
            7 => EepromI2CState::ReadData,
            _ => return false,
        };
        self.spec.address_bits = data[12];
        self.spec.size_mask = u16::from_le_bytes([data[13], data[14]]);
        self.spec.pagewrite_mask = u16::from_le_bytes([data[15], data[16]]);
        self.scl_in_bit = data[17];
        self.sda_in_bit = data[18];
        self.sda_out_bit = data[19];
        
        true
    }
    
    /// Get SCL in bit position
    pub fn scl_in_bit(&self) -> u8 {
        self.scl_in_bit
    }
    
    /// Get SDA in bit position
    pub fn sda_in_bit(&self) -> u8 {
        self.sda_in_bit
    }
    
    /// Get SDA out bit position
    pub fn sda_out_bit(&self) -> u8 {
        self.sda_out_bit
    }
}

impl super::Eeprom for EepromI2C {
    fn init(&mut self, sram: &mut BackupRam) {
        self.init(sram);
    }
    
    fn write(&mut self, data: u8, sram: &mut BackupRam) {
        let sda = (data >> self.sda_in_bit) & 1;
        let scl = (data >> self.scl_in_bit) & 1;
        self.write(sda, scl, sram);
    }
    
    fn read(&self, _address: u32) -> u32 {
        // Note: This needs to be implemented with actual SRAM access
        // For now, return the output bit
        self.sda_out_bit as u32
    }
    
    fn reset(&mut self) {
        let eeprom_type = self.spec_from_type();
        *self = Self::new(eeprom_type);
    }
    
    fn eeprom_type(&self) -> super::EepromType {
        super::EepromType::I2C(self.spec_from_type())
    }
    
    fn save_state(&self) -> Vec<u8> {
        self.save_state()
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        self.load_state(data)
    }
}

impl Default for EepromI2C {
    fn default() -> Self {
        Self::new(EepromI2CType::X24C01)
    }
}