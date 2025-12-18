//! Estruturas e funções para gerenciamento de cartuchos.
//! Baseado em `cart.h` e `cart.c` do Genesis Plus GX.

use crate::core::memory::{MAX_ROM_SIZE, MAX_SRAM_SIZE, MemoryError, MemoryResult};
use std::path::Path;
use std::fs::File;
use std::io::Read;
use log::{info, warn, error};

/// Tipo de mapeador de cartucho
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapperType {
    None,           // ROM linear (até 4MB)
    Sega,           // Mapper Sega (Bank Register)
    Codemasters,    // Codemasters (simples)
    Korean,         // Mapper coreano (SSF2, etc.)
    Realtec,        // Realtec (512K banks)
    Sms,            // Master System
    GameGear,       // Game Gear
}

/// Estrutura principal do cartucho
pub struct Cartridge {
    pub rom: Vec<u8>,
    pub rom_size: usize,
    pub rom_mask: u32,
    
    pub sram: Vec<u8>,
    pub sram_size: usize,
    pub sram_mask: u32,
    pub sram_enabled: bool,
    pub sram_dirty: bool,
    
    pub mapper: MapperType,
    pub bank_regs: [u8; 8],  // Registradores de banco
    pub bank_start: [u32; 8], // Endereços iniciais dos bancos
    
    pub has_sram: bool,
    pub has_eeprom: bool,
    pub is_pal: bool,
    pub region: u8,
    pub header: [u8; 0x200], // Cabeçalho ROM
}

impl Cartridge {
    /// Cria um novo cartucho vazio
    pub fn new() -> Self {
        Self {
            rom: Vec::new(),
            rom_size: 0,
            rom_mask: 0,
            
            sram: Vec::new(),
            sram_size: 0,
            sram_mask: 0,
            sram_enabled: false,
            sram_dirty: false,
            
            mapper: MapperType::None,
            bank_regs: [0; 8],
            bank_start: [0; 8],
            
            has_sram: false,
            has_eeprom: false,
            is_pal: false,
            region: 0,
            header: [0; 0x200],
        }
    }
    
    /// Carrega uma ROM do arquivo
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> MemoryResult<()> {
        let mut file = File::open(path).map_err(|_| MemoryError::InvalidCartridge)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).map_err(|_| MemoryError::InvalidCartridge)?;
        
        self.load_from_buffer(&buffer)
    }
    
    /// Carrega uma ROM de um buffer (usado pelo RetroArch)
    pub fn load_from_buffer(&mut self, buffer: &[u8]) -> MemoryResult<()> {
        if buffer.len() > MAX_ROM_SIZE {
            return Err(MemoryError::RomTooLarge);
        }
        
        self.rom_size = buffer.len();
        self.rom = buffer.to_vec();
        
        // Máscara para endereçamento (potência de 2 - 1)
        self.rom_mask = if self.rom_size.is_power_of_two() {
            self.rom_size as u32 - 1
        } else {
            (1 << (32 - self.rom_size.leading_zeros())) as u32 - 1
        };
        
        // Copia cabeçalho (se disponível)
        let header_len = std::cmp::min(0x200, self.rom_size);
        self.header[..header_len].copy_from_slice(&self.rom[..header_len]);
        
        // Detecta mapeador e configurações
        self.detect_mapper()?;
        self.detect_sram();
        self.detect_region();
        
        info!("Cartucho carregado: {} bytes, Mapper: {:?}, SRAM: {}", 
              self.rom_size, self.mapper, self.has_sram);
        
        Ok(())
    }
    
    /// Detecta o tipo de mapeador automaticamente
    fn detect_mapper(&mut self) -> MemoryResult<()> {
        // Verificação baseada no tamanho e no cabeçalho
        if self.rom_size <= 0x400000 { // Até 4MB
            self.mapper = MapperType::None;
            
            // Verifica assinaturas específicas no cabeçalho
            if self.rom_size >= 0x200 {
                // Verifica se é Codemasters
                if self.header[0x180] == 0x53 && self.header[0x181] == 0x45 &&  // "SE"
                   self.header[0x182] == 0x47 && self.header[0x183] == 0x41 {   // "GA"
                    self.mapper = MapperType::Sega;
                }
                // Mais detecções aqui...
            }
        } else {
            // ROMs > 4MB precisam de mapper Sega
            self.mapper = MapperType::Sega;
        }
        
        // Inicializa registradores de banco
        self.reset_banks();
        
        Ok(())
    }
    
    /// Detecta presença de Save RAM
    fn detect_sram(&mut self) {
        // Verifica no cabeçalho ou por heurísticas
        self.has_sram = false;
        self.sram_size = 0;
        
        if self.rom_size >= 0x200 {
            // Algumas heurísticas simples
            // (No emulador real, isso é mais complexo)
            if self.header[0x1B0] == 0x52 && self.header[0x1B1] == 0x41 { // "RA"
                self.has_sram = true;
                self.sram_size = 0x8000; // 32KB padrão
            }
        }
        
        if self.has_sram {
            self.sram = vec![0; self.sram_size];
            self.sram_mask = self.sram_size as u32 - 1;
        }
    }
    
    /// Detecta região (NTSC/PAL)
    fn detect_region(&mut self) {
        self.region = 0;
        self.is_pal = false;
        
        if self.rom_size >= 0x1F0 {
            match self.header[0x1F0] {
                0x31 | 0x34 | 0x35 | 0x36 => { // Japão/USA
                    self.region = self.header[0x1F0];
                    self.is_pal = false;
                }
                0x32 | 0x33 | 0x37 | 0x38 | 0x39 | 0x42 | 0x44 | 0x46 | 0x48 | 0x4A => { // Europa
                    self.region = self.header[0x1F0];
                    self.is_pal = true;
                }
                _ => {
                    self.region = 0x34; // USA padrão
                }
            }
        }
    }
    
    /// Reseta os bancos para estado inicial
    fn reset_banks(&mut self) {
        self.bank_regs = [0; 8];
        
        // Configuração inicial depende do mapper
        match self.mapper {
            MapperType::Sega => {
                // Bancos 0-5 mapeiam para a ROM base
                for i in 0..6 {
                    self.bank_start[i] = (i as u32) * 0x10000; // 64KB cada
                }
                // Bancos 6-7 para SRAM/IO
                self.bank_start[6] = 0x300000; // SRAM
                self.bank_start[7] = 0x400000; // I/O
            }
            MapperType::None => {
                // Mapeamento linear
                for i in 0..8 {
                    self.bank_start[i] = (i as u32) * 0x80000; // 512KB cada
                }
            }
            _ => {
                // Outros mappers
                for i in 0..8 {
                    self.bank_start[i] = 0;
                }
            }
        }
    }
    
    /// Atualiza um registrador de banco
    pub fn write_bank_reg(&mut self, reg: usize, value: u8) {
        if reg < 8 {
            self.bank_regs[reg] = value;
            self.update_bank(reg);
        }
    }
    
    /// Atualiza o mapeamento de um banco específico
    fn update_bank(&mut self, bank: usize) {
        match self.mapper {
            MapperType::Sega => {
                if bank < 6 {
                    let bank_num = self.bank_regs[bank] as u32;
                    self.bank_start[bank] = (bank_num & 0x3F) * 0x10000;
                }
            }
            MapperType::Codemasters => {
                if bank == 0 {
                    let bank_num = self.bank_regs[0] as u32;
                    self.bank_start[0] = (bank_num & 0x3) * 0x4000;
                }
            }
            _ => {}
        }
    }
    
    /// Lê um byte da ROM (com mapeamento aplicado)
    pub fn read_rom(&self, addr: u32) -> u8 {
        let mut effective_addr = addr & self.rom_mask;

        if self.mapper != MapperType::None {
            let bank = ((addr >> 16) & 0x07) as usize;
            effective_addr = self.bank_start[bank] | (addr & 0xFFFF);
            effective_addr &= self.rom_mask;
        }

        self.rom
            .get(effective_addr as usize)
            .copied()
            .unwrap_or(0xFF)
    }

    /// Lê um byte da Save RAM
    pub fn read_sram(&self, addr: u32) -> u8 {
        if self.sram_enabled && self.has_sram {
            let sram_addr = (addr & self.sram_mask) as usize;

            if sram_addr < self.sram_size {
                self.sram[sram_addr]
            } else {
                0xFF
            }
        } else {
            0xFF
        }
    }

    /// Escreve um byte na Save RAM
    pub fn write_sram(&mut self, addr: u32, value: u8) {
        if self.sram_enabled && self.has_sram {
            let sram_addr = (addr & self.sram_mask) as usize;

            if sram_addr < self.sram_size {
                self.sram[sram_addr] = value;
                self.sram_dirty = true;
            }
        }
    }
}