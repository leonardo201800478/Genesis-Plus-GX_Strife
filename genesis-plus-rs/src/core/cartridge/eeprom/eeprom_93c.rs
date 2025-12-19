// genesis-plus-rs/src/core/cartridge/eeprom/eeprom_93c.rs

use crate::core::cartridge::sram::BackupRam;
use log::info;

/// EEPROM 93C46 state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eeprom93CState {
    WaitStandby,
    WaitStart,
    GetOpcode,
    WriteWord,
    ReadWord,
}

/// EEPROM 93C46 structure
#[derive(Debug, Clone)]
pub struct Eeprom93C {
    pub enabled: bool,      // Chip enabled
    pub cs: u8,            // CHIP SELECT line state
    pub clk: u8,           // CLK line state
    pub data: u8,          // DATA OUT line state
    pub cycles: u8,        // Current operation cycle
    pub we: bool,          // Write enabled
    pub opcode: u8,        // 8-bit opcode + address
    pub buffer: u16,       // 16-bit data buffer
    pub state: Eeprom93CState, // Current operation state
    
    // Fixed board implementation
    bit_data: u8,
    bit_clk: u8,
    bit_cs: u8,
}

impl Eeprom93C {
    pub fn new() -> Self {
        Self {
            enabled: false,
            cs: 0,
            clk: 0,
            data: 1,  // Default DATA OUT is high
            cycles: 0,
            we: false,
            opcode: 0,
            buffer: 0,
            state: Eeprom93CState::WaitStart,
            
            // Fixed board implementation bits
            bit_data: 0,
            bit_clk: 1,
            bit_cs: 2,
        }
    }
    
    /// Initialize the EEPROM
    pub fn init(&mut self, sram: &mut BackupRam) {
        info!("Initializing Microwire 93C46 EEPROM");
        
        // Reset EEPROM state
        self.enabled = true;
        self.cs = 0;
        self.clk = 0;
        self.data = 1;
        self.cycles = 0;
        self.we = false;
        self.opcode = 0;
        self.buffer = 0;
        self.state = Eeprom93CState::WaitStart;
        
        // Enable backup RAM with custom type 3
        sram.custom = 3;
        sram.on = true;
    }
    
    /// Process write to EEPROM control lines
    pub fn write(&mut self, data: u8, sram: &mut BackupRam) {
        // Make sure CS is HIGH
        if data & (1 << self.bit_cs) != 0 {
            // Data latched on CLK positive edge
            if (data & (1 << self.bit_clk) != 0) && self.clk == 0 {
                // Current EEPROM state
                match self.state {
                    Eeprom93CState::WaitStart => {
                        // Wait for START bit
                        if data & (1 << self.bit_data) != 0 {
                            self.opcode = 0;
                            self.cycles = 0;
                            self.state = Eeprom93CState::GetOpcode;
                        }
                    }
                    
                    Eeprom93CState::GetOpcode => {
                        // 8-bit buffer (opcode + address)
                        self.opcode |= ((data >> self.bit_data) & 1) << (7 - self.cycles);
                        self.cycles += 1;
                        
                        if self.cycles == 8 {
                            // Decode instruction
                            match (self.opcode >> 6) & 3 {
                                1 => {
                                    // WRITE
                                    self.buffer = 0;
                                    self.cycles = 0;
                                    self.state = Eeprom93CState::WriteWord;
                                }
                                
                                2 => {
                                    // READ
                                    let addr = (self.opcode & 0x3F) << 1;
                                    if addr < sram.sram.len() {
                                        self.buffer = u16::from_le_bytes([
                                            sram.sram[addr],
                                            sram.sram[addr + 1],
                                        ]);
                                    }
                                    self.cycles = 0;
                                    self.state = Eeprom93CState::ReadWord;
                                    
                                    // Force DATA OUT
                                    self.data = 0;
                                }
                                
                                3 => {
                                    // ERASE
                                    if self.we {
                                        let addr = (self.opcode & 0x3F) << 1;
                                        if addr < sram.sram.len() {
                                            sram.sram[addr] = 0xFF;
                                            sram.sram[addr + 1] = 0xFF;
                                        }
                                    }
                                    
                                    // Wait for next command
                                    self.state = Eeprom93CState::WaitStandby;
                                }
                                
                                _ => {
                                    // Special command
                                    match (self.opcode >> 4) & 3 {
                                        1 => {
                                            // WRITE ALL
                                            self.buffer = 0;
                                            self.cycles = 0;
                                            self.state = Eeprom93CState::WriteWord;
                                        }
                                        
                                        2 => {
                                            // ERASE ALL
                                            if self.we {
                                                for byte in sram.sram.iter_mut() {
                                                    *byte = 0xFF;
                                                }
                                            }
                                            
                                            // Wait for next command
                                            self.state = Eeprom93CState::WaitStandby;
                                        }
                                        
                                        _ => {
                                            // WRITE ENABLE/DISABLE
                                            self.we = ((self.opcode >> 4) & 1) != 0;
                                            
                                            // Wait for next command
                                            self.state = Eeprom93CState::WaitStandby;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    Eeprom93CState::WriteWord => {
                        // 16-bit data buffer
                        self.buffer |= ((data >> self.bit_data) as u16 & 1) << (15 - self.cycles);
                        self.cycles += 1;
                        
                        if self.cycles == 16 {
                            // Check EEPROM write protection
                            if self.we {
                                if self.opcode & 0x40 != 0 {
                                    // Write one word
                                    let addr = (self.opcode & 0x3F) << 1;
                                    if addr < sram.sram.len() {
                                        sram.sram[addr] = (self.buffer & 0xFF) as u8;
                                        sram.sram[addr + 1] = (self.buffer >> 8) as u8;
                                    }
                                } else {
                                    // Write 64 words
                                    for i in 0..64 {
                                        let addr = i << 1;
                                        if addr < sram.sram.len() {
                                            sram.sram[addr] = (self.buffer & 0xFF) as u8;
                                            sram.sram[addr + 1] = (self.buffer >> 8) as u8;
                                        }
                                    }
                                }
                            }
                            
                            // Wait for next command
                            self.state = Eeprom93CState::WaitStandby;
                        }
                    }
                    
                    Eeprom93CState::ReadWord => {
                        // Set DATA OUT
                        self.data = ((self.buffer >> (15 - self.cycles)) & 1) as u8;
                        self.cycles += 1;
                        
                        if self.cycles == 16 {
                            // Read next word (93C46B)
                            self.opcode = self.opcode.wrapping_add(1);
                            self.cycles = 0;
                            let addr = (self.opcode & 0x3F) << 1;
                            if addr < sram.sram.len() {
                                self.buffer = u16::from_le_bytes([
                                    sram.sram[addr],
                                    sram.sram[addr + 1],
                                ]);
                            }
                        }
                    }
                    
                    _ => {
                        // Wait for STANDBY mode
                    }
                }
            }
        } else {
            // CS HIGH->LOW transition
            if self.cs != 0 {
                // Standby mode
                self.data = 1;
                self.state = Eeprom93CState::WaitStart;
            }
        }
        
        // Update input lines
        self.cs = (data >> self.bit_cs) & 1;
        self.clk = (data >> self.bit_clk) & 1;
    }
    
    /// Read from EEPROM
    pub fn read(&self) -> u8 {
        ((self.cs << self.bit_cs) | (self.data << self.bit_data) | (1 << self.bit_clk))
    }
    
    /// Save state
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::with_capacity(16);
        
        state.push(self.enabled as u8);
        state.push(self.cs);
        state.push(self.clk);
        state.push(self.data);
        state.push(self.cycles);
        state.push(self.we as u8);
        state.push(self.opcode);
        state.extend_from_slice(&self.buffer.to_le_bytes());
        state.push(self.state as u8);
        
        state
    }
    
    /// Load state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 16 {
            return false;
        }
        
        self.enabled = data[0] != 0;
        self.cs = data[1];
        self.clk = data[2];
        self.data = data[3];
        self.cycles = data[4];
        self.we = data[5] != 0;
        self.opcode = data[6];
        self.buffer = u16::from_le_bytes([data[7], data[8]]);
        self.state = match data[9] {
            0 => Eeprom93CState::WaitStandby,
            1 => Eeprom93CState::WaitStart,
            2 => Eeprom93CState::GetOpcode,
            3 => Eeprom93CState::WriteWord,
            4 => Eeprom93CState::ReadWord,
            _ => return false,
        };
        
        true
    }
}

impl super::Eeprom for Eeprom93C {
    fn init(&mut self, sram: &mut BackupRam) {
        self.init(sram);
    }
    
    fn write(&mut self, data: u8, sram: &mut BackupRam) {
        self.write(data, sram);
    }
    
    fn read(&self, _address: u32) -> u32 {
        self.read() as u32
    }
    
    fn reset(&mut self) {
        *self = Self::new();
    }
    
    fn eeprom_type(&self) -> super::EepromType {
        super::EepromType::Microwire93C46
    }
    
    fn save_state(&self) -> Vec<u8> {
        self.save_state()
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        self.load_state(data)
    }
}

impl Default for Eeprom93C {
    fn default() -> Self {
        Self::new()
    }
}