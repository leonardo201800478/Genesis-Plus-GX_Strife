// genesis-plus-rs/src/core/cartridge/chips/paprium/interface.rs

//! Interface do chip Paprium com o sistema Genesis
//! Mapeamento de memória e integração

use std::sync::{Arc, Mutex};
use super::minimp3::{PapriumMp3System, Mp3Decoder};
use super::{PapriumAsic, PAPRIUM_BOSS1, PAPRIUM_BOSS2, PAPRIUM_BOSS3, PAPRIUM_BOSS4};
use crate::core::memory::MemoryBus;
use crate::core::snd::Sound;
use crate::core::vdp::Vdp;
use log::{info, warn, debug, trace};

/// Interface de memória do Paprium
pub struct PapriumMemoryInterface {
    /// Chip ASIC principal
    pub asic: PapriumAsic,
    
    /// Sistema MP3
    pub mp3_system: PapriumMp3System,
    
    /// Estado da TMSS
    pub tmss_enabled: bool,
    
    /// Callbacks de acesso
    pub read_callback: Option<Box<dyn Fn(u32) -> u8>>,
    pub write_callback: Option<Box<dyn Fn(u32, u8)>>,
}

impl PapriumMemoryInterface {
    /// Cria nova interface
    pub fn new(asic: PapriumAsic, mp3_dir: std::path::PathBuf) -> Self {
        let mp3_system = PapriumMp3System::new(mp3_dir);
        
        Self {
            asic,
            mp3_system,
            tmss_enabled: true,
            read_callback: None,
            write_callback: None,
        }
    }
    
    /// Inicializa a interface
    pub fn init(&mut self) {
        info!("Inicializando interface de memória do Paprium");
        
        // Inicializa ASIC
        self.asic.init().unwrap_or_else(|e| {
            warn!("Erro ao inicializar ASIC: {:?}", e);
        });
        
        // Carrega faixas MP3
        if let Err(e) = self.mp3_system.load_boss_tracks() {
            warn!("Erro ao carregar faixas MP3: {}", e);
        }
        
        // Configura callbacks
        self.setup_callbacks();
    }
    
    /// Configura callbacks de memória
    fn setup_callbacks(&mut self) {
        // Callback de leitura
        let asic_ref = self.asic.clone_state();
        let mp3_ref = self.mp3_system.clone_state();
        
        self.read_callback = Some(Box::new(move |addr| {
            Self::handle_read(addr, &asic_ref, &mp3_ref)
        }));
        
        // Callback de escrita
        let mut asic_mut = self.asic.clone_state_mut();
        let mut mp3_mut = self.mp3_system.clone_state_mut();
        
        self.write_callback = Some(Box::new(move |addr, value| {
            Self::handle_write(addr, value, &mut asic_mut, &mut mp3_mut)
        }));
    }
    
    /// Manipula leitura de memória
    fn handle_read(addr: u32, asic: &PapriumAsic, mp3_system: &PapriumMp3System) -> u8 {
        // Verifica se é acesso à área do Paprium
        if Self::is_paprium_area(addr) {
            return asic.read_byte(addr & 0xFFFF);
        }
        
        // Verifica I/O do Paprium
        if Self::is_paprium_io(addr) {
            return Self::handle_io_read(addr, asic, mp3_system);
        }
        
        // Outros acessos (delegar para sistema normal)
        0xFF
    }
    
    /// Manipula escrita de memória
    fn handle_write(addr: u32, value: u8, asic: &mut PapriumAsic, mp3_system: &mut PapriumMp3System) {
        // Verifica se é acesso à área do Paprium
        if Self::is_paprium_area(addr) {
            asic.write_byte(addr & 0xFFFF, value);
            return;
        }
        
        // Verifica I/O do Paprium
        if Self::is_paprium_io(addr) {
            Self::handle_io_write(addr, value, asic, mp3_system);
            return;
        }
        
        // Verifica TMSS
        if addr == 0xA14101 {
            asic.tmss = value != 0;
        }
    }
    
    /// Verifica se endereço está na área do Paprium
    fn is_paprium_area(addr: u32) -> bool {
        // Paprium usa parte da área de memória do Genesis
        matches!(addr, 
            0x000000..=0x00FFFF |   // RAM do Paprium
            0xC00000..=0xC0FFFF     // Decoder buffer
        )
    }
    
    /// Verifica se é I/O do Paprium
    fn is_paprium_io(addr: u32) -> bool {
        matches!(addr,
            0xA130F0..=0xA130FF |   // Bank switching (desabilitado)
            0xA14101                // TMSS
        )
    }
    
    /// Manipula leitura de I/O
    fn handle_io_read(addr: u32, asic: &PapriumAsic, mp3_system: &PapriumMp3System) -> u8 {
        match addr {
            // TMSS
            0xA14101 => asic.tmss as u8,
            
            // Outros registros I/O
            _ => 0xFF,
        }
    }
    
    /// Manipula escrita de I/O
    fn handle_io_write(addr: u32, value: u8, asic: &mut PapriumAsic, mp3_system: &mut PapriumMp3System) {
        match addr {
            // TMSS
            0xA14101 => {
                asic.tmss = value != 0;
                debug!("TMSS configurado: {}", asic.tmss);
            }
            
            // Bank switching (desabilitado no Paprium)
            0xA130F0..=0xA130FF => {
                // Ignorado - Paprium tem seu próprio mapeamento
            }
            
            _ => {
                trace!("Escrita I/O desconhecida: {:08X} = {:02X}", addr, value);
            }
        }
    }
    
    /// Processa áudio
    pub fn process_audio(&mut self, cycles: u32, sound: &mut Sound) {
        // Atualiza track MP3 se necessário
        if self.mp3_system.track_changed() {
            self.mp3_system.update_last_track();
        }
        
        // Processa áudio do ASIC
        self.asic.update_audio(cycles, sound);
        
        // Processa áudio MP3 se houver
        if self.asic.music_track != 0 {
            self.process_mp3_audio(cycles, sound);
        }
    }
    
    /// Processa áudio MP3
    fn process_mp3_audio(&mut self, cycles: u32, sound: &mut Sound) {
        // Obtém ponteiro atual
        let ptr = self.asic.mp3_ptr as usize;
        
        // Obtém amostra
        let (l, r) = self.mp3_system.get_sample(ptr);
        
        // Aplica volume
        let volume = self.asic.music_volume;
        let l_scaled = (l * volume) / 256;
        let r_scaled = (r * volume) / 256;
        
        // Adiciona ao buffer de áudio
        if let Some(blip) = sound.blips.get_mut(3) {
            blip.add_delta_fast(0, l_scaled as i16, r_scaled as i16);
        }
        
        // Atualiza ponteiro
        self.asic.music_tick += 0x10000;
        if self.asic.music_tick >= 0x10000 {
            self.asic.music_tick -= 0x10000;
            self.asic.mp3_ptr = self.asic.mp3_ptr.wrapping_add(2);
            
            // Verifica loop
            self.check_mp3_loop();
        }
    }
    
    /// Verifica loop do MP3
    fn check_mp3_loop(&mut self) {
        let track = self.asic.music_track;
        let ptr = self.asic.mp3_ptr as usize;
        
        let info = match track {
            PAPRIUM_BOSS1 => &self.mp3_system.info_boss1,
            PAPRIUM_BOSS2 => &self.mp3_system.info_boss2,
            PAPRIUM_BOSS3 => &self.mp3_system.info_boss3,
            PAPRIUM_BOSS4 => &self.mp3_system.info_boss4,
            _ => &self.mp3_system.info,
        };
        
        if let Some(info) = info {
            if ptr >= info.samples {
                self.asic.mp3_ptr = 0;
                debug!("Loop MP3 track {}", track);
            }
        }
    }
    
    /// Carrega track MP3
    pub fn load_mp3_track(&mut self, track: i32, reload: bool) {
        self.mp3_system.load_track(track, reload);
        self.asic.music_track = track;
        self.asic.mp3_ptr = 0;
        self.asic.music_tick = 0;
        
        debug!("Track MP3 carregada: {:02X}", track);
    }
    
    /// Reseta a interface
    pub fn reset(&mut self) {
        info!("Resetando interface Paprium");
        
        self.asic.reset();
        self.mp3_system.reset();
        self.tmss_enabled = true;
    }
    
    /// Conecta ao barramento
    pub fn connect_bus(&mut self, bus: Arc<Mutex<MemoryBus>>) {
        self.asic.connect_bus(bus);
    }
    
    /// Conecta ao VDP
    pub fn connect_vdp(&mut self, vdp: Arc<Mutex<Vdp>>) {
        self.asic.connect_vdp(vdp);
    }
    
    /// Conecta ao som
    pub fn connect_sound(&mut self, sound: Arc<Mutex<Sound>>) {
        self.asic.connect_sound(sound);
    }
    
    /// Clona estado (para callbacks)
    fn clone_state(&self) -> PapriumAsic {
        // Implementação simplificada de clone
        // Na prática, seria necessário serializar/deserializar
        let mut clone = PapriumAsic::new(
            self.asic.rom_data.clone(),
            self.asic.rom_dir.clone()
        );
        
        // Copia estado importante
        clone.enabled = self.asic.enabled;
        clone.music_track = self.asic.music_track;
        clone.mp3_ptr = self.asic.mp3_ptr;
        clone.music_volume = self.asic.music_volume;
        clone.sfx_volume = self.asic.sfx_volume;
        
        clone
    }
    
    /// Clona estado mutável (para callbacks)
    fn clone_state_mut(&mut self) -> PapriumAsic {
        self.clone_state()
    }
}

// Clone para Mp3System
impl PapriumMp3System {
    fn clone_state(&self) -> Self {
        let mut clone = PapriumMp3System::new(self.mp3_dir.clone());
        clone.current_track = self.current_track;
        clone.last_track = self.last_track;
        clone
    }
    
    fn clone_state_mut(&mut self) -> Self {
        self.clone_state()
    }
}

/// Wrapper do Paprium para integração com o sistema
pub struct PapriumChip {
    /// Interface principal
    pub interface: PapriumMemoryInterface,
    
    /// Enabled
    pub enabled: bool,
    
    /// Configuração
    pub config: PapriumConfig,
}

/// Configuração do chip Paprium
pub struct PapriumConfig {
    /// Ativar debug
    pub debug_mode: bool,
    
    /// Ativar cheats
    pub enable_cheats: bool,
    
    /// Hack de DMA rápido
    pub fast_dma_hack: bool,
    
    /// Diretório dos MP3
    pub mp3_dir: std::path::PathBuf,
    
    /// Caminho da ROM
    pub rom_path: std::path::PathBuf,
}

impl Default for PapriumConfig {
    fn default() -> Self {
        Self {
            debug_mode: false,
            enable_cheats: false,
            fast_dma_hack: true,
            mp3_dir: std::path::PathBuf::from("./paprium"),
            rom_path: std::path::PathBuf::from(""),
        }
    }
}

impl PapriumChip {
    /// Cria novo chip Paprium
    pub fn new(rom_data: Arc<Vec<u8>>, config: PapriumConfig) -> Self {
        info!("Criando chip Paprium");
        
        // Cria ASIC
        let asic = PapriumAsic::new(rom_data, config.rom_path.clone());
        
        // Cria interface
        let interface = PapriumMemoryInterface::new(asic, config.mp3_dir.clone());
        
        Self {
            interface,
            enabled: true,
            config,
        }
    }
    
    /// Inicializa o chip
    pub fn init(&mut self) -> Result<(), String> {
        info!("Inicializando chip Paprium");
        
        // Configura debug
        self.interface.asic.set_debug_mode(self.config.debug_mode);
        
        // Configura hacks
        self.interface.asic.fast_dma_hack = self.config.fast_dma_hack;
        
        // Inicializa interface
        self.interface.init();
        
        info!("Chip Paprium inicializado");
        Ok(())
    }
    
    /// Reseta o chip
    pub fn reset(&mut self) {
        info!("Resetando chip Paprium");
        self.interface.reset();
    }
    
    /// Processa um ciclo
    pub fn tick(&mut self, cycles: u32) {
        if !self.enabled {
            return;
        }
        
        // Atualiza ASIC
        self.interface.asic.update(cycles);
    }
    
    /// Processa áudio
    pub fn process_audio(&mut self, cycles: u32, sound: &mut Sound) {
        if !self.enabled {
            return;
        }
        
        self.interface.process_audio(cycles, sound);
    }
    
    /// Lê byte
    pub fn read_byte(&self, addr: u32) -> u8 {
        if !self.enabled {
            return 0xFF;
        }
        
        self.interface.asic.read_byte(addr)
    }
    
    /// Lê word
    pub fn read_word(&self, addr: u32) -> u16 {
        if !self.enabled {
            return 0xFFFF;
        }
        
        self.interface.asic.read_word(addr)
    }
    
    /// Escreve byte
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        if !self.enabled {
            return;
        }
        
        self.interface.asic.write_byte(addr, value);
    }
    
    /// Escreve word
    pub fn write_word(&mut self, addr: u32, value: u16) {
        if !self.enabled {
            return;
        }
        
        self.interface.asic.write_word(addr, value);
    }
    
    /// Habilita/desabilita
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        info!("Paprium chip {}", if enabled { "habilitado" } else { "desabilitado" });
    }
    
    /// Salva estado
    pub fn save_state(&self) -> Vec<u8> {
        self.interface.asic.save_state()
    }
    
    /// Carrega estado
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        self.interface.asic.load_state(data)
    }
}

// Implementação da trait CartridgeChip para PapriumChip
impl crate::core::cartridge::chips::CartridgeChip for PapriumChip {
    fn init(&mut self) {
        if let Err(e) = self.init() {
            error!("Falha ao inicializar Paprium: {}", e);
        }
    }
    
    fn reset(&mut self) {
        self.reset();
    }
    
    fn update(&mut self, cycles: u32) {
        self.tick(cycles);
    }
    
    fn save_state(&self) -> Vec<u8> {
        self.save_state()
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        self.load_state(data)
    }
    
    fn chip_type(&self) -> crate::core::cartridge::chips::ChipType {
        crate::core::cartridge::chips::ChipType::Paprium
    }
    
    fn read_byte(&self, addr: u32) -> u8 {
        self.read_byte(addr)
    }
    
    fn read_word(&self, addr: u32) -> u16 {
        self.read_word(addr)
    }
    
    fn write_byte(&mut self, addr: u32, value: u8) {
        self.write_byte(addr, value);
    }
    
    fn write_word(&mut self, addr: u32, value: u16) {
        self.write_word(addr, value);
    }
    
    fn update_audio(&mut self, samples: u32, sound: &mut Sound) {
        self.process_audio(samples, sound);
    }
}