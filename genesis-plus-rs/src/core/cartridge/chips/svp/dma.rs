//! Controlador DMA do SVP

pub struct SVPDmaController {
    src_addr: u32,
    dst_addr: u32,
    length: u16,
    active: bool,
    direction: DmaDirection, // 0: DRAM->TexRAM, 1: TexRAM->DRAM
}

#[derive(Debug, Clone, Copy)]
pub enum DmaDirection {
    DramToTexRam,
    TexRamToDram,
}

impl SVPDmaController {
    pub fn new() -> Self {
        Self {
            src_addr: 0,
            dst_addr: 0,
            length: 0,
            active: false,
            direction: DmaDirection::DramToTexRam,
        }
    }
    
    pub fn reset(&mut self) {
        self.src_addr = 0;
        self.dst_addr = 0;
        self.length = 0;
        self.active = false;
        self.direction = DmaDirection::DramToTexRam;
    }
    
    pub fn set_source(&mut self, addr: u32) {
        self.src_addr = addr & 0x00FFFFFF;
    }
    
    pub fn set_destination(&mut self, addr: u32) {
        self.dst_addr = addr & 0x00FFFFFF;
        // Determina direção baseada nos endereços
        self.direction = if self.dst_addr >= 0x004000 {
            DmaDirection::DramToTexRam
        } else {
            DmaDirection::TexRamToDram
        };
    }
    
    pub fn set_length(&mut self, length: u16) {
        self.length = length & 0x7FFF;
    }
    
    pub fn start(&mut self) {
        self.active = true;
    }
    
    pub fn is_active(&self) -> bool {
        self.active
    }
    
    /// Executa um ciclo de transferência DMA
    pub fn transfer_cycle(&mut self, dram: &mut [u16], texram: &mut [u8]) {
        if !self.active || self.length == 0 {
            return;
        }
        
        match self.direction {
            DmaDirection::DramToTexRam => {
                let src_idx = (self.src_addr >> 1) as usize;
                let dst_idx = self.dst_addr as usize;
                
                if src_idx < dram.len() && dst_idx < texram.len() - 1 {
                    let word = dram[src_idx];
                    texram[dst_idx] = word as u8;
                    texram[dst_idx + 1] = (word >> 8) as u8;
                }
                
                self.src_addr = self.src_addr.wrapping_add(2);
                self.dst_addr = self.dst_addr.wrapping_add(2);
            }
            
            DmaDirection::TexRamToDram => {
                let src_idx = self.src_addr as usize;
                let dst_idx = (self.dst_addr >> 1) as usize;
                
                if src_idx < texram.len() - 1 && dst_idx < dram.len() {
                    let word = (texram[src_idx + 1] as u16) << 8 | 
                               texram[src_idx] as u16;
                    dram[dst_idx] = word;
                }
                
                self.src_addr = self.src_addr.wrapping_add(2);
                self.dst_addr = self.dst_addr.wrapping_add(2);
            }
        }
        
        self.length -= 1;
        if self.length == 0 {
            self.active = false;
            // DMA completo - poderia gerar IRQ aqui
        }
    }
}