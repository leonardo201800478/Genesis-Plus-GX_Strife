//! EEPROM 93C46

use super::*;

/// EEPROM 93C46
pub struct Eeprom93c46 {
    memory: [u16; 64],  // 128 bytes organizados como 64 palavras de 16 bits
    enabled: bool,
    cs: bool,
    sk: bool,
    di: bool,
    do_out: bool,
    opcode: u8,
    address: u8,
    data: u16,
    bits: u8,
    state: EepromState,
}

#[derive(Clone, Copy, PartialEq)]
enum EepromState {
    Idle,
    Start,
    Opcode,
    Address,
    ReadData,
    WriteData,
    Erase,
    WriteAll,
    EraseAll,
}

impl Eeprom93c46 {
    pub fn new() -> Self {
        Self {
            memory: [0xFFFF; 64],
            enabled: false,
            cs: false,
            sk: false,
            di: false,
            do_out: false,
            opcode: 0,
            address: 0,
            data: 0,
            bits: 0,
            state: EepromState::Idle,
        }
    }
    
    pub fn write_control(&mut self, value: u8) {
        self.enabled = (value & 0x08) != 0;
        
        if (value & 0x80) != 0 {
            self.reset();
        }
    }
    
    pub fn write_data(&mut self, value: u8) {
        if !self.enabled {
            return;
        }
        
        let new_cs = (value & 0x01) != 0;
        let new_sk = (value & 0x02) != 0;
        let new_di = (value & 0x04) != 0;
        
        // Detect rising edge on SK
        if !self.sk && new_sk {
            self.clock(new_cs, new_di);
        }
        
        self.cs = new_cs;
        self.sk = new_sk;
        self.di = new_di;
    }
    
    pub fn read_data(&self) -> u8 {
        if !self.enabled {
            return 0xFF;
        }
        
        (self.do_out as u8) << 7
    }
    
    fn clock(&mut self, cs: bool, di: bool) {
        if !cs {
            self.state = EepromState::Idle;
            self.bits = 0;
            return;
        }
        
        match self.state {
            EepromState::Idle => {
                if di {
                    self.state = EepromState::Start;
                    self.bits = 1;
                }
            }
            EepromState::Start => {
                self.opcode = (self.opcode << 1) | (di as u8);
                self.bits += 1;
                
                if self.bits == 3 {
                    self.state = EepromState::Opcode;
                    self.bits = 0;
                }
            }
            EepromState::Opcode => {
                self.opcode = (self.opcode << 1) | (di as u8);
                self.bits += 1;
                
                if self.bits == 3 {
                    match self.opcode & 0x07 {
                        0x02 => { // WRITE
                            self.state = EepromState::Address;
                            self.bits = 0;
                        }
                        0x03 => { // READ
                            self.state = EepromState::Address;
                            self.bits = 0;
                        }
                        0x01 => { // ERASE
                            self.state = EepromState::Address;
                            self.bits = 0;
                        }
                        0x00 => { // EWDS (Erase/Write Disable)
                            self.enabled = false;
                            self.state = EepromState::Idle;
                        }
                        0x04 => { // ERAL (Erase All)
                            self.state = EepromState::EraseAll;
                            self.bits = 0;
                        }
                        0x05 => { // WRAL (Write All)
                            self.state = EepromState::WriteAll;
                            self.bits = 0;
                        }
                        0x06 => { // EWEN (Erase/Write Enable)
                            self.enabled = true;
                            self.state = EepromState::Idle;
                        }
                        _ => {
                            self.state = EepromState::Idle;
                        }
                    }
                }
            }
            EepromState::Address => {
                self.address = (self.address << 1) | (di as u8);
                self.bits += 1;
                
                if self.bits == 7 {
                    match (self.opcode >> 1) & 0x03 {
                        0x01 => { // READ
                            self.data = self.memory[self.address as usize];
                            self.do_out = false;
                            self.state = EepromState::ReadData;
                            self.bits = 0;
                        }
                        0x00 => { // WRITE
                            self.state = EepromState::WriteData;
                            self.bits = 0;
                            self.data = 0;
                        }
                        0x02 => { // ERASE
                            self.memory[self.address as usize] = 0xFFFF;
                            self.do_out = false;
                            self.state = EepromState::Idle;
                        }
                        _ => {
                            self.state = EepromState::Idle;
                        }
                    }
                }
            }
            EepromState::ReadData => {
                self.do_out = (self.data & 0x8000) != 0;
                self.data <<= 1;
                self.bits += 1;
                
                if self.bits == 16 {
                    self.state = EepromState::Idle;
                    self.do_out = true;
                }
            }
            EepromState::WriteData => {
                self.data = (self.data << 1) | (di as u16);
                self.bits += 1;
                
                if self.bits == 16 {
                    self.memory[self.address as usize] = self.data;
                    self.do_out = false;
                    self.state = EepromState::Idle;
                }
            }
            EepromState::EraseAll => {
                if self.bits == 0 {
                    // Primeiro bit deve ser 1
                    if di {
                        self.bits = 1;
                    } else {
                        self.state = EepromState::Idle;
                    }
                } else if self.bits < 65 {
                    // Aguarda 64 bits de dummy
                    self.bits += 1;
                } else {
                    // Apaga toda a memória
                    for i in 0..64 {
                        self.memory[i] = 0xFFFF;
                    }
                    self.state = EepromState::Idle;
                }
            }
            EepromState::WriteAll => {
                if self.bits < 16 {
                    self.data = (self.data << 1) | (di as u16);
                    self.bits += 1;
                } else if self.bits < 80 {
                    // Aguarda 64 bits de dummy após os dados
                    self.bits += 1;
                } else {
                    // Escreve dados em toda a memória
                    for i in 0..64 {
                        self.memory[i] = self.data;
                    }
                    self.state = EepromState::Idle;
                }
            }
        }
    }
    
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}
