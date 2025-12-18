//! Unidade de Textura do SVP (Sega Virtua Processor)
//! Gerencia texturas para renderização 3D no Virtua Racing
//! Baseado em observações do código original e documentação do SVP

use crate::core::cartridge::chips::svp::dma::SVPDmaController;
use log::{trace, warn, debug};
use std::sync::{Arc, Mutex};

/// Formato de textura suportado pelo SVP
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFormat {
    /// 4-bit indexed (16 colors)
    Indexed4,
    /// 8-bit indexed (256 colors)
    Indexed8,
    /// 15-bit RGB (5-5-5)
    RGB555,
    /// 16-bit RGB (5-6-5) - comum no 32X
    RGB565,
}

/// Modo de filtragem de textura
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Nearest,    // Filtro ponto a ponto
    Bilinear,   // Interpolação bilinear
}

/// Modo de repetição de textura
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureWrap {
    Repeat,     // Repete a textura
    Clamp,      // Limita às bordas
    Mirror,     // Espelha a textura
}

/// Estrutura que representa uma textura individual
pub struct Texture {
    pub id: u8,                 // ID da textura (0-15)
    pub width: u16,             // Largura em pixels
    pub height: u16,            // Altura em pixels
    pub format: TextureFormat,  // Formato de pixel
    pub data: Vec<u16>,         // Dados da textura (sempre 16-bit para uniformidade)
    pub base_addr: u32,         // Endereço base na Texture RAM
    pub palette: Vec<u16>,      // Paleta de cores (para texturas indexadas)
    pub mip_levels: u8,         // Níveis de mipmap
    pub filter: TextureFilter,  // Filtro aplicado
    pub wrap_u: TextureWrap,    // Modo de repetição horizontal
    pub wrap_v: TextureWrap,    // Modo de repetição vertical
}

/// Cache de textura para acesso rápido
pub struct TextureCache {
    textures: [Option<Texture>; 16],  // Cache para até 16 texturas
    lru_counter: u32,                 // Contador LRU para substituição
    lru_timestamps: [u32; 16],        // Timestamps LRU
    hit_count: u32,                   // Estatísticas de cache hit
    miss_count: u32,                  // Estatísticas de cache miss
}

/// Unidade de Textura principal do SVP
pub struct TextureUnit {
    cache: TextureCache,                   // Cache de texturas
    texram: Option<Arc<Mutex<[u8; 131072]>>>, // Referência à Texture RAM (128KB)
    dma: Option<Arc<Mutex<SVPDmaController>>>, // Referência ao controlador DMA
    current_texture: Option<u8>,          // Textura atualmente vinculada
    active_palette: [u16; 256],           // Paleta ativa (256 cores max)
    bilinear_enabled: bool,               // Filtro bilinear habilitado
    mipmapping_enabled: bool,             // Mipmapping habilitado
    anisotropic_level: u8,                // Nível de anisotropia (1-16)
    stat_reg: u16,                        // Registrador de status
    control_reg: u16,                     // Registrador de controle
}

impl Texture {
    /// Cria uma nova textura
    pub fn new(id: u8, width: u16, height: u16, format: TextureFormat, base_addr: u32) -> Self {
        let pixel_count = width as usize * height as usize;
        
        Self {
            id,
            width,
            height,
            format,
            data: vec![0; pixel_count],
            base_addr,
            palette: match format {
                TextureFormat::Indexed4 => vec![0; 16],
                TextureFormat::Indexed8 => vec![0; 256],
                _ => vec![],
            },
            mip_levels: 1,
            filter: TextureFilter::Nearest,
            wrap_u: TextureWrap::Repeat,
            wrap_v: TextureWrap::Repeat,
        }
    }
    
    /// Calcula o tamanho da textura em bytes
    pub fn size_bytes(&self) -> usize {
        let pixel_count = self.width as usize * self.height as usize;
        
        match self.format {
            TextureFormat::Indexed4 => pixel_count / 2,  // 4 bits por pixel
            TextureFormat::Indexed8 => pixel_count,      // 8 bits por pixel
            TextureFormat::RGB555 => pixel_count * 2,    // 16 bits por pixel
            TextureFormat::RGB565 => pixel_count * 2,    // 16 bits por pixel
        }
    }
    
    /// Converte coordenadas UV para coordenadas de textura, aplicando wrapping
    pub fn uv_to_texcoord(&self, u: f32, v: f32) -> (u16, u16) {
        let (u_wrapped, v_wrapped) = self.apply_wrapping(u, v);
        
        let x = (u_wrapped * self.width as f32) as u16;
        let y = (v_wrapped * self.height as f32) as u16;
        
        (x.min(self.width - 1), y.min(self.height - 1))
    }
    
    /// Aplica o modo de wrapping às coordenadas UV
    fn apply_wrapping(&self, u: f32, v: f32) -> (f32, f32) {
        let u_frac = u.fract();
        let v_frac = v.fract();
        
        let u_wrapped = match self.wrap_u {
            TextureWrap::Repeat => {
                if u_frac >= 0.0 { u_frac } else { 1.0 + u_frac }
            }
            TextureWrap::Clamp => u.clamp(0.0, 1.0),
            TextureWrap::Mirror => {
                let abs_u = u_frac.abs();
                if (u_frac.floor() as i32) % 2 == 0 { abs_u } else { 1.0 - abs_u }
            }
        };
        
        let v_wrapped = match self.wrap_v {
            TextureWrap::Repeat => {
                if v_frac >= 0.0 { v_frac } else { 1.0 + v_frac }
            }
            TextureWrap::Clamp => v.clamp(0.0, 1.0),
            TextureWrap::Mirror => {
                let abs_v = v_frac.abs();
                if (v_frac.floor() as i32) % 2 == 0 { abs_v } else { 1.0 - abs_v }
            }
        };
        
        (u_wrapped, v_wrapped)
    }
    
    /// Amostra um pixel da textura com filtro de ponto
    pub fn sample_nearest(&self, u: f32, v: f32) -> u16 {
        let (x, y) = self.uv_to_texcoord(u, v);
        self.get_pixel(x, y)
    }
    
    /// Amostra um pixel da textura com filtro bilinear
    pub fn sample_bilinear(&self, u: f32, v: f32) -> u16 {
        let tex_x = u * self.width as f32 - 0.5;
        let tex_y = v * self.height as f32 - 0.5;
        
        let x0 = tex_x.floor() as i32;
        let y0 = tex_y.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;
        
        // Peso das amostras
        let fx = tex_x.fract() as f32;
        let fy = tex_y.fract() as f32;
        let fx1 = 1.0 - fx;
        let fy1 = 1.0 - fy;
        
        // Amostra os 4 pixels vizinhos
        let p00 = self.get_pixel_wrapped(x0, y0);
        let p10 = self.get_pixel_wrapped(x1, y0);
        let p01 = self.get_pixel_wrapped(x0, y1);
        let p11 = self.get_pixel_wrapped(x1, y1);
        
        // Interpolação bilinear
        let top = self.interpolate_color(p00, p10, fx);
        let bottom = self.interpolate_color(p01, p11, fx);
        self.interpolate_color_u16(top, bottom, fy)
    }
    
    /// Obtém um pixel com wrapping aplicado
    fn get_pixel_wrapped(&self, x: i32, y: i32) -> u16 {
        let wrapped_x = self.wrap_coordinate(x, self.width);
        let wrapped_y = self.wrap_coordinate(y, self.height);
        self.get_pixel(wrapped_x, wrapped_y)
    }
    
    /// Aplica wrapping a uma coordenada
    fn wrap_coordinate(&self, coord: i32, max: u16) -> u16 {
        let max_i = max as i32;
        let wrapped = match self.wrap_u {
            TextureWrap::Repeat => ((coord % max_i) + max_i) % max_i,
            TextureWrap::Clamp => coord.clamp(0, max_i - 1),
            TextureWrap::Mirror => {
                let period = max_i * 2;
                let pos = ((coord % period) + period) % period;
                if pos < max_i { pos } else { period - pos - 1 }
            }
        };
        wrapped as u16
    }
    
    /// Obtém um pixel específico
    pub fn get_pixel(&self, x: u16, y: u16) -> u16 {
        let idx = y as usize * self.width as usize + x as usize;
        if idx < self.data.len() {
            self.data[idx]
        } else {
            0x0000
        }
    }
    
    /// Define um pixel específico
    pub fn set_pixel(&mut self, x: u16, y: u16, color: u16) {
        let idx = y as usize * self.width as usize + x as usize;
        if idx < self.data.len() {
            self.data[idx] = color;
        }
    }
    
    /// Carrega dados da Texture RAM para esta textura
    pub fn load_from_texram(&mut self, texram: &[u8]) -> bool {
        let start = self.base_addr as usize;
        let size = self.size_bytes();
        let end = start + size;
        
        if end <= texram.len() {
            match self.format {
                TextureFormat::Indexed4 => {
                    // Cada byte contém 2 pixels (4 bits cada)
                    for i in 0..size {
                        let byte = texram[start + i];
                        let idx = i * 2;
                        
                        if idx < self.data.len() {
                            self.data[idx] = (byte & 0x0F) as u16;
                        }
                        if idx + 1 < self.data.len() {
                            self.data[idx + 1] = ((byte >> 4) & 0x0F) as u16;
                        }
                    }
                }
                TextureFormat::Indexed8 => {
                    // Cada byte é um índice de paleta
                    for i in 0..size {
                        if i < self.data.len() {
                            self.data[i] = texram[start + i] as u16;
                        }
                    }
                }
                TextureFormat::RGB555 | TextureFormat::RGB565 => {
                    // Dois bytes por pixel
                    for i in 0..(size / 2) {
                        let byte_idx = start + i * 2;
                        if i < self.data.len() {
                            let low = texram[byte_idx] as u16;
                            let high = texram[byte_idx + 1] as u16;
                            self.data[i] = (high << 8) | low;
                        }
                    }
                }
            }
            true
        } else {
            warn!("Texture {}: endereço fora dos limites da Texture RAM", self.id);
            false
        }
    }
    
    /// Salva dados da textura de volta para a Texture RAM
    pub fn save_to_texram(&self, texram: &mut [u8]) -> bool {
        let start = self.base_addr as usize;
        let size = self.size_bytes();
        let end = start + size;
        
        if end <= texram.len() {
            match self.format {
                TextureFormat::Indexed4 => {
                    for i in 0..size {
                        let idx = i * 2;
                        let mut byte = 0;
                        
                        if idx < self.data.len() {
                            byte |= (self.data[idx] & 0x0F) as u8;
                        }
                        if idx + 1 < self.data.len() {
                            byte |= ((self.data[idx + 1] & 0x0F) as u8) << 4;
                        }
                        
                        texram[start + i] = byte;
                    }
                }
                TextureFormat::Indexed8 => {
                    for i in 0..size {
                        if i < self.data.len() {
                            texram[start + i] = self.data[i] as u8;
                        }
                    }
                }
                TextureFormat::RGB555 | TextureFormat::RGB565 => {
                    for i in 0..(size / 2) {
                        let byte_idx = start + i * 2;
                        if i < self.data.len() {
                            let pixel = self.data[i];
                            texram[byte_idx] = pixel as u8;
                            texram[byte_idx + 1] = (pixel >> 8) as u8;
                        }
                    }
                }
            }
            true
        } else {
            false
        }
    }
    
    /// Interpola entre duas cores (helper para bilinear)
    fn interpolate_color(&self, c1: u16, c2: u16, t: f32) -> u16 {
        if self.format == TextureFormat::RGB555 || self.format == TextureFormat::RGB565 {
            self.interpolate_color_u16(c1, c2, t)
        } else {
            // Para texturas indexadas, interpolamos os índices
            let idx1 = c1 as f32;
            let idx2 = c2 as f32;
            ((idx1 * (1.0 - t) + idx2 * t) as u16).min(255)
        }
    }
    
    /// Interpola entre duas cores RGB
    fn interpolate_color_u16(&self, c1: u16, c2: u16, t: f32) -> u16 {
        let r1 = ((c1 >> 10) & 0x1F) as f32;
        let g1 = ((c1 >> 5) & 0x1F) as f32;
        let b1 = (c1 & 0x1F) as f32;
        
        let r2 = ((c2 >> 10) & 0x1F) as f32;
        let g2 = ((c2 >> 5) & 0x1F) as f32;
        let b2 = (c2 & 0x1F) as f32;
        
        let r = (r1 * (1.0 - t) + r2 * t) as u16;
        let g = (g1 * (1.0 - t) + g2 * t) as u16;
        let b = (b1 * (1.0 - t) + b2 * t) as u16;
        
        (r << 10) | (g << 5) | b
    }
}

impl TextureCache {
    /// Cria um novo cache de textura
    pub fn new() -> Self {
        Self {
            textures: [const { None }; 16],
            lru_counter: 0,
            lru_timestamps: [0; 16],
            hit_count: 0,
            miss_count: 0,
        }
    }
    
    /// Obtém uma textura do cache
    pub fn get(&mut self, id: u8) -> Option<&Texture> {
        if id < 16 {
            if self.textures[id as usize].is_some() {
                self.hit_count += 1;
                self.lru_timestamps[id as usize] = self.lru_counter;
                self.lru_counter += 1;
                self.textures[id as usize].as_ref()
            } else {
                self.miss_count += 1;
                None
            }
        } else {
            None
        }
    }
    
    /// Obtém uma textura mutável do cache
    pub fn get_mut(&mut self, id: u8) -> Option<&mut Texture> {
        if id < 16 {
            if self.textures[id as usize].is_some() {
                self.lru_timestamps[id as usize] = self.lru_counter;
                self.lru_counter += 1;
                self.textures[id as usize].as_mut()
            } else {
                None
            }
        } else {
            None
        }
    }
    
    /// Adiciona uma textura ao cache
    pub fn insert(&mut self, texture: Texture) -> bool {
        let id = texture.id;
        if id < 16 {
            self.textures[id as usize] = Some(texture);
            self.lru_timestamps[id as usize] = self.lru_counter;
            self.lru_counter += 1;
            true
        } else {
            false
        }
    }
    
    /// Remove uma textura do cache
    pub fn remove(&mut self, id: u8) -> Option<Texture> {
        if id < 16 {
            self.textures[id as usize].take()
        } else {
            None
        }
    }
    
    /// Encontra o slot LRU para substituição
    fn find_lru_slot(&self) -> Option<usize> {
        if self.textures.iter().any(|t| t.is_none()) {
            // Temos slots vazios
            self.textures.iter().position(|t| t.is_none())
        } else {
            // Todos slots ocupados, encontra o LRU
            self.lru_timestamps
                .iter()
                .enumerate()
                .min_by_key(|&(_, &ts)| ts)
                .map(|(idx, _)| idx)
        }
    }
    
    /// Limpa o cache
    pub fn clear(&mut self) {
        for i in 0..16 {
            self.textures[i] = None;
            self.lru_timestamps[i] = 0;
        }
        self.lru_counter = 0;
        self.hit_count = 0;
        self.miss_count = 0;
    }
    
    /// Retorna estatísticas do cache
    pub fn stats(&self) -> (u32, u32, f32) {
        let total = self.hit_count + self.miss_count;
        let hit_rate = if total > 0 {
            self.hit_count as f32 / total as f32
        } else {
            0.0
        };
        (self.hit_count, self.miss_count, hit_rate)
    }
}

impl TextureUnit {
    /// Cria uma nova unidade de textura
    pub fn new() -> Self {
        Self {
            cache: TextureCache::new(),
            texram: None,
            dma: None,
            current_texture: None,
            active_palette: [0; 256],
            bilinear_enabled: false,
            mipmapping_enabled: false,
            anisotropic_level: 1,
            stat_reg: 0,
            control_reg: 0,
        }
    }
    
    /// Reseta a unidade de textura
    pub fn reset(&mut self) {
        self.cache.clear();
        self.texram = None;
        self.dma = None;
        self.current_texture = None;
        self.active_palette.fill(0);
        self.bilinear_enabled = false;
        self.mipmapping_enabled = false;
        self.anisotropic_level = 1;
        self.stat_reg = 0;
        self.control_reg = 0;
    }
    
    /// Conecta à Texture RAM
    pub fn connect_texram(&mut self, texram: Arc<Mutex<[u8; 131072]>>) {
        self.texram = Some(texram);
    }
    
    /// Conecta ao controlador DMA
    pub fn connect_dma(&mut self, dma: Arc<Mutex<SVPDmaController>>) {
        self.dma = Some(dma);
    }
    
    /// Vincula uma textura para renderização
    pub fn bind_texture(&mut self, texture_id: u8) -> bool {
        if texture_id < 16 {
            self.current_texture = Some(texture_id);
            
            // Se a textura não está no cache, tenta carregá-la
            if self.cache.get(texture_id).is_none() {
                self.load_texture(texture_id);
            }
            
            true
        } else {
            false
        }
    }
    
    /// Carrega uma textura no cache
    pub fn load_texture(&mut self, texture_id: u8) -> bool {
        if let Some(texram) = &self.texram {
            let texram_lock = texram.lock().unwrap();
            
            // Tenta detectar o formato e tamanho da textura
            // (No SVP real, isso provavelmente é controlado por registradores)
            
            // Assumimos texturas 64x64 RGB555 por padrão para Virtua Racing
            let width = 64;
            let height = 64;
            let format = TextureFormat::RGB555;
            let base_addr = (texture_id as u32) * 0x2000; // 8KB por textura
            
            let mut texture = Texture::new(texture_id, width, height, format, base_addr);
            
            if texture.load_from_texram(&texram_lock) {
                self.cache.insert(texture);
                debug!("Textura {} carregada: {}x{} {:?}", 
                       texture_id, width, height, format);
                true
            } else {
                warn!("Falha ao carregar textura {}", texture_id);
                false
            }
        } else {
            false
        }
    }
    
    /// Desvincula a textura atual
    pub fn unbind_texture(&mut self) {
        self.current_texture = None;
    }
    
    /// Amostra a textura atual nas coordenadas UV
    pub fn sample(&self, u: f32, v: f32, lod: f32) -> u16 {
        if let Some(texture_id) = self.current_texture {
            if let Some(texture) = self.cache.get(texture_id) {
                if self.bilinear_enabled && texture.filter == TextureFilter::Bilinear {
                    texture.sample_bilinear(u, v)
                } else {
                    texture.sample_nearest(u, v)
                }
            } else {
                // Textura não encontrada, retorna cor padrão
                0x7C00 // Vermelho em RGB555 (para debug)
            }
        } else {
            0x0000 // Preto
        }
    }
    
    /// Define uma cor na paleta ativa
    pub fn set_palette_color(&mut self, index: u8, color: u16) {
        if index < 256 {
            self.active_palette[index as usize] = color;
        }
    }
    
    /// Obtém uma cor da paleta ativa
    pub fn get_palette_color(&self, index: u8) -> u16 {
        if index < 256 {
            self.active_palette[index as usize]
        } else {
            0x0000
        }
    }
    
    /// Carrega a paleta da Texture RAM
    pub fn load_palette(&mut self, base_addr: u32) -> bool {
        if let Some(texram) = &self.texram {
            let texram_lock = texram.lock().unwrap();
            let start = base_addr as usize;
            let end = start + 512; // 256 cores * 2 bytes
            
            if end <= texram_lock.len() {
                for i in 0..256 {
                    let idx = start + i * 2;
                    let low = texram_lock[idx] as u16;
                    let high = texram_lock[idx + 1] as u16;
                    self.active_palette[i] = (high << 8) | low;
                }
                true
            } else {
                false
            }
        } else {
            false
        }
    }
    
    /// Processa uma transferência DMA para a Texture RAM
    pub fn process_dma_transfer(&mut self) {
        if let (Some(dma), Some(texram)) = (&self.dma, &self.texram) {
            let mut dma_lock = dma.lock().unwrap();
            let mut texram_lock = texram.lock().unwrap();
            
            if dma_lock.is_active() {
                // Implementação simplificada de transferência DMA
                // Na realidade, o SVP tem DMA dedicado para texturas
                trace!("TextureUnit: Processando transferência DMA");
                
                // Marca todas as texturas afetadas como inválidas no cache
                self.invalidate_affected_textures(&dma_lock);
            }
        }
    }
    
    /// Invalida texturas no cache que foram afetadas por DMA
    fn invalidate_affected_textures(&mut self, dma: &SVPDmaController) {
        // Verifica quais texturas no cache estão na região do DMA
        for i in 0..16 {
            if let Some(texture) = self.cache.get_mut(i) {
                let tex_start = texture.base_addr;
                let tex_end = tex_start + texture.size_bytes() as u32;
                
                let dma_start = dma.source_address();
                let dma_end = dma_start + dma.length() as u32;
                
                // Se há sobreposição, remove do cache
                if dma_start < tex_end && dma_end > tex_start {
                    self.cache.remove(i);
                    trace!("TextureUnit: Textura {} invalidada por DMA", i);
                }
            }
        }
    }
    
    /// Habilita/desabilita filtro bilinear
    pub fn set_bilinear_filter(&mut self, enabled: bool) {
        self.bilinear_enabled = enabled;
    }
    
    /// Habilita/desabilita mipmapping
    pub fn set_mipmapping(&mut self, enabled: bool) {
        self.mipmapping_enabled = enabled;
    }
    
    /// Define o nível de anisotropia
    pub fn set_anisotropy(&mut self, level: u8) {
        self.anisotropic_level = level.clamp(1, 16);
    }
    
    /// Escreve em um registrador da unidade de textura
    pub fn write_register(&mut self, reg: u8, value: u16) {
        match reg {
            0 => { // Control register
                self.control_reg = value;
                self.bilinear_enabled = (value & 0x0001) != 0;
                self.mipmapping_enabled = (value & 0x0002) != 0;
                let anisotropy = ((value >> 2) & 0x000F) as u8;
                self.set_anisotropy(anisotropy.max(1));
            }
            1 => { // Palette base address
                let base_addr = value as u32 * 0x200;
                self.load_palette(base_addr);
            }
            2 => { // Current texture ID
                let texture_id = (value & 0x000F) as u8;
                self.bind_texture(texture_id);
            }
            _ => {
                trace!("TextureUnit: Registrador {:X} = {:04X}", reg, value);
            }
        }
    }
    
    /// Lê de um registrador da unidade de textura
    pub fn read_register(&self, reg: u8) -> u16 {
        match reg {
            0 => self.stat_reg,
            1 => self.control_reg,
            _ => 0x0000,
        }
    }
    
    /// Retorna estatísticas de performance
    pub fn get_stats(&self) -> (u32, u32, f32) {
        self.cache.stats()
    }
}