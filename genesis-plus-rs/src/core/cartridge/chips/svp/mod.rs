//! Sega Virtua Processor (SVP)
//! Chip RISC customizado usado em Virtua Racing para renderização 3D
//! Baseado em arquivos svp.c/svp.h do Genesis Plus GX

use crate::core::cartridge::types::{CartridgeError, CartridgeResult};
use crate::core::memory::MemoryBus;
use log::{info, warn, trace};
use std::sync::{Arc, Mutex};

// Re-exportações
pub use processor::SVPProcessor;
pub use dma::SVPDmaController;
pub use renderer::SVPRenderer;

mod processor;
mod dma;
mod texture;
mod renderer;

/// Estrutura principal do SVP
pub struct SVP {
    /// Processador RISC interno (SSOP-like)
    processor: SVPProcessor,
    
    /// Controlador DMA
    dma: SVPDmaController,

    /// Unidade de textura
    pub texture_unit: texture::TextureUnit,

    /// Renderizador 3D
    pub renderer: renderer::SVPRenderer,
    
    /// Renderizador de polígonos
    renderer: SVPRenderer,
    
    /// Memória interna do SVP (16KB DRAM + 128KB Texture RAM)
    dram: [u16; 8192],      // 16KB como words
    texram: [u8; 131072],   // 128KB Texture RAM
    
    /// Registradores do SVP mapeados na ROM
    regs: [u16; 16],
    
    /// Estado do chip
    enabled: bool,
    running: bool,
    irq_pending: bool,
    
    /// Referência ao barramento principal (para acessar RAM do MD)
    bus: Option<Arc<Mutex<MemoryBus>>>,
}

impl SVP {
    /// Cria uma nova instância do SVP
    pub fn new() -> Self {
        info!("Inicializando SVP (Sega Virtua Processor)");
        
        Self {
            processor: SVPProcessor::new(),
            dma: SVPDmaController::new(),
            renderer: SVPRenderer::new(),
            renderer: renderer::SVPRenderer::new(),
            dram: [0; 8192],
            texture_unit: texture::TextureUnit::new(),
            texram: [0; 131072],
            regs: [0; 16],
            enabled: false,
            running: false,
            irq_pending: false,
            bus: None,
        }
    }
    
    /// Conecta o SVP ao barramento de memória principal
    pub fn connect_bus(&mut self, bus: Arc<Mutex<MemoryBus>>) {
        self.bus = Some(bus);
        
        // Conecta a unidade de textura
        if let Some(texram_arc) = &self.texram_arc {
            self.texture_unit.connect_texram(Arc::clone(texram_arc));
        }
        
        info!("SVP conectado ao barramento principal");
    }
    
    /// Reseta o SVP
    pub fn reset(&mut self) {
        self.processor.reset();
        self.dma.reset();
        self.renderer.reset();
        self.dram.fill(0);
        self.texram.fill(0);
        self.regs.fill(0);
        self.enabled = false;
        self.running = false;
        self.irq_pending = false;
        
        info!("SVP resetado");
    }
    
    /// Habilita/desabilita o SVP
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if enabled {
            info!("SVP habilitado");
            // Inicializa com valores padrão
            self.regs[0] = 0x8000; // Status register
        } else {
            info!("SVP desabilitado");
        }
    }
    
    /// Processa um ciclo do SVP
    pub fn tick(&mut self) {
        if !self.enabled || !self.running {
            return;
        }
        
        // Executa um ciclo do processador RISC
        self.processor.execute_cycle();
        
        // Processa DMA se ativo
        if self.dma.is_active() {
            self.dma.transfer_cycle(&mut self.dram, &mut self.texram);
        }
        
        // Verifica se há renderização pendente
        if self.processor.render_command_ready() {
            self.process_render_command();
        }
        
        // Verifica IRQ
        if self.processor.irq_asserted() {
            self.irq_pending = true;
            // Sinaliza IRQ para a CPU 68000
            self.signal_irq();
        }
    }
    
    /// Lê um word (16-bit) do espaço de endereçamento do SVP
    pub fn read_word(&self, addr: u32) -> u16 {
        let svp_addr = addr & 0x00FFFFFF;
        
        match svp_addr {
            // DRAM interna (0x000000-0x003FFF)
            0x000000..=0x003FFF => {
                let index = (svp_addr >> 1) as usize;
                if index < self.dram.len() {
                    self.dram[index]
                } else {
                    0x0000
                }
            }
            
            // Texture RAM (0x004000-0x023FFF)
            0x004000..=0x023FFF => {
                let index = svp_addr as usize;
                if index < self.texram.len() {
                    // Combina dois bytes em um word
                    let byte1 = self.texram[index] as u16;
                    let byte2 = self.texram[index + 1] as u16;
                    (byte2 << 8) | byte1
                } else {
                    0x0000
                }
            }
            
            // Registradores do SVP (0x030000-0x03001F)
            0x030000..=0x03001F => {
                let reg_index = ((svp_addr - 0x030000) >> 1) as usize;
                if reg_index < self.regs.len() {
                    self.regs[reg_index]
                } else {
                    0x0000
                }
            }
            
            // Acesso à ROM do cartucho via SVP
            0x040000..=0x3FFFFF => {
                // O SVP pode acessar a ROM diretamente
                // Esta é uma implementação simplificada
                if let Some(bus) = &self.bus {
                    let bus = bus.lock().unwrap();
                    bus.read_word(addr)
                } else {
                    0x0000
                }
            }
            
            _ => {
                warn!("SVP: Leitura de endereço inválido: {:06X}", svp_addr);
                0x0000
            }
        }
    }
    
    /// Escreve um word (16-bit) no espaço de endereçamento do SVP
    pub fn write_word(&mut self, addr: u32, value: u16) {
        let svp_addr = addr & 0x00FFFFFF;
        
        match svp_addr {
            // DRAM interna
            0x000000..=0x003FFF => {
                let index = (svp_addr >> 1) as usize;
                if index < self.dram.len() {
                    self.dram[index] = value;
                    
                    // Se escrever no buffer de comandos, processa
                    if index >= 0x100 && index < 0x180 {
                        self.processor.write_command_buffer(value);
                    }
                }
            }
            
            // Texture RAM
            0x004000..=0x023FFF => {
                let index = svp_addr as usize;
                if index < self.texram.len() - 1 {
                    self.texram[index] = value as u8;
                    self.texram[index + 1] = (value >> 8) as u8;
                }
            }
            
            // Registradores do SVP
            0x030000..=0x03001F => {
                let reg_index = ((svp_addr - 0x030000) >> 1) as usize;
                if reg_index < self.regs.len() {
                    self.write_register(reg_index, value);
                }
            }
            
            _ => {
                warn!("SVP: Escrita em endereço inválido: {:06X} = {:04X}", 
                      svp_addr, value);
            }
        }
    }
    
    /// Processa escrita em registrador
    fn write_register(&mut self, reg: usize, value: u16) {
        self.regs[reg] = value;
        
        match reg {
            0 => { // Status/Control register
                let old_running = self.running;
                self.running = (value & 0x0001) != 0;
                
                if !old_running && self.running {
                    info!("SVP: Processador iniciado");
                    self.processor.start();
                } else if old_running && !self.running {
                    info!("SVP: Processador parado");
                    self.processor.stop();
                }
                
                // Bit de reset
                if (value & 0x8000) != 0 {
                    self.reset();
                    self.regs[0] = 0x8000; // Mantém bit de reset
                }
            }
            
            1 => { // DMA Source Address
                self.dma.set_source(value as u32);
            }
            
            2 => { // DMA Destination Address
                self.dma.set_destination(value as u32);
            }
            
            3 => { // DMA Length/Control
                self.dma.set_length(value);
                if (value & 0x8000) != 0 {
                    self.dma.start();
                }
            }
            
            4 => { // IRQ Control
                if (value & 0x0001) != 0 {
                    self.irq_pending = false;
                    self.processor.clear_irq();
                }
            }
            
            _ => {
                trace!("SVP: Registrador {:X} = {:04X}", reg, value);
            }
        }
    }

    // Método para obter o framebuffer:
    pub fn get_frame_buffer(&self) -> &[u16] {
        self.renderer.get_frame_buffer()
    }
    
    /// Processa comando de renderização
    fn process_render_command(&mut self) {
        if let Some(cmd) = self.processor.get_render_command() {
            trace!("SVP: Processando comando de renderização: {:?}", cmd);
            
            // Cria vértices a partir do comando
            let vertices: Vec<renderer::Vertex> = cmd.vertices.iter()
                .map(|&(x, y)| {
                    renderer::Vertex {
                        x: x as f32,
                        y: y as f32,
                        z: 0.0, // Preencha com valores apropriados
                        w: 1.0,
                        u: 0.0, // Preencha com coordenadas de textura
                        v: 0.0,
                        color: cmd.color,
                        intensity: 1.0,
                    }
                })
                .collect();
            
            match cmd.cmd_type {
                processor::RenderCmdType::DrawPolygon => {
                    self.renderer.draw_polygon(
                        &vertices,
                        &self.texture_unit,
                        cmd.texture_id,
                        cmd.color
                    );
                }
                processor::RenderCmdType::DrawLine => {
                    if vertices.len() >= 2 {
                        self.renderer.draw_line(
                            vertices[0],
                            vertices[1],
                            cmd.color
                        );
                    }
                }
                processor::RenderCmdType::ClearFrame => {
                    self.renderer.clear();
                }
                _ => {}
            }
            
            self.processor.command_completed();
        }
    }
    
    /// Sinaliza IRQ para a CPU 68000
    fn signal_irq(&self) {
        if let Some(bus) = &self.bus {
            let mut bus = bus.lock().unwrap();
            // Implementar sinalização de IRQ
            // Normalmente através de um registrador no barramento
        }
    }
    
    /// Retorna o framebuffer renderizado
    pub fn get_frame_buffer(&self) -> &[u16] {
        self.renderer.get_frame_buffer()
    }
    
    /// Retorna se há um IRQ pendente
    pub fn irq_pending(&self) -> bool {
        self.irq_pending
    }
}

/// Trait para chips do cartucho
impl crate::core::cartridge::chips::CartridgeChip for SVP {
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
        self.write_word(addr, value)
    }
    
    fn reset(&mut self) {
        self.reset();
    }
    
    fn tick(&mut self, cycles: u32) {
        for _ in 0..cycles {
            self.tick();
        }
    }
    
    fn get_type(&self) -> crate::core::cartridge::chips::ChipType {
        crate::core::cartridge::chips::ChipType::SVP
    }
}