//! Tabelas de mapeamento e handlers de memória.
//! Baseado em `mem68k.c` e `memory.h` do Genesis Plus GX.

use crate::core::memory::{ADDRESS_MASK, MemoryError};
use crate::core::memory::cart::Cartridge;
use std::sync::{Arc, Mutex};

/// Região de memória
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemRegion {
    Rom,        // Cartucho ROM
    Sram,       // Save RAM
    Zram,       // RAM do Z80 (8KB)
    Zrom,       // ROM do Z80 (cartucho)
    Io,         // I/O (VDP, PSG, etc.)
    Vdp,        // Vídeo Display Processor
    Psg,        // Programmable Sound Generator
    Unmapped,   // Nada mapeado
}

/// Handler para acesso à memória (usando trait objects para flexibilidade)
pub struct MemoryHandler {
    pub read_byte: Box<dyn Fn(u32) -> u8 + Send + Sync>,
    pub read_word: Box<dyn Fn(u32) -> u16 + Send + Sync>,
    pub write_byte: Box<dyn Fn(u32, u8) + Send + Sync>,
    pub write_word: Box<dyn Fn(u32, u16) + Send + Sync>,
    pub region: MemRegion,
}

impl Clone for MemoryHandler {
    fn clone(&self) -> Self {
        Self {
            read_byte: self.read_byte.clone(),
            read_word: self.read_word.clone(),
            write_byte: self.write_byte.clone(),
            write_word: self.write_word.clone(),
            region: self.region,
        }
    }
}

/// Tabela de mapeamento (indexada por página de 64KB)
pub struct MemoryMap {
    pub handlers: [MemoryHandler; 256], // 16MB / 64KB = 256 páginas
}

impl MemoryMap {
    /// Cria um novo mapa de memória vazio
    pub fn new() -> Self {
        let unmapped_handler = MemoryHandler {
            read_byte: Box::new(|_| 0xFF),
            read_word: Box::new(|_| 0xFFFF),
            write_byte: Box::new(|_, _| {}),
            write_word: Box::new(|_, _| {}),
            region: MemRegion::Unmapped,
        };
        
        Self {
            handlers: [unmapped_handler; 256],
        }
    }
    
    /// Mapeia uma região de endereços para um handler
    pub fn map_region(&mut self, start: u32, end: u32, handler: MemoryHandler) {
        let page_start = (start >> 16) as usize;
        let page_end = (end >> 16) as usize;
        
        for page in page_start..=page_end {
            if page < 256 {
                self.handlers[page] = handler.clone();
            }
        }
    }
    
    /// Obtém o handler para um endereço específico
    pub fn get_handler(&self, addr: u32) -> &MemoryHandler {
        let page = ((addr & ADDRESS_MASK) >> 16) as usize;
        &self.handlers[page]
    }
}

/// Cria handlers para a ROM do cartucho
pub fn create_rom_handlers(cart: Arc<Mutex<Cartridge>>) 
    -> (MemoryHandler, MemoryHandler, MemoryHandler, MemoryHandler) 
{
    let cart1 = Arc::clone(&cart);
    let read_byte = move |addr: u32| {
        let cart = cart1.lock().unwrap();
        cart.read_rom(addr)
    };
    
    let cart2 = Arc::clone(&cart);
    let read_word = move |addr: u32| {
        let cart = cart2.lock().unwrap();
        let low = cart.read_rom(addr);
        let high = cart.read_rom(addr.wrapping_add(1));
        (high as u16) << 8 | low as u16
    };
    
    let write_byte = |_: u32, _: u8| {};
    let write_word = |_: u32, _: u16| {};
    
    let handler = MemoryHandler {
        read_byte: Box::new(read_byte),
        read_word: Box::new(read_word),
        write_byte: Box::new(write_byte),
        write_word: Box::new(write_word),
        region: MemRegion::Rom,
    };
    
    (handler.clone(), handler.clone(), handler.clone(), handler)
}