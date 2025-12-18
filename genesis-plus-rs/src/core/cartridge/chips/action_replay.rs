//! Action Replay / Pro Action Replay hardware support
//! Baseado em areplay.c do Genesis Plus GX

use crate::core::memory::MemoryBus;
use crate::core::cartridge::Cartridge;
use log::{info, warn, debug};
use std::path::Path;
use std::fs::File;
use std::io::Read;

/// Tipos de Action Replay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionReplayType {
    None,      // Não habilitado
    Standard,  // Action Replay normal (32KB ROM)
    Pro1,      // Pro Action Replay (2x32KB ROM)
    Pro2,      // Pro Action Replay 2 (2x32KB ROM)
}

/// Status do Action Replay
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActionReplayStatus {
    Off,       // Desligado
    On,        // Cheats ativados
    Trainer,   // Modo trainer (apenas para Pro)
}

/// Estrutura principal do Action Replay
pub struct ActionReplay {
    enabled: ActionReplayType,
    status: ActionReplayStatus,
    ram: [u8; 0x10000],       // 64KB de RAM interna
    regs: [u16; 13],          // Registradores
    old: [u16; 4],            // Dados originais (para restaurar)
    data: [u16; 4],           // Dados dos patches
    addr: [u32; 4],           // Endereços dos patches
    rom: Vec<u8>,             // ROM do Action Replay (até 64KB)
}

impl ActionReplay {
    /// Cria uma nova instância do Action Replay
    pub fn new() -> Self {
        Self {
            enabled: ActionReplayType::None,
            status: ActionReplayStatus::Off,
            ram: [0xFF; 0x10000], // RAM inicializada com 0xFF
            regs: [0; 13],
            old: [0; 4],
            data: [0; 4],
            addr: [0; 4],
            rom: Vec::new(),
        }
    }
    
    /// Tenta carregar a ROM do Action Replay e detectar o tipo
    pub fn init<P: AsRef<Path>>(&mut self, rom_path: P) -> bool {
        // Tenta carregar o arquivo (máx 64KB)
        let mut file = match File::open(&rom_path) {
            Ok(f) => f,
            Err(e) => {
                warn!("Não foi possível abrir a ROM do Action Replay: {}", e);
                return false;
            }
        };
        
        let mut buffer = Vec::new();
        if let Err(e) = file.read_to_end(&mut buffer) {
            warn!("Erro ao ler a ROM do Action Replay: {}", e);
            return false;
        }
        
        if buffer.len() > 0x10000 {
            warn!("ROM do Action Replay muito grande ({} bytes)", buffer.len());
            return false;
        }
        
        self.rom = buffer;
        
        // Detecta o tipo de Action Replay
        self.detect_type()
    }
    
    /// Detecta o tipo de Action Replay baseado na assinatura
    fn detect_type(&mut self) -> bool {
        if self.rom.len() < 0x130 {
            warn!("ROM do Action Replay muito pequena para detecção");
            return false;
        }
        
        // Verifica assinatura "ACTION REPLAY   " (16 bytes)
        let signature_standard = b"ACTION REPLAY   ";
        let signature_pro1 = b"ACTION REPLAY 2 ";
        let signature_pro2 = b"ACTION REPLAY II";
        
        // Action Replay normal (32KB ROM)
        if &self.rom[0x120..0x130] == signature_standard {
            info!("Action Replay normal detectado");
            self.enabled = ActionReplayType::Standard;
            
            // $0000-$7fff espelhado em $8000-$ffff
            let rom_lower = &self.rom[0..0x8000];
            self.rom.extend_from_slice(rom_lower);
            
            return true;
        }
        
        // Lê o byte do stack pointer (para detecção Pro)
        let sp = self.rom[0x01];
        
        // Pro Action Replay (2x32KB ROM)
        if sp == 0x42 && &self.rom[0x120..0x130] == signature_pro1 {
            info!("Pro Action Replay detectado");
            self.enabled = ActionReplayType::Pro1;
            return true;
        }
        
        // Pro Action Replay 2 (2x32KB ROM)
        if sp == 0x60 && self.rom.len() >= 0x3D6 && &self.rom[0x3C6..0x3D6] == signature_pro2 {
            info!("Pro Action Replay 2 detectado");
            self.enabled = ActionReplayType::Pro2;
            return true;
        }
        
        warn("Tipo de Action Replay não reconhecido");
        false
    }
    
    /// Reseta o Action Replay
    pub fn reset(&mut self, hard_reset: bool) {
        if self.enabled == ActionReplayType::None {
            return;
        }
        
        if hard_reset || self.status == ActionReplayStatus::Trainer {
            // Reseta registradores internos
            self.regs = [0; 13];
            self.old = [0; 4];
            self.data = [0; 4];
            self.addr = [0; 4];
            
            // Reseta RAM interna no power-on
            if hard_reset {
                self.ram = [0xFF; 0x10000];
            }
        }
    }
    
    /// Retorna o status atual
    pub fn get_status(&self) -> Option<ActionReplayStatus> {
        if self.enabled != ActionReplayType::None {
            Some(self.status)
        } else {
            None
        }
    }
    
    /// Define o status do Action Replay
    pub fn set_status(&mut self, status: ActionReplayStatus, cart: &mut Cartridge) {
        if self.enabled == ActionReplayType::None {
            return;
        }
        
        // No Trainer mode para Action Replay normal
        if self.enabled == ActionReplayType::Standard && status == ActionReplayStatus::Trainer {
            return;
        }
        
        // Verifica mudanças de status
        match status {
            ActionReplayStatus::Off | ActionReplayStatus::Trainer => {
                // Checa se os patches estavam ativados
                if self.status == ActionReplayStatus::On {
                    // Restaura dados originais
                    self.restore_patches(cart);
                }
            }
            
            ActionReplayStatus::On => {
                // Checa se os patches estavam desativados
                if self.status != ActionReplayStatus::On {
                    // Aplica patches
                    self.apply_patches(cart);
                }
            }
        }
        
        // Atualiza status
        self.status = status;
        info!("Action Replay status: {:?}", status);
    }
    
    /// Aplica os patches ao cartucho
    fn apply_patches(&mut self, cart: &mut Cartridge) {
        // Decodifica dados dos patches
        self.data[0] = self.regs[0];
        self.data[1] = self.regs[4];
        self.data[2] = self.regs[7];
        self.data[3] = self.regs[10];
        
        // Decodifica endereços dos patches ($000000-$7fffff)
        self.addr[0] = ((self.regs[1] as u32) | (((self.regs[2] as u32) & 0x3F00) << 8)) << 1;
        self.addr[1] = ((self.regs[5] as u32) | (((self.regs[6] as u32) & 0x3F00) << 8)) << 1;
        self.addr[2] = ((self.regs[8] as u32) | (((self.regs[9] as u32) & 0x3F00) << 8)) << 1;
        self.addr[3] = ((self.regs[11] as u32) | (((self.regs[12] as u32) & 0x3F00) << 8)) << 1;
        
        // Salva dados originais
        // NOTA: Precisamos de uma forma de ler/escrever na ROM do cartucho
        // Isso será integrado quando tivermos o sistema de cartuchos completo
        
        debug!("Action Replay patches aplicados: {:?} em {:?}", self.data, self.addr);
    }
    
    /// Restaura os dados originais do cartucho
    fn restore_patches(&self, cart: &mut Cartridge) {
        // Restaura dados originais
        // NOTA: Implementação similar à apply_patches, mas restaurando self.old
        debug!("Action Replay patches restaurados");
    }
    
    /// Escreve nos registradores do Action Replay
    pub fn write_regs(&mut self, address: u32, data: u16, cart: &mut Cartridge) {
        // Offset do registrador
        let offset = ((address & 0xFFFF) >> 1) as usize;
        
        if offset > 12 {
            // Escrita ignorada (deveria ser tratada como escrita não usada)
            return;
        }
        
        // Atualiza registrador interno
        self.regs[offset] = data;
        
        // Registrador MODE
        if offset == 3 && data == 0xFFFF {
            // Checa status do switch
            if self.status == ActionReplayStatus::On {
                // Reseta patches existentes
                self.set_status(ActionReplayStatus::Off, cart);
                self.set_status(ActionReplayStatus::On, cart);
            }
            
            // Habilita ROM do cartucho (não da Action Replay)
            // Isso será tratado pelo mapeamento de memória
            debug!("Action Replay: ROM do cartucho habilitada via MODE register");
        }
    }
    
    /// Escreve no registrador do Action Replay 2
    pub fn write_reg_pro2(&mut self, address: u32, data: u16) {
        // Habilita ROM do cartucho
        if (address & 0xFF) == 0x78 && data == 0xFFFF {
            debug!("Action Replay 2: ROM do cartucho habilitada");
        }
    }
    
    /// Escreve na RAM do Action Replay (8-bit)
    pub fn write_ram_8(&mut self, address: u32, data: u8) {
        let addr = (address & 0xFFFE) as usize;
        
        // Escritas de byte são tratadas como escritas de word,
        // com LSB duplicado em MSB (/LWR não é usado)
        if addr < self.ram.len() - 1 {
            let word = (data as u16) | ((data as u16) << 8);
            self.ram[addr] = word as u8;
            self.ram[addr + 1] = (word >> 8) as u8;
        }
    }
    
    /// Lê da ROM do Action Replay
    pub fn read_rom(&self, address: u32) -> u16 {
        let addr = (address & 0xFFFF) as usize;
        
        if addr < self.rom.len() - 1 {
            let low = self.rom[addr] as u16;
            let high = self.rom[addr + 1] as u16;
            (high << 8) | low
        } else {
            0xFFFF
        }
    }
    
    /// Lê da RAM do Action Replay
    pub fn read_ram(&self, address: u32) -> u16 {
        let addr = (address & 0xFFFE) as usize;
        
        if addr < self.ram.len() - 1 {
            let low = self.ram[addr] as u16;
            let high = self.ram[addr + 1] as u16;
            (high << 8) | low
        } else {
            0xFFFF
        }
    }
    
    /// Retorna se o Action Replay está habilitado
    pub fn is_enabled(&self) -> bool {
        self.enabled != ActionReplayType::None
    }
    
    /// Retorna o tipo do Action Replay
    pub fn get_type(&self) -> ActionReplayType {
        self.enabled
    }
    
    /// Configura o mapeamento de memória no barramento
    pub fn setup_memory_map(&self, bus: &mut MemoryBus) {
        if !self.is_enabled() {
            return;
        }
        
        match self.enabled {
            ActionReplayType::Standard | ActionReplayType::Pro1 => {
                // Registradores mapeados em $010000-$01FFFF
                // NOTA: Isso será integrado quando tivermos o sistema de mapeamento
            }
            ActionReplayType::Pro2 => {
                // Registrador mapeado em $100000-$10FFFF
                // NOTA: Isso será integrado quando tivermos o sistema de mapeamento
            }
            _ => {}
        }
        
        // RAM interna mapeada em $420000-$42FFFF ou $600000-$60FFFF
        // dependendo do tipo
        let ram_page = match self.enabled {
            ActionReplayType::Pro1 => 0x42,  // $420000-$42FFFF
            ActionReplayType::Pro2 => 0x60,  // $600000-$60FFFF
            _ => return,
        };
        
        // NOTA: O mapeamento será feito quando integrarmos com o MemoryBus
        debug!("Action Replay RAM mapeada em ${:06X}-${:06X}", 
               ram_page << 16, (ram_page << 16) | 0xFFFF);
    }
}

impl Default for ActionReplay {
    fn default() -> Self {
        Self::new()
    }
}