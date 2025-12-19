// genesis-plus-rs/src/core/cartridge/eeprom/eeprom_spi.rs

use crate::core::cartridge::sram::BackupRam;
use log::info;

/// SPI EEPROM state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EepromSPIState {
    Standby,
    GetOpcode,
    GetAddress,
    WriteByte,
    ReadByte,
}

/// SPI EEPROM structure
#[derive(Debug, Clone)]
pub struct EepromSPI {
    pub cs: u8,           // !CS line state
    pub clk: u8,          // SCLK line state
    pub out: u8,          // SO line state
    pub status: u8,       // Status register
    pub opcode: u8,       // 8-bit opcode
    pub buffer: u8,       // 8-bit data buffer
    pub addr: u16,        // 16-bit address
    pub cycles: u32,      // Current operation cycle
    pub state: EepromSPIState, // Current operation state
    
    // Hard-coded board implementation (!WP pin not used)
    bit_data: u8,
    bit_clk: u8,
    bit_hold: u8,
    bit_cs: u8,
    
    // Constants
    size_mask: u16,
    page_mask: u8,
}

impl EepromSPI {
    pub fn new() -> Self {
        // Max supported size 64KB (25x512/95x512)
        Self {
            cs: 0,
            clk: 0,
            out: 1,  // Default output is high
            status: 0,
            opcode: 0,
            buffer: 0,
            addr: 0,
            cycles: 0,
            state: EepromSPIState::GetOpcode,
            
            // Hard-coded board implementation bits
            bit_data: 0,
            bit_clk: 1,
            bit_hold: 2,
            bit_cs: 3,
            
            // Constants
            size_mask: 0xFFFF,
            page_mask: 0x7F,
        }
    }
    
    /// Initialize the EEPROM
    pub fn init(&mut self, sram: &mut BackupRam) {
        info!("Initializing SPI EEPROM");
        
        // Reset EEPROM state
        self.cs = 0;
        self.clk = 0;
        self.out = 1;
        self.status = 0;
        self.opcode = 0;
        self.buffer = 0;
        self.addr = 0;
        self.cycles = 0;
        self.state = EepromSPIState::GetOpcode;
        
        // Enable backup RAM
        sram.custom = 2;
        sram.on = true;
    }
    
    /// Process write to EEPROM control lines
    pub fn write(&mut self, data: u8, sram: &mut BackupRam) {
        // Make sure !HOLD is high
        if data & (1 << self.bit_hold) != 0 {
            // Check !CS state
            if data & (1 << self.bit_cs) != 0 {
                // !CS high -> end of current operation
                self.cycles = 0;
                self.out = 1;
                self.opcode = 0;
                self.state = EepromSPIState::GetOpcode;
            } else {
                // !CS low -> process current operation
                match self.state {
                    EepromSPIState::GetOpcode => {
                        // Latch data on CLK positive edge
                        if (data & (1 << self.bit_clk) != 0) && self.clk == 0 {
                            // 8-bit opcode buffer
                            self.opcode |= ((data >> self.bit_data) & 1);
                            self.cycles += 1;
                            
                            // Last bit?
                            if self.cycles == 8 {
                                // Reset cycles count
                                self.cycles = 0;
                                
                                // Decode instruction
                                match self.opcode {
                                    0x01 => {
                                        // WRITE STATUS
                                        self.buffer = 0;
                                        self.state = EepromSPIState::WriteByte;
                                    }
                                    
                                    0x02 => {
                                        // WRITE BYTE
                                        self.addr = 0;
                                        self.state = EepromSPIState::GetAddress;
                                    }
                                    
                                    0x03 => {
                                        // READ BYTE
                                        self.addr = 0;
                                        self.state = EepromSPIState::GetAddress;
                                    }
                                    
                                    0x04 => {
                                        // WRITE DISABLE
                                        self.status &= !0x02;
                                        self.state = EepromSPIState::Standby;
                                    }
                                    
                                    0x05 => {
                                        // READ STATUS
                                        self.buffer = self.status;
                                        self.state = EepromSPIState::ReadByte;
                                    }
                                    
                                    0x06 => {
                                        // WRITE ENABLE
                                        self.status |= 0x02;
                                        self.state = EepromSPIState::Standby;
                                    }
                                    
                                    _ => {
                                        // Specific instructions (not supported)
                                        self.state = EepromSPIState::Standby;
                                    }
                                }
                            } else {
                                // Shift opcode value
                                self.opcode = self.opcode << 1;
                            }
                        }
                    }
                    
                    EepromSPIState::GetAddress => {
                        // Latch data on CLK positive edge
                        if (data & (1 << self.bit_clk) != 0) && self.clk == 0 {
                            // 16-bit address
                            self.addr |= ((data >> self.bit_data) as u16 & 1);
                            self.cycles += 1;
                            
                            // Last bit?
                            if self.cycles == 16 {
                                // Reset cycles count
                                self.cycles = 0;
                                
                                // Mask unused address bits
                                self.addr &= self.size_mask;
                                
                                // Operation type
                                if self.opcode & 0x01 != 0 {
                                    // READ operation
                                    let addr_usize = self.addr as usize;
                                    if addr_usize < sram.sram.len() {
                                        self.buffer = sram.sram[addr_usize];
                                    }
                                    self.state = EepromSPIState::ReadByte;
                                } else {
                                    // WRITE operation
                                    self.buffer = 0;
                                    self.state = EepromSPIState::WriteByte;
                                }
                            } else {
                                // Shift address value
                                self.addr = self.addr << 1;
                            }
                        }
                    }
                    
                    EepromSPIState::WriteByte => {
                        // Latch data on CLK positive edge
                        if (data & (1 << self.bit_clk) != 0) && self.clk == 0 {
                            // 8-bit data buffer
                            self.buffer |= ((data >> self.bit_data) & 1);
                            self.cycles += 1;
                            
                            // Last bit?
                            if self.cycles == 8 {
                                // Reset cycles count
                                self.cycles = 0;
                                
                                // Write data to destination
                                if self.opcode & 0x01 != 0 {
                                    // Update status register
                                    self.status = (self.status & 0x02) | (self.buffer & 0x0C);
                                    
                                    // Wait for operation end
                                    self.state = EepromSPIState::Standby;
                                } else {
                                    // Memory Array (write-protected)
                                    if self.status & 2 != 0 {
                                        // Check array protection bits (BP0, BP1)
                                        match (self.status >> 2) & 0x03 {
                                            0x01 => {
                                                // $C000-$FFFF (sector #3) is protected
                                                if self.addr < 0xC000 {
                                                    let addr_usize = self.addr as usize;
                                                    if addr_usize < sram.sram.len() {
                                                        sram.sram[addr_usize] = self.buffer;
                                                    }
                                                }
                                            }
                                            
                                            0x02 => {
                                                // $8000-$FFFF (sectors #2 and #3) is protected
                                                if self.addr < 0x8000 {
                                                    let addr_usize = self.addr as usize;
                                                    if addr_usize < sram.sram.len() {
                                                        sram.sram[addr_usize] = self.buffer;
                                                    }
                                                }
                                            }
                                            
                                            0x03 => {
                                                // $0000-$FFFF (all sectors) is protected
                                                // Do nothing
                                            }
                                            
                                            _ => {
                                                // No sectors protected
                                                let addr_usize = self.addr as usize;
                                                if addr_usize < sram.sram.len() {
                                                    sram.sram[addr_usize] = self.buffer;
                                                }
                                            }
                                        }
                                    }
                                    
                                    // Reset data buffer
                                    self.buffer = 0;
                                    
                                    // Increase array address (sequential writes limited within same page)
                                    self.addr = (self.addr & !(self.page_mask as u16)) | 
                                               ((self.addr + 1) & (self.page_mask as u16));
                                }
                            } else {
                                // Shift data buffer value
                                self.buffer = self.buffer << 1;
                            }
                        }
                    }
                    
                    EepromSPIState::ReadByte => {
                        // Output data on CLK positive edge
                        if (data & (1 << self.bit_clk) != 0) && self.clk == 0 {
                            // Read out bits
                            self.out = (self.buffer >> (7 - self.cycles as u8)) & 1;
                            self.cycles += 1;
                            
                            // Last bit?
                            if self.cycles == 8 {
                                // Reset cycles count
                                self.cycles = 0;
                                
                                // Read from memory array?
                                if self.opcode == 0x03 {
                                    // Read next array byte
                                    self.addr = (self.addr + 1) & self.size_mask;
                                    let addr_usize = self.addr as usize;
                                    if addr_usize < sram.sram.len() {
                                        self.buffer = sram.sram[addr_usize];
                                    }
                                }
                            }
                        }
                    }
                    
                    _ => {
                        // Wait for !CS low->high transition
                    }
                }
            }
        }
        
        // Update input lines
        self.cs = (data >> self.bit_cs) & 1;
        self.clk = (data >> self.bit_clk) & 1;
    }
    
    /// Read from EEPROM
    pub fn read(&self, _address: u32) -> u32 {
        (self.out << self.bit_data) as u32
    }
    
    /// Save state
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(32);
        
        state.push(self.cs);
        state.push(self.clk);
        state.push(self.out);
        state.push(self.status);
        state.push(self.opcode);
        state.push(self.buffer);
        state.extend_from_slice(&self.addr.to_le_bytes());
        state.extend_from_slice(&self.cycles.to_le_bytes());
        state.push(self.state as u8);
        
        state
    }
    
    /// Load state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 32 {
            return false;
        }
        
        self.cs = data[0];
        self.clk = data[1];
        self.out = data[2];
        self.status = data[3];
        self.opcode = data[4];
        self.buffer = data[5];
        self.addr = u16::from_le_bytes([data[6], data[7]]);
        self.cycles = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        self.state = match data[12] {
            0 => EepromSPIState::Standby,
            1 => EepromSPIState::GetOpcode,
            2 => EepromSPIState::GetAddress,
            3 => EepromSPIState::WriteByte,
            4 => EepromSPIState::ReadByte,
            _ => return false,
        };
        
        true
    }
}

impl super::Eeprom for EepromSPI {
    fn init(&mut self, sram: &mut BackupRam) {
        self.init(sram);
    }
    
    fn write(&mut self, data: u8, sram: &mut BackupRam) {
        self.write(data, sram);
    }
    
    fn read(&self, address: u32) -> u32 {
        self.read(address)
    }
    
    fn reset(&mut self) {
        *self = Self::new();
    }
    
    fn eeprom_type(&self) -> super::EepromType {
        super::EepromType::Spi
    }
    
    fn save_state(&self) -> Vec<u8> {
        self.save_state()
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        self.load_state(data)
    }
}

impl Default for EepromSPI {
    fn default() -> Self {
        Self::new()
    }
}