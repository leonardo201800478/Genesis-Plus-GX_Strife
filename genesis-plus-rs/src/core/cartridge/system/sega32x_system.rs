//! Sistema Sega 32X (Super 32X)
//! Expansão para Mega Drive com dois processadores SH-2

use crate::core::cartridge::types::*;
use crate::core::cartridge::mapper::sega32x::Sega32XMapper;
use crate::core::cartridge::memory::{Rom, Sram};
use crate::core::cartridge::chips;
use log::{info, warn};

/// Estrutura principal do cartucho 32X
pub struct Sega32XCartridge {
    info: CartridgeInfo,
    mapper: Box<dyn Sega32XMapper>,
    rom: Box<dyn Rom>,
    sram: Option<Box<dyn Sram>>,
    // Chips específicos do 32X
    communication_regs: [u32; 8], // Registradores COMM0-COMM7
    framebuffer: Vec<u16>,        // Framebuffer 256x224
    is_32x_mode: bool,            // 32X ativado
    is_master_sh2_halted: bool,   // SH-2 Master em halt
    is_slave_sh2_halted: bool,    // SH-2 Slave em halt
}

impl Sega32XCartridge {
    pub fn create(rom_data: Vec<u8>, region_hint: Option<Region>) -> CartridgeResult<Self> {
        info!("Criando cartucho Sega 32X - Tamanho ROM: {} bytes", rom_data.len());
        
        // Detecta tipo baseado no cabeçalho
        let cart_type = detect_32x_type(&rom_data)?;
        let region = detect_region(&rom_data, region_hint);
        
        // Cria mapper específico para 32X
        let mapper = crate::core::cartridge::mapper::sega32x::create_mapper(cart_type, rom_data.len())?;
        
        // Cria ROM
        let rom = Box::new(crate::core::cartridge::memory::rom::StandardRom::new(rom_data));
        
        // 32X geralmente tem 256KB de backup RAM
        let sram_size = 256 * 1024;
        let sram = Some(Box::new(crate::core::cartridge::memory::sram::StandardSram::new(sram_size)) 
            as Box<dyn Sram>);
        
        let info = CartridgeInfo {
            system: System::Sega32X,
            cartridge_type: cart_type,
            region,
            rom_size: rom.size(),
            sram_size,
            has_eeprom: false,
            eeprom_type: None,
            has_flash: false,
            has_special_chip: false,
            special_chip: None,
        };
        
        Ok(Self {
            info,
            mapper,
            rom,
            sram,
            communication_regs: [0; 8],
            framebuffer: vec![0; 256 * 224], // 256x224 pixels
            is_32x_mode: false,
            is_master_sh2_halted: true,
            is_slave_sh2_halted: true,
        })
    }
    
    /// Processa acesso aos registradores de comunicação 68K<->SH2
    fn handle_communication(&mut self, addr: u32, value: u32, is_write: bool) {
        let reg_index = ((addr - 0xA15100) / 4) as usize;
        
        if reg_index < 8 {
            if is_write {
                self.communication_regs[reg_index] = value;
                
                // Processa escritas específicas
                match addr {
                    0xA15100 => { // COMM0 - Habilita 32X
                        self.is_32x_mode = (value & 0x01) != 0;
                        info!("32X mode: {}", self.is_32x_mode);
                    }
                    0xA15104 => { // COMM2 - Halt control
                        self.is_master_sh2_halted = (value & 0x01) != 0;
                        self.is_slave_sh2_halted = (value & 0x02) != 0;
                    }
                    _ => {}
                }
            }
        }
    }
    
    /// Lê do framebuffer do 32X
    fn read_framebuffer(&self, addr: u32) -> u16 {
        let index = ((addr - 0xA15180) / 2) as usize;
        if index < self.framebuffer.len() {
            self.framebuffer[index]
        } else {
            0
        }
    }
    
    /// Escreve no framebuffer do 32X
    fn write_framebuffer(&mut self, addr: u32, value: u16) {
        let index = ((addr - 0xA15180) / 2) as usize;
        if index < self.framebuffer.len() {
            self.framebuffer[index] = value;
        }
    }
}

impl Cartridge for Sega32XCartridge {
    fn read_byte(&self, addr: u32) -> u8 {
        let mapped = self.mapper.map_address(addr, AccessType::Read);
        
        match mapped.component {
            ComponentType::Rom => self.rom.read_byte(mapped.addr),
            ComponentType::Sram => self.sram.as_ref()
                .map_or(0xFF, |s| s.read_byte(mapped.addr)),
            ComponentType::Communication => {
                let reg_index = ((mapped.addr) / 4) as usize;
                if reg_index < 8 {
                    let shift = (mapped.addr % 4) * 8;
                    ((self.communication_regs[reg_index] >> shift) & 0xFF) as u8
                } else {
                    0xFF
                }
            }
            ComponentType::Framebuffer => {
                let word = self.read_framebuffer(mapped.addr & !1);
                if (mapped.addr & 1) == 0 {
                    (word & 0xFF) as u8
                } else {
                    (word >> 8) as u8
                }
            }
            _ => 0xFF,
        }
    }
    
    fn read_word(&self, addr: u32) -> u16 {
        if addr & 1 == 0 {
            let mapped = self.mapper.map_address(addr, AccessType::Read);
            
            match mapped.component {
                ComponentType::Rom => self.rom.read_word(mapped.addr),
                ComponentType::Sram => self.sram.as_ref()
                    .map_or(0xFFFF, |s| s.read_word(mapped.addr)),
                ComponentType::Communication => {
                    let reg_index = (mapped.addr / 4) as usize;
                    if reg_index < 8 {
                        (self.communication_regs[reg_index] & 0xFFFF) as u16
                    } else {
                        0xFFFF
                    }
                }
                ComponentType::Framebuffer => self.read_framebuffer(mapped.addr),
                _ => {
                    let low = self.read_byte(addr) as u16;
                    let high = self.read_byte(addr + 1) as u16;
                    (high << 8) | low
                }
            }
        } else {
            // Endereço ímpar
            let low = self.read_byte(addr) as u16;
            let high = self.read_byte(addr + 1) as u16;
            (high << 8) | low
        }
    }
    
    fn write_byte(&mut self, addr: u32, value: u8) {
        let mapped = self.mapper.map_address(addr, AccessType::Write);
        
        match mapped.component {
            ComponentType::Rom => {
                // Escrita em ROM geralmente muda bancos
                self.mapper.write_register(addr, value);
            }
            ComponentType::Sram => {
                if let Some(sram) = &mut self.sram {
                    sram.write_byte(mapped.addr, value);
                }
            }
            ComponentType::Communication => {
                let reg_index = (mapped.addr / 4) as usize;
                if reg_index < 8 {
                    let shift = (mapped.addr % 4) * 8;
                    let mask = 0xFFu32 << shift;
                    self.communication_regs[reg_index] = 
                        (self.communication_regs[reg_index] & !mask) | 
                        ((value as u32) << shift);
                    self.handle_communication(addr, value as u32, true);
                }
            }
            ComponentType::Framebuffer => {
                let word_addr = mapped.addr & !1;
                let mut word = self.read_framebuffer(word_addr);
                
                if (mapped.addr & 1) == 0 {
                    word = (word & 0xFF00) | (value as u16);
                } else {
                    word = (word & 0x00FF) | ((value as u16) << 8);
                }
                
                self.write_framebuffer(word_addr, word);
            }
            _ => {}
        }
    }
    
    fn write_word(&mut self, addr: u32, value: u16) {
        if addr & 1 == 0 {
            let mapped = self.mapper.map_address(addr, AccessType::Write);
            
            match mapped.component {
                ComponentType::Rom => {
                    self.mapper.write_register(addr, (value >> 8) as u8);
                    self.mapper.write_register(addr + 1, value as u8);
                }
                ComponentType::Sram => {
                    if let Some(sram) = &mut self.sram {
                        sram.write_word(mapped.addr, value);
                    }
                }
                ComponentType::Communication => {
                    let reg_index = (mapped.addr / 4) as usize;
                    if reg_index < 8 {
                        self.communication_regs[reg_index] = 
                            (self.communication_regs[reg_index] & 0xFFFF0000) | 
                            (value as u32);
                        self.handle_communication(addr, value as u32, true);
                    }
                }
                ComponentType::Framebuffer => {
                    self.write_framebuffer(mapped.addr, value);
                }
                _ => {
                    self.write_byte(addr, value as u8);
                    self.write_byte(addr + 1, (value >> 8) as u8);
                }
            }
        } else {
            // Endereço ímpar
            self.write_byte(addr, value as u8);
            self.write_byte(addr + 1, (value >> 8) as u8);
        }
    }
    
    // ... implementações restantes do trait Cartridge
    fn reset(&mut self) {
        self.mapper.reset();
        if let Some(sram) = &mut self.sram {
            sram.reset();
        }
        self.communication_regs = [0; 8];
        self.framebuffer.fill(0);
        self.is_32x_mode = false;
        self.is_master_sh2_halted = true;
        self.is_slave_sh2_halted = true;
    }
    
    fn info(&self) -> &CartridgeInfo {
        &self.info
    }
    
    fn tick(&mut self, cycles: u32) {
        // Atualização temporal se necessário
    }
    
    fn save_state(&self) -> Vec<u8> {
        Vec::new() // Placeholder
    }
    
    fn load_state(&mut self, data: &[u8]) -> CartridgeResult<()> {
        Ok(()) // Placeholder
    }
}

/// Detecta tipo de cartucho 32X
fn detect_32x_type(rom_data: &[u8]) -> CartridgeResult<CartridgeType> {
    if rom_data.len() < 0x100 {
        return Err(CartridgeError::InvalidRom);
    }
    
    // Verifica assinatura "32X"
    if rom_data.len() > 0x100 && &rom_data[0x100..0x104] == b"32X\0" {
        Ok(CartridgeType::Sega32XStandard)
    } else {
        // Heurísticas para detecção
        Ok(CartridgeType::Sega32XStandard)
    }
}