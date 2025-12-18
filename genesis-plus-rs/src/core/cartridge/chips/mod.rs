// genesis-plus-rs/src/core/cartridge/chips/mod.rs

//! Cartridge enhancement chips module
//! 
//! This module contains implementations of various cartridge enhancement chips
//! used in Genesis/Mega Drive games.

// Submódulos
pub mod dma;
pub mod action_replay;
pub mod mega_sd;
pub mod processor;
pub mod renderer;
pub mod game_genie;
pub mod texture;
pub mod yx5200;

// Módulo SVP separado - já existe em svp/mod.rs
pub mod svp;

// Re-export main types for easier access
pub use dma::SVPDmaController;
pub use action_replay::{ActionReplay, ActionReplayType, ActionReplayStatus};
pub use mega_sd::{MegaSD, MegaSDMemoryHandler};
pub use processor::{SVPProcessor, RenderCmdType, RenderCommand};
pub use renderer::{SVPRenderer, RenderMode, FrameBuffer, ZBuffer, Camera, Vertex, Polygon};
pub use game_genie::{GameGenie, GameGenieMemoryHandler};
pub use texture::{TextureUnit, Texture, TextureCache, TextureFormat, TextureFilter, TextureWrap};
pub use yx5200::Yx5200;
pub use svp::SVP;  // Re-export do módulo SVP existente

use crate::core::snd::Sound;
use log::{info, warn, debug};

// Common traits for cartridge chips
pub trait CartridgeChip {
    /// Initialize the chip
    fn init(&mut self);
    
    /// Reset the chip to its initial state
    fn reset(&mut self);
    
    /// Update the chip state
    fn update(&mut self, cycles: u32);
    
    /// Save chip state for save states
    fn save_state(&self) -> Vec<u8>;
    
    /// Load chip state from save states
    fn load_state(&mut self, data: &[u8]) -> bool;
    
    /// Get chip type identifier
    fn chip_type(&self) -> ChipType;
    
    /// Check if chip has IRQ pending
    fn irq_pending(&self) -> bool { false }
    
    // Memory interface methods (optional - implemented by chips that need them)
    fn read_byte(&self, _addr: u32) -> u8 { 0xFF }
    fn read_word(&self, _addr: u32) -> u16 { 0xFFFF }
    fn write_byte(&mut self, _addr: u32, _value: u8) {}
    fn write_word(&mut self, _addr: u32, _value: u16) {}
    
    // Audio interface methods (optional - implemented by audio chips)
    fn init_audio(&mut self, _samplerate: u32, _sound: &mut Sound) {}
    fn update_audio(&mut self, _samples: u32, _sound: &mut Sound) {}
    fn write_serial(&mut self, _data: u8) {}
    
    // SVP-specific methods (optional)
    fn get_render_command(&mut self) -> Option<RenderCommand> { None }
    fn command_completed(&mut self) {}
    fn render_command_ready(&self) -> bool { false }
    fn write_command_buffer(&mut self, _value: u16) {}
    fn tick(&mut self, _cycles: u32) {}  // SVP-specific tick method
    fn get_frame_buffer(&self) -> &[u16] { &[] }  // For SVP renderer
}

/// Chip type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChipType {
    /// No enhancement chip
    None,
    /// SVP (Sega Virtua Processor) - Virtua Racing
    SVP,
    /// YX5200 MP3 audio player chip
    Yx5200,
    /// Action Replay / Pro Action Replay
    ActionReplay,
    /// Game Genie
    GameGenie,
    /// MegaSD flashcart
    MegaSD,
    /// Sega PCM sound chip
    SegaPCM,
    /// Other/unknown chip
    Other(u8),
}

impl Default for ChipType {
    fn default() -> Self {
        ChipType::None
    }
}

impl std::fmt::Display for ChipType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ChipType::None => write!(f, "None"),
            ChipType::SVP => write!(f, "SVP"),
            ChipType::Yx5200 => write!(f, "YX5200"),
            ChipType::ActionReplay => write!(f, "ActionReplay"),
            ChipType::GameGenie => write!(f, "GameGenie"),
            ChipType::MegaSD => write!(f, "MegaSD"),
            ChipType::SegaPCM => write!(f, "SegaPCM"),
            ChipType::Other(code) => write!(f, "Other({:02X})", code),
        }
    }
}

// Implementação da trait CartridgeChip para o SVP existente
impl CartridgeChip for SVP {
    fn init(&mut self) {
        // SVP já está inicializado no new()
        info!("SVP initialized");
    }
    
    fn reset(&mut self) {
        SVP::reset(self);
    }
    
    fn update(&mut self, cycles: u32) {
        // O SVP usa tick() em vez de update()
        for _ in 0..cycles {
            self.tick();
        }
    }
    
    fn save_state(&self) -> Vec<u8> {
        // Implementação simplificada - na prática seria mais complexa
        let mut state = Vec::new();
        
        // Salva estado básico
        state.push(self.enabled as u8);
        state.push(self.running as u8);
        state.push(self.irq_pending as u8);
        
        // Salva registradores
        for reg in self.regs.iter() {
            state.extend_from_slice(&reg.to_le_bytes());
        }
        
        // Salva DRAM
        for word in self.dram.iter() {
            state.extend_from_slice(&word.to_le_bytes());
        }
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        // Implementação simplificada
        if data.len() < 3 + self.regs.len() * 2 + self.dram.len() * 2 {
            return false;
        }
        
        let mut offset = 0;
        
        // Carrega estado básico
        self.enabled = data[offset] != 0; offset += 1;
        self.running = data[offset] != 0; offset += 1;
        self.irq_pending = data[offset] != 0; offset += 1;
        
        // Carrega registradores
        for reg in self.regs.iter_mut() {
            *reg = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
        }
        
        // Carrega DRAM
        for word in self.dram.iter_mut() {
            *word = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
        }
        
        true
    }
    
    fn chip_type(&self) -> ChipType {
        ChipType::SVP
    }
    
    fn irq_pending(&self) -> bool {
        self.irq_pending()
    }
    
    fn get_render_command(&mut self) -> Option<RenderCommand> {
        // Delega para o processador interno
        self.processor.get_render_command()
    }
    
    fn command_completed(&mut self) {
        self.processor.command_completed();
    }
    
    fn render_command_ready(&self) -> bool {
        self.processor.render_command_ready()
    }
    
    fn write_command_buffer(&mut self, value: u16) {
        self.processor.write_command_buffer(value);
    }
    
    fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.tick();
        }
    }
    
    fn get_frame_buffer(&self) -> &[u16] {
        self.get_frame_buffer()
    }
    
    fn read_byte(&self, addr: u32) -> u8 {
        let word = self.read_word(addr & !1);
        if (addr & 1) == 0 {
            word as u8
        } else {
            (word >> 8) as u8
        }
    }
    
    fn read_word(&self, addr: u32) -> u16 {
        self.read_word(addr)
    }
    
    fn write_byte(&mut self, addr: u32, value: u8) {
        let word_addr = addr & !1;
        let current = self.read_word(word_addr);
        let new_word = if (addr & 1) == 0 {
            (current & 0xFF00) | (value as u16)
        } else {
            (current & 0x00FF) | ((value as u16) << 8)
        };
        self.write_word(word_addr, new_word);
    }
    
    fn write_word(&mut self, addr: u32, value: u16) {
        self.write_word(addr, value);
    }
}

// Implementações da trait CartridgeChip para outros chips...
// (Manter as implementações anteriores para Yx5200, ActionReplay, etc.)

/// Chip factory function
pub fn create_chip(chip_type: ChipType, rom_path: &str) -> Option<Box<dyn CartridgeChip>> {
    match chip_type {
        ChipType::SVP => {
            info!("Creating SVP chip");
            Some(Box::new(SVP::new()))
        }
        ChipType::Yx5200 => {
            info!("Creating YX5200 chip");
            Some(Box::new(Yx5200::new(rom_path)))
        }
        ChipType::ActionReplay => {
            info!("Creating Action Replay chip");
            let mut ar = ActionReplay::new();
            // Note: Action Replay needs ROM file to be loaded separately
            Some(Box::new(ar))
        }
        ChipType::GameGenie => {
            info!("Creating Game Genie chip");
            let mut gg = GameGenie::new();
            if gg.init() {
                Some(Box::new(gg))
            } else {
                warn!("Failed to initialize Game Genie");
                None
            }
        }
        ChipType::MegaSD => {
            info!("Creating MegaSD chip");
            Some(Box::new(MegaSD::new()))
        }
        ChipType::None => {
            debug!("No cartridge chip needed");
            None
        }
        _ => {
            warn!("Cartridge chip type {} not implemented", chip_type);
            None
        }
    }
}

/// Detect which chip is present based on ROM header or other indicators
pub fn detect_chip(rom_data: &[u8], rom_path: &str) -> ChipType {
    // Check for SVP (Virtua Racing)
    if rom_data.len() >= 0x100 {
        // Check for "VIRTUA RACING" at 0x100
        if &rom_data[0x100..0x10C] == b"VIRTUA RACING" {
            info!("SVP (Virtua Racing) detected");
            return ChipType::SVP;
        }
        
        // Check for other known SVP games by header
        let header = &rom_data[0x100..0x110];
        if header.starts_with(b"SEGA") {
            // Check game name in header
            if let Ok(game_name) = std::str::from_utf8(&header[0x10..0x20]) {
                if game_name.contains("VIRTUA RACING") {
                    info!("SVP detected by game name: {}", game_name);
                    return ChipType::SVP;
                }
            }
        }
    }
    
    // Check for YX5200 by file extension or path
    let lower_path = rom_path.to_lowercase();
    if lower_path.ends_with(".mp3") || 
       lower_path.contains("yx5200") ||
       lower_path.contains("mp3") {
        info!("YX5200 MP3 player detected");
        return ChipType::Yx5200;
    }
    
    // Check for Action Replay or Game Genie by file name
    let file_name = std::path::Path::new(rom_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    if file_name.contains("action") && file_name.contains("replay") {
        info!("Action Replay detected");
        return ChipType::ActionReplay;
    }
    
    if file_name.contains("game") && file_name.contains("genie") {
        info!("Game Genie detected");
        return ChipType::GameGenie;
    }
    
    // Check for MegaSD indicators
    if file_name.contains("megasd") || file_name.contains("mega_sd") {
        info!("MegaSD detected");
        return ChipType::MegaSD;
    }
    
    // Check for known chip signatures in ROM
    if rom_data.len() > 0x200 {
        // Check for Sega PCM signature
        if &rom_data[0x200..0x204] == b"SEGA" {
            // Additional checks for PCM games
            // This is simplified
            return ChipType::SegaPCM;
        }
    }
    
    ChipType::None
}

/// Chip manager that handles multiple chips
pub struct ChipManager {
    chips: Vec<Box<dyn CartridgeChip>>,
    active_chip_type: ChipType,
}

impl ChipManager {
    pub fn new() -> Self {
        Self {
            chips: Vec::new(),
            active_chip_type: ChipType::None,
        }
    }
    
    pub fn add_chip(&mut self, chip: Box<dyn CartridgeChip>) {
        let chip_type = chip.chip_type();
        self.chips.push(chip);
        debug!("Added chip: {}", chip_type);
    }
    
    pub fn remove_chip(&mut self, chip_type: ChipType) -> Option<Box<dyn CartridgeChip>> {
        if let Some(pos) = self.chips.iter().position(|c| c.chip_type() == chip_type) {
            let chip = self.chips.remove(pos);
            debug!("Removed chip: {}", chip_type);
            
            // Se estava ativo, limpa o chip ativo
            if self.active_chip_type == chip_type {
                self.active_chip_type = ChipType::None;
            }
            
            Some(chip)
        } else {
            None
        }
    }
    
    pub fn get_chip(&self, chip_type: ChipType) -> Option<&dyn CartridgeChip> {
        self.chips.iter()
            .find(|c| c.chip_type() == chip_type)
            .map(|c| c.as_ref())
    }
    
    pub fn get_chip_mut(&mut self, chip_type: ChipType) -> Option<&mut dyn CartridgeChip> {
        self.chips.iter_mut()
            .find(|c| c.chip_type() == chip_type)
            .map(|c| c.as_mut())
    }
    
    pub fn set_active_chip(&mut self, chip_type: ChipType) {
        self.active_chip_type = chip_type;
        debug!("Active chip set to: {}", chip_type);
    }
    
    pub fn get_active_chip(&self) -> Option<&dyn CartridgeChip> {
        self.get_chip(self.active_chip_type)
    }
    
    pub fn get_active_chip_mut(&mut self) -> Option<&mut dyn CartridgeChip> {
        self.get_chip_mut(self.active_chip_type)
    }
    
    pub fn has_chip(&self, chip_type: ChipType) -> bool {
        self.chips.iter().any(|c| c.chip_type() == chip_type)
    }
    
    pub fn get_chip_types(&self) -> Vec<ChipType> {
        self.chips.iter().map(|c| c.chip_type()).collect()
    }
    
    pub fn reset_all(&mut self) {
        for chip in &mut self.chips {
            chip.reset();
        }
        debug!("All chips reset");
    }
    
    pub fn update_all(&mut self, cycles: u32) {
        for chip in &mut self.chips {
            chip.update(cycles);
        }
    }
    
    pub fn check_irqs(&self) -> Vec<ChipType> {
        self.chips.iter()
            .filter(|c| c.irq_pending())
            .map(|c| c.chip_type())
            .collect()
    }
}

impl Default for ChipManager {
    fn default() -> Self {
        Self::new()
    }
}

// Estrutura de diretórios esperada:
// genesis-plus-rs/
// ├── src/
// │   └── core/
// │       └── cartridge/
// │           └── chips/
// │               ├── mod.rs                # Este arquivo
// │               ├── dma.rs
// │               ├── action_replay.rs
// │               ├── mega_sd.rs
// │               ├── processor.rs
// │               ├── renderer.rs
// │               ├── game_genie.rs
// │               ├── texture.rs
// │               ├── yx5200.rs
// │               └── svp/                  # Módulo SVP separado
// │                   ├── mod.rs            # Código SVP que você forneceu
// │                   ├── processor.rs      # Cópia do processor.rs
// │                   ├── dma.rs            # Cópia do dma.rs
// │                   ├── texture.rs        # Cópia do texture.rs
// │                   └── renderer.rs       # Cópia do renderer.rs

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_chip() {
        // Test with empty ROM data
        let rom_data = vec![0u8; 256];
        let chip_type = detect_chip(&rom_data, "test.bin");
        assert_eq!(chip_type, ChipType::None);
        
        // Test with MP3 file extension
        let chip_type = detect_chip(&rom_data, "test.mp3");
        assert_eq!(chip_type, ChipType::Yx5200);
    }
    
    #[test]
    fn test_create_chip() {
        // Test creating YX5200 chip
        let chip = create_chip(ChipType::Yx5200, "/test/path");
        assert!(chip.is_some());
        
        // Test creating SVP chip
        let chip = create_chip(ChipType::SVP, "/test/path");
        assert!(chip.is_some());
    }
    
    #[test]
    fn test_chip_manager() {
        let mut manager = ChipManager::new();
        
        // Add chips
        manager.add_chip(create_chip(ChipType::Yx5200, "test").unwrap());
        manager.add_chip(create_chip(ChipType::SVP, "test").unwrap());
        
        assert!(manager.has_chip(ChipType::Yx5200));
        assert!(manager.has_chip(ChipType::SVP));
        assert!(!manager.has_chip(ChipType::ActionReplay));
        
        // Test active chip
        manager.set_active_chip(ChipType::Yx5200);
        assert!(manager.get_active_chip().is_some());
        assert_eq!(manager.get_active_chip().unwrap().chip_type(), ChipType::Yx5200);
        
        // Test update all
        manager.update_all(100);
        
        // Test chip removal
        let removed = manager.remove_chip(ChipType::Yx5200);
        assert!(removed.is_some());
        assert!(!manager.has_chip(ChipType::Yx5200));
    }
}