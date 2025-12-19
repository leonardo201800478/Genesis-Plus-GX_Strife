//! Mapeadores de cartucho para Master System / Game Gear
//! 
//! Este módulo implementa todos os mappers suportados pelo Genesis Plus GX
//! para sistemas Master System, Game Gear e SG-1000.

mod common;
mod sega;
mod codemasters;
mod korean;
mod msx;
mod multi;
mod zemina;
mod eeprom;
mod terebi;
mod ram;

use crate::core::cartridge::Cartridge;
use crate::core::system::{System, Region};
use crate::core::z80::memory::MemoryMap;
use crate::utils::crc32;

// Constantes dos mappers
pub const MAPPER_NONE: u8             = 0x00;
pub const MAPPER_TEREBI: u8           = 0x01;
pub const MAPPER_RAM_2K: u8           = 0x02;
pub const MAPPER_RAM_8K: u8           = 0x03;
pub const MAPPER_RAM_8K_EXT1: u8      = 0x04;
pub const MAPPER_SEGA: u8             = 0x10;
pub const MAPPER_SEGA_X: u8           = 0x11;
pub const MAPPER_93C46: u8            = 0x12;
pub const MAPPER_CODIES: u8           = 0x13;
pub const MAPPER_MULTI_16K: u8        = 0x14;
pub const MAPPER_KOREA_16K_V1: u8     = 0x15;
pub const MAPPER_KOREA_16K_V2: u8     = 0x16;
pub const MAPPER_MULTI_2X16K_V1: u8   = 0x17;
pub const MAPPER_MULTI_2X16K_V2: u8   = 0x18;
pub const MAPPER_MULTI_16K_32K_V1: u8 = 0x19;
pub const MAPPER_MULTI_16K_32K_V2: u8 = 0x1A;
pub const MAPPER_ZEMINA_16K_32K: u8   = 0x1B;
pub const MAPPER_HWASUNG: u8          = 0x1C;
pub const MAPPER_MSX_16K: u8          = 0x1D;
pub const MAPPER_KOREA_8K: u8         = 0x20;
pub const MAPPER_MSX_8K: u8           = 0x21;
pub const MAPPER_MSX_8K_NEMESIS: u8   = 0x22;
pub const MAPPER_MULTI_8K: u8         = 0x23;
pub const MAPPER_MULTI_4X8K: u8       = 0x24;
pub const MAPPER_ZEMINA_4X8K: u8      = 0x25;
pub const MAPPER_MULTI_32K: u8        = 0x40;
pub const MAPPER_MULTI_32K_16K: u8    = 0x41;
pub const MAPPER_HICOM: u8            = 0x42;

/// Informações do ROM detectadas
#[derive(Debug, Clone)]
pub struct RomInfo {
    pub crc: u32,
    pub g_3d: bool,
    pub fm: bool,
    pub peripheral: u8,
    pub mapper: u8,
    pub system: System,
    pub region: Region,
}

/// Configuração de hardware do ROM
#[derive(Debug, Clone)]
pub struct RomHardware {
    pub fcr: [u8; 4],    // Frame Control Registers
    pub mapper: u8,
    pub pages: u16,      // Número de páginas/bancos
}

/// Slots de memória atuais
#[derive(Debug, Clone)]
pub struct MemorySlot {
    pub rom: *const u8,  // Ponteiro para ROM (usar raw pointer por performance)
    pub fcr: [u8; 4],
    pub mapper: u8,
    pub pages: u16,
}

/// Contexto do cartucho SMS
pub struct SmsCartridge {
    pub rom_hw: RomHardware,
    pub bios_hw: RomHardware,
    pub slot: MemorySlot,
    pub game_list: Vec<RomInfo>,
}

impl SmsCartridge {
    /// Cria um novo contexto de cartucho SMS
    pub fn new() -> Self {
        Self {
            rom_hw: RomHardware {
                fcr: [0; 4],
                mapper: MAPPER_NONE,
                pages: 0,
            },
            bios_hw: RomHardware {
                fcr: [0; 4],
                mapper: MAPPER_NONE,
                pages: 0,
            },
            slot: MemorySlot {
                rom: std::ptr::null(),
                fcr: [0; 4],
                mapper: MAPPER_NONE,
                pages: 0,
            },
            game_list: Self::create_game_list(),
        }
    }
    
    /// Inicializa o cartucho
    pub fn init(&mut self, cart: &Cartridge, system_hw: System, region: Region) {
        let crc = crc32::calculate(&cart.rom);
        
        // Configuração padrão
        self.rom_hw.mapper = if cart.rom.len() > 0xC000 { MAPPER_SEGA } else { MAPPER_NONE };
        self.rom_hw.pages = self.calculate_pages(cart.rom.len());
        
        // Auto-detecção do jogo
        self.auto_detect(crc, system_hw, region);
        
        // Inicializa hardware extra
        self.init_extra_hardware();
    }
    
    /// Reinicia o cartucho
    pub fn reset(&mut self, cart: &Cartridge, memory: &mut MemoryMap) {
        self.reset_paging();
        self.reset_memory_map(cart, memory);
    }
    
    /// Alterna entre cartucho/BIOS
    pub fn switch(&mut self, mode: u8, cart: &Cartridge, memory: &mut MemoryMap) {
        // Implementação do sms_cart_switch
        // ...
    }
    
    /// Calcula tamanho da RAM
    pub fn ram_size(&self) -> usize {
        match self.rom_hw.mapper {
            MAPPER_RAM_8K | MAPPER_RAM_8K_EXT1 => 0x2000,
            MAPPER_RAM_2K => 0x800,
            _ => 0,
        }
    }
    
    /// Detecta região
    pub fn detect_region(&self, cart: &Cartridge) -> Region {
        let crc = crc32::calculate(&cart.rom);
        
        // Verifica lista de jogos
        for game in &self.game_list {
            if game.crc == crc {
                return game.region;
            }
        }
        
        // Região padrão
        Region::USA
    }
    
    /// Cria a lista de jogos conhecidos
    fn create_game_list() -> Vec<RomInfo> {
        vec![
            // Jogos usando mapper SEGA
            RomInfo {
                crc: 0x32759751,
                g_3d: false,
                fm: true,
                peripheral: 0,
                mapper: MAPPER_SEGA,
                system: System::SMS,
                region: Region::JapanNTSC,
            },
            // ... adicionar todos os jogos da lista
            // Jogos usando mapper Codemasters
            RomInfo {
                crc: 0x29822980,
                g_3d: false,
                fm: false,
                peripheral: 0,
                mapper: MAPPER_CODIES,
                system: System::SMS2,
                region: Region::Europe,
            },
            // ... continuar com todos os jogos
        ]
    }
    
    /// Calcula número de páginas baseado no tamanho e mapper
    fn calculate_pages(&self, rom_size: usize) -> u16 {
        match self.rom_hw.mapper {
            m if m < MAPPER_SEGA => ((rom_size + (1 << 10) - 1) >> 10) as u16,
            m if (m & MAPPER_KOREA_8K) != 0 => ((rom_size + (1 << 13) - 1) >> 13) as u16,
            m if (m & MAPPER_MULTI_32K) != 0 => ((rom_size + (1 << 15) - 1) >> 15) as u16,
            _ => ((rom_size + (1 << 14) - 1) >> 14) as u16,
        }
    }
    
    /// Auto-detecta configurações do jogo
    fn auto_detect(&mut self, crc: u32, system_hw: System, region: Region) {
        for game in &self.game_list {
            if game.crc == crc {
                self.rom_hw.mapper = game.mapper;
                // Configurações específicas do jogo
                break;
            }
        }
    }
    
    /// Inicializa hardware extra
    fn init_extra_hardware(&self) {
        match self.rom_hw.mapper {
            MAPPER_93C46 => {
                // Inicializa EEPROM
            }
            MAPPER_TEREBI => {
                // Inicializa Terebi Oekaki
            }
            _ => {}
        }
    }
    
    /// Reinicia paginação
    fn reset_paging(&mut self) {
        match self.rom_hw.mapper {
            MAPPER_SEGA | MAPPER_SEGA_X => {
                self.rom_hw.fcr = [0, 0, 1, 2];
            }
            MAPPER_ZEMINA_16K_32K => {
                self.rom_hw.fcr = [0, 0, 1, 1];
            }
            MAPPER_ZEMINA_4X8K => {
                self.rom_hw.fcr = [3, 2, 1, 0];
            }
            MAPPER_KOREA_8K | MAPPER_MSX_8K | MAPPER_MSX_8K_NEMESIS |
            MAPPER_MSX_16K | MAPPER_MULTI_4X8K | MAPPER_MULTI_8K => {
                self.rom_hw.fcr = [0; 4];
            }
            _ => {
                self.rom_hw.fcr = [0, 0, 1, 0];
            }
        }
        
        // BIOS usa SEGA mapper por padrão
        self.bios_hw.fcr = [0, 0, 1, 2];
    }
    
    /// Reinicia mapa de memória
    fn reset_memory_map(&mut self, cart: &Cartridge, memory: &mut MemoryMap) {
        // Implementação do mapper_reset
        // ...
    }
}

// Trait para mappers
pub trait Mapper {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, value: u8);
    fn reset(&mut self);
}

// Re-exportação dos módulos
pub use self::common::*;
pub use self::sega::*;
pub use self::codemasters::*;
pub use self::korean::*;
pub use self::msx::*;
pub use self::multi::*;
pub use self::zemina::*;
pub use self::eeprom::*;
pub use self::terebi::*;
pub use self::ram::*;
