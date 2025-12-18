//! Mapeadores para Sega 32X

use super::{Mapper, MappedAddress, AccessType};
use crate::core::cartridge::types::{CartridgeType, CartridgeError, CartridgeResult};

/// Componentes específicos do 32X
#[derive(Debug, Clone, Copy)]
pub enum ComponentType {
    Rom,
    Sram,
    Communication,  // 0xA15100-0xA1511F
    Framebuffer,    // 0xA15180-0xA151FF
    Dram,           // 0x200000-0x20FFFF
    BootRom,        // 0x400000-0x400FFF
}

/// Trait específico para mappers 32X
pub trait Sega32XMapper: Mapper {
    fn get_component_type(&self, addr: u32) -> ComponentType;
}

/// Mapper padrão do 32X
pub struct Standard32XMapper {
    rom_mask: u32,
    rom_banks: [u32; 8],
    is_32x_mode: bool,
}

impl Standard32XMapper {
    pub fn new(rom_size: usize) -> Self {
        let rom_mask = if rom_size.is_power_of_two() {
            rom_size as u32 - 1
        } else {
            (1 << (32 - rom_size.leading_zeros())) as u32 - 1
        };
        
        let mut banks = [0; 8];
        for i in 0..8 {
            banks[i] = (i as u32) * 0x20000; // 128KB por banco inicialmente
        }
        
        Self {
            rom_mask,
            rom_banks: banks,
            is_32x_mode: false,
        }
    }
}

impl Mapper for Standard32XMapper {
    fn map_address(&self, addr: u32, _access: AccessType) -> MappedAddress {
        let masked = addr & 0x00FFFFFF;
        
        match masked {
            // ROM normal do Mega Drive (0x000000-0x3FFFFF)
            0x000000..=0x3FFFFF => {
                if self.is_32x_mode && masked >= 0x200000 {
                    // No modo 32X, parte da ROM é remapeada
                    MappedAddress {
                        addr: masked & 0x1FFFFF, // Apenas 2MB no 32X
                        component: ComponentType::Rom,
                    }
                } else {
                    let bank = (masked >> 17) as usize & 0x07;
                    let offset = masked & 0x1FFFF;
                    MappedAddress {
                        addr: (self.rom_banks[bank] + offset) & self.rom_mask,
                        component: ComponentType::Rom,
                    }
                }
            }
            
            // DRAM do 32X (256KB)
            0x200000..=0x20FFFF => MappedAddress {
                addr: masked - 0x200000,
                component: ComponentType::Dram,
            },
            
            // Registradores de comunicação
            0xA15100..=0xA1511F => MappedAddress {
                addr: masked - 0xA15100,
                component: ComponentType::Communication,
            },
            
            // Framebuffer do 32X
            0xA15180..=0xA151FF => MappedAddress {
                addr: masked - 0xA15180,
                component: ComponentType::Framebuffer,
            },
            
            // Boot ROM do 32X
            0x400000..=0x400FFF => MappedAddress {
                addr: masked - 0x400000,
                component: ComponentType::BootRom,
            },
            
            // Padrão: trata como ROM
            _ => MappedAddress {
                addr: masked & self.rom_mask,
                component: ComponentType::Rom,
            },
        }
    }
    
    fn write_register(&mut self, reg: u8, value: u8) {
        // Implementação de escrita de registradores
        match reg {
            0xA1 => { // Provável registrador de controle
                self.is_32x_mode = (value & 0x80) != 0;
            }
            _ => {
                // Atualiza bancos de ROM
                if reg >= 0x00 && reg <= 0x07 {
                    self.rom_banks[reg as usize] = (value as u32) * 0x20000;
                }
            }
        }
    }
    
    fn reset(&mut self) {
        self.is_32x_mode = false;
        for i in 0..8 {
            self.rom_banks[i] = (i as u32) * 0x20000;
        }
    }
}

impl Sega32XMapper for Standard32XMapper {
    fn get_component_type(&self, addr: u32) -> ComponentType {
        let mapped = self.map_address(addr, AccessType::Read);
        mapped.component
    }
}

/// Cria mapper apropriado para 32X
pub fn create_mapper(
    cart_type: CartridgeType,
    rom_size: usize,
) -> CartridgeResult<Box<dyn Sega32XMapper>> {
    match cart_type {
        CartridgeType::Sega32XStandard |
        CartridgeType::Sega32XROM |
        CartridgeType::Sega32XRAM => {
            Ok(Box::new(Standard32XMapper::new(rom_size)))
        }
        _ => Err(CartridgeError::UnsupportedHardware(
            format!("Mapper 32X para {:?} não implementado", cart_type)
        )),
    }
}