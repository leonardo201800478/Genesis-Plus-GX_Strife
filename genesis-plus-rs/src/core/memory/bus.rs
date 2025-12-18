//! Barramento de memória principal - funções READ/WRITE.
//! Este é o núcleo do sistema de memória, chamado pela CPU.

use std::sync::{Arc, Mutex};
use crate::core::memory::map::create_rom_handlers;
use crate::core::memory::{ADDRESS_MASK, MemoryError, MemoryResult};
use crate::core::memory::cart::Cartridge;
use crate::core::memory::map::{MemoryMap, MemoryHandler, MemRegion};
use crate::core::memory::sram::SaveRam;
use log::{trace, warn};

/// Barramento de memória principal
pub struct MemoryBus {
    pub cart: Option<Arc<Mutex<Cartridge>>>,
    pub map: MemoryMap,
    pub zram: [u8; 8192],     // 8KB Z80 RAM
    pub ioports: [u8; 256],   // Portas I/O
    pub vram: [u16; 65536],   // 128KB VRAM (64K words)
    pub cram: [u16; 64],      // 128 bytes CRAM (64 words)
    pub vsram: [u16; 40],     // 80 bytes VSRAM (40 words)
    
    pub genesis_mode: bool,   // true = Genesis, false = Master System
    pub tmss_enabled: bool,   // Proteção TMSS
    pub tmss_reg: u8,
    
    pub cycles: u64,          // Ciclos totais executados
}

impl MemoryBus {
    /// Cria um novo barramento de memória
    pub fn new() -> Self {
        Self {
            cart: None,
            map: MemoryMap::new(),
            zram: [0; 8192],
            ioports: [0; 256],
            vram: [0; 65536],
            cram: [0; 64],
            vsram: [0; 40],
            
            genesis_mode: true,
            tmss_enabled: false,
            tmss_reg: 0,
            
            cycles: 0,
        }
    }
    
    /// Inicializa o barramento com um cartucho
    pub fn init(&mut self, cart: Cartridge) -> MemoryResult<()> {
        let cart_arc = Arc::new(Mutex::new(cart));
        self.cart = Some(Arc::clone(&cart_arc));
        self.setup_memory_map(cart_arc)
    }
    
    /// Configura o mapa de memória baseado no cartucho
    fn setup_memory_map(&mut self, cart: Arc<Mutex<Cartridge>>) -> MemoryResult<()> {
        // Mapeia ROM (0x000000 - 0x3FFFFF) em 4 blocos de 1MB
        let (rom_handler0, rom_handler1, rom_handler2, rom_handler3) = create_rom_handlers(cart);
        
        self.map.map_region(0x000000, 0x0FFFFF, rom_handler0);
        self.map.map_region(0x100000, 0x1FFFFF, rom_handler1);
        self.map.map_region(0x200000, 0x2FFFFF, rom_handler2);
        self.map.map_region(0x300000, 0x3FFFFF, rom_handler3);
        
        // Mapeia ROM (0x000000 - 0x3FFFFF)
        // (Implementação completa requer create_rom_handlers)
        
        // Mapeia Z80 RAM (0xA00000 - 0xA01FFF)
        let zram_handler = MemoryHandler {
            read_byte: |addr| self.read_zram(addr),
            read_word: |addr| self.read_zram_word(addr),
            write_byte: |addr, val| self.write_zram(addr, val),
            write_word: |addr, val| self.write_zram_word(addr, val),
            region: MemRegion::Zram,
        };
        self.map.map_region(0xA00000, 0xA01FFF, zram_handler);
        
        // Mapeia I/O (0xA10000 - 0xA1001F)
        let io_handler = MemoryHandler {
            read_byte: |addr| self.read_io(addr),
            read_word: |addr| self.read_io_word(addr),
            write_byte: |addr, val| self.write_io(addr, val),
            write_word: |addr, val| self.write_io_word(addr, val),
            region: MemRegion::Io,
        };
        self.map.map_region(0xA10000, 0xA1001F, io_handler);
        
        // Mapeia VDP (0xC00000 - 0xC0001F)
        let vdp_handler = MemoryHandler {
            read_byte: |addr| self.read_vdp(addr),
            read_word: |addr| self.read_vdp_word(addr),
            write_byte: |addr, val| self.write_vdp(addr, val),
            write_word: |addr, val| self.write_vdp_word(addr, val),
            region: MemRegion::Vdp,
        };
        self.map.map_region(0xC00000, 0xC0001F, vdp_handler);
        
        Ok(())
    }
    
    // --- Funções principais de acesso à memória (chamadas pela CPU) ---
    
    /// Lê um byte (8-bit) do endereço especificado
    pub fn read_byte(&self, addr: u32) -> u8 {
        let masked_addr = addr & ADDRESS_MASK;
        let handler = self.map.get_handler(masked_addr);
        (handler.read_byte)(masked_addr)
    }
    
    /// Lê uma palavra (16-bit) do endereço especificado
    pub fn read_word(&self, addr: u32) -> u16 {
        let masked_addr = addr & ADDRESS_MASK;
        let handler = self.map.get_handler(masked_addr);
        
        // Endereços ímpares são permitidos no 68000 mas mais lentos
        if masked_addr & 1 == 1 {
            let low = (handler.read_byte)(masked_addr) as u16;
            let high = (handler.read_byte)(masked_addr.wrapping_add(1)) as u16;
            (high << 8) | low
        } else {
            (handler.read_word)(masked_addr)
        }
    }
    
    /// Escreve um byte no endereço especificado
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        let masked_addr = addr & ADDRESS_MASK;
        let handler = self.map.get_handler(masked_addr);
        (handler.write_byte)(masked_addr, value);
    }
    
    /// Escreve uma palavra no endereço especificado
    pub fn write_word(&mut self, addr: u32, value: u16) {
        let masked_addr = addr & ADDRESS_MASK;
        let handler = self.map.get_handler(masked_addr);
        
        if masked_addr & 1 == 1 {
            // Escrita não alinhada
            (handler.write_byte)(masked_addr, value as u8);
            (handler.write_byte)(masked_addr.wrapping_add(1), (value >> 8) as u8);
        } else {
            (handler.write_word)(masked_addr, value);
        }
    }
    
    // --- Handlers específicos para cada região ---
    
    /// Lê da Z80 RAM
    fn read_zram(&self, addr: u32) -> u8 {
        let offset = (addr & 0x1FFF) as usize;
        if offset < 8192 {
            self.zram[offset]
        } else {
            0xFF
        }
    }
    
    fn read_zram_word(&self, addr: u32) -> u16 {
        let offset = (addr & 0x1FFF) as usize;
        if offset < 8191 {
            let low = self.zram[offset] as u16;
            let high = self.zram[offset + 1] as u16;
            (high << 8) | low
        } else {
            0xFFFF
        }
    }
    
    fn write_zram(&mut self, addr: u32, value: u8) {
        let offset = (addr & 0x1FFF) as usize;
        if offset < 8192 {
            self.zram[offset] = value;
        }
    }
    
    fn write_zram_word(&mut self, addr: u32, value: u16) {
        let offset = (addr & 0x1FFF) as usize;
        if offset < 8191 {
            self.zram[offset] = value as u8;
            self.zram[offset + 1] = (value >> 8) as u8;
        }
    }
    
    /// Lê de I/O
    fn read_io(&self, addr: u32) -> u8 {
        let offset = (addr & 0x1F) as usize;
        if offset < 256 {
            self.ioports[offset]
        } else {
            0xFF
        }
    }
    
    fn read_io_word(&self, addr: u32) -> u16 {
        let offset = (addr & 0x1F) as usize;
        if offset < 255 {
            let low = self.ioports[offset] as u16;
            let high = self.ioports[offset + 1] as u16;
            (high << 8) | low
        } else {
            0xFFFF
        }
    }
    
    fn write_io(&mut self, addr: u32, value: u8) {
        let offset = (addr & 0x1F) as usize;
        if offset < 256 {
            self.ioports[offset] = value;
        }
    }
    
    fn write_io_word(&mut self, addr: u32, value: u16) {
        let offset = (addr & 0x1F) as usize;
        if offset < 255 {
            self.ioports[offset] = value as u8;
            self.ioports[offset + 1] = (value >> 8) as u8;
        }
    }
    
    /// Lê do VDP (implementação simplificada)
    fn read_vdp(&self, addr: u32) -> u8 {
        // Implementação real é complexa
        0
    }
    
    fn read_vdp_word(&self, addr: u32) -> u16 {
        0
    }
    
    fn write_vdp(&mut self, addr: u32, value: u8) {
        // Será implementado no módulo VDP
    }
    
    fn write_vdp_word(&mut self, addr: u32, value: u16) {
        // Será implementado no módulo VDP
    }
    
    /// Avança o contador de ciclos
    pub fn add_cycles(&mut self, cycles: u32) {
        self.cycles = self.cycles.wrapping_add(cycles as u64);
    }
    
    /// Reseta o barramento
    pub fn reset(&mut self) {
        self.zram = [0; 8192];
        self.ioports = [0; 256];
        self.vram = [0; 65536];
        self.cram = [0; 64];
        self.vsram = [0; 40];
        self.cycles = 0;
        
        if let Some(cart) = &mut self.cart {
            // Reseta o cartucho também
        }
    }
}