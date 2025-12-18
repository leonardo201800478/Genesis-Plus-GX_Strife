// genesis-plus-rs/src/core/cartridge/chips/paprium/mod.rs

//! Paprium ASIC - Custom chip for the Paprium game
//! Based on "Project Little Man" original C code

use crate::core::cartridge::types::{CartridgeError, CartridgeResult};
use crate::core::memory::MemoryBus;
use crate::core::snd::{Sound, Blip};
use crate::core::vdp::Vdp;
use log::{info, warn, debug, trace, error};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;

// Submódulos
pub mod audio;
pub mod decoder;
pub mod sprite;
pub mod scaler;

// Re-exportações
pub use audio::PapriumAudio;
pub use decoder::{PapriumDecoder, DecoderType};
pub use sprite::PapriumSpriteEngine;
pub use scaler::PapriumScaler;

// Constantes
pub const PAPRIUM_BOSS1: u8 = 0x17;
pub const PAPRIUM_BOSS2: u8 = 0x21;
pub const PAPRIUM_BOSS3: u8 = 0x22;
pub const PAPRIUM_BOSS4: u8 = 0x23;

// Tabela de volume (convertida do C original)
pub const PAPRIUM_VOLUME_TABLE: [u8; 256] = [
    0x00, 0x03, 0x07, 0x11, 0x15, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x40,
    0x44, 0x48, 0x4C, 0x50, 0x54, 0x58, 0x5C, 0x60, 0x64, 0x68, 0x6C, 0x70, 0x74, 0x78, 0x7C, 0x80,
    0x84, 0x88, 0x8C, 0x90, 0x94, 0x98, 0x9C, 0xA0, 0xA4, 0xA8, 0xAC, 0xB0, 0xB4, 0xB8, 0xBC, 0xC0,
    0xC4, 0xC8, 0xCC, 0xD0, 0xD4, 0xD8, 0xDC, 0xE0, 0xE4, 0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0xFC, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

/// Voz de áudio (para SFX e música)
#[derive(Debug, Clone, Default)]
pub struct PapriumVoice {
    pub volume: i32,
    pub panning: i32,
    pub flags: i32,
    pub voice_type: i32,
    
    pub size: i32,
    pub ptr: i32,
    pub tick: i32,
    
    pub loop_flag: i32,
    pub echo: i32,
    pub program: i32,
    
    pub count: i32,
    pub time: i32,
    
    pub start: i32,
    pub num: i32,
    
    pub decay: i32,
    pub decay2: i32,
    pub release: i32,
    pub sustain: i32,
    
    pub duration: i32,
    pub velocity: i32,
    pub keyon: i32,
    pub key_type: i32,
    pub pitch: i32,
    pub cents: i32,
    pub modulation: i32,
}

/// Estrutura principal do Paprium
pub struct PapriumAsic {
    // Memórias internas
    pub ram: [u8; 0x10000],
    pub decoder_ram: [u8; 0x10000],
    pub scaler_ram: [u8; 0x1000],
    pub music_ram: [u8; 0x8000],
    pub exps_ram: [u8; 14 * 8],
    
    // Vozes de áudio
    pub sfx: [PapriumVoice; 8],
    pub music: [PapriumVoice; 26],
    
    // Estado do áudio
    pub music_section: i32,
    pub audio_tick: i32,
    pub music_segment: i32,
    
    pub out_l: i32,
    pub out_r: i32,
    pub audio_flags: i32,
    pub sfx_volume: i32,
    pub music_volume: i32,
    
    // Decodificador
    pub decoder_mode: i32,
    pub decoder_ptr: i32,
    pub decoder_size: i32,
    
    // Renderização de sprites
    pub draw_src: i32,
    pub draw_dst: i32,
    pub obj: [i32; 0x31],
    
    // Eco de áudio
    pub echo_l: [i32; 48000 / 4],
    pub echo_r: [i32; 48000 / 4],
    pub echo_ptr: i32,
    pub echo_pan: i32,
    
    // Música MP3
    pub music_track: i32,
    pub mp3_ptr: i32,
    pub music_tick: i32,
    
    // Memórias de dados (externas)
    pub obj_ram: Vec<u8>,
    pub wave_ram: Vec<u8>,
    
    // Ponteiros para dados ROM
    pub music_ptr: u32,
    pub wave_ptr: u32,
    pub sfx_ptr: u32,
    pub tile_ptr: u32,
    pub sprite_ptr: u32,
    
    // Estado
    pub enabled: bool,
    pub tmss: i32,
    pub fast_dma_hack: bool,
    pub skip_boot1: bool,
    
    // Contadores de debug
    pub cmd_count: i32,
    
    // Referências externas
    pub rom_data: Arc<Vec<u8>>,
    pub sound: Option<Arc<Mutex<Sound>>>,
    pub bus: Option<Arc<Mutex<MemoryBus>>>,
    pub vdp: Option<Arc<Mutex<Vdp>>>,
    
    // Caminho ROM
    pub rom_dir: PathBuf,
}

impl PapriumAsic {
    /// Cria uma nova instância do Paprium ASIC
    pub fn new(rom_data: Arc<Vec<u8>>, rom_dir: PathBuf) -> Self {
        info!("Inicializando Paprium ASIC (Project Little Man)");
        
        Self {
            ram: [0; 0x10000],
            decoder_ram: [0; 0x10000],
            scaler_ram: [0; 0x1000],
            music_ram: [0; 0x8000],
            exps_ram: [0; 14 * 8],
            
            sfx: [PapriumVoice::default(); 8],
            music: [PapriumVoice::default(); 26],
            
            music_section: 0,
            audio_tick: 0,
            music_segment: 0,
            
            out_l: 0,
            out_r: 0,
            audio_flags: 0,
            sfx_volume: 0,
            music_volume: 0,
            
            decoder_mode: 0,
            decoder_ptr: 0,
            decoder_size: 0,
            
            draw_src: 0,
            draw_dst: 0,
            obj: [0; 0x31],
            
            echo_l: [0; 48000 / 4],
            echo_r: [0; 48000 / 4],
            echo_ptr: 0,
            echo_pan: 0,
            
            music_track: 0,
            mp3_ptr: 0,
            music_tick: 0,
            
            obj_ram: vec![0; 0x80000],
            wave_ram: vec![0; 0x180000],
            
            music_ptr: 0,
            wave_ptr: 0,
            sfx_ptr: 0,
            tile_ptr: 0,
            sprite_ptr: 0,
            
            enabled: false,
            tmss: 1,
            fast_dma_hack: false,
            skip_boot1: true,
            
            cmd_count: 0,
            
            rom_data,
            sound: None,
            bus: None,
            vdp: None,
            rom_dir,
        }
    }
    
    /// Inicializa o Paprium ASIC
    pub fn init(&mut self) -> CartridgeResult<()> {
        info!("Paprium ASIC inicializando...");
        
        // Copia os primeiros 64KB da ROM para a RAM
        let copy_size = self.rom_data.len().min(0x10000);
        self.ram[..copy_size].copy_from_slice(&self.rom_data[..copy_size]);
        
        // Configuração inicial
        self.fast_dma_hack = true;
        self.skip_boot1 = true;
        
        // Inicializa ponteiros (simplificado - na realidade seria dinâmico)
        // Estes valores devem vir da ROM
        self.music_ptr = 0;
        self.wave_ptr = 0;
        self.sfx_ptr = 0;
        self.tile_ptr = 0;
        self.sprite_ptr = 0;
        
        // Decodifica dados de sprite
        if self.sprite_ptr < self.rom_data.len() as u32 {
            // Aqui seria a decodificação real
            // Por enquanto apenas copiamos
            let src = self.sprite_ptr as usize;
            let dst_size = self.obj_ram.len().min(self.rom_data.len() - src);
            self.obj_ram[..dst_size].copy_from_slice(&self.rom_data[src..src + dst_size]);
        }
        
        // Configuração de controles
        // *(uint16*)(paprium_s.ram + 0x192) = 0x3634;  /* 6-button, multitap */
        let addr = 0x192;
        self.ram[addr] = 0x36;
        self.ram[addr + 1] = 0x34;
        
        // Patch dinâmico
        self.ram[0x1D1D] = 0x04;  // rom ok
        self.ram[0x1D2C] = 0x67;
        
        // Boot hack (simplificado)
        let boot_addr = 0x1560;
        self.ram[boot_addr] = 0x4E;
        self.ram[boot_addr + 1] = 0xF9;
        self.ram[boot_addr + 2] = 0x00;
        self.ram[boot_addr + 3] = 0x01;
        self.ram[boot_addr + 4] = 0x01;
        self.ram[boot_addr + 5] = 0x00;
        
        self.music_segment = -1;
        self.enabled = true;
        
        info!("Paprium ASIC inicializado com sucesso");
        Ok(())
    }
    
    /// Reseta o Paprium ASIC
    pub fn reset(&mut self) {
        info!("Resentando Paprium ASIC");
        
        // Reinicializa memórias
        self.ram.fill(0);
        self.decoder_ram.fill(0);
        self.scaler_ram.fill(0);
        self.music_ram.fill(0);
        self.exps_ram.fill(0);
        
        // Reinicializa vozes
        for voice in &mut self.sfx {
            *voice = PapriumVoice::default();
        }
        for voice in &mut self.music {
            *voice = PapriumVoice::default();
        }
        
        // Reinicializa estado
        self.music_section = 0;
        self.audio_tick = 0;
        self.music_segment = -1;
        
        self.out_l = 0;
        self.out_r = 0;
        self.audio_flags = 0;
        self.sfx_volume = 0x80;  // Volume padrão
        self.music_volume = 0x80;
        
        self.decoder_mode = 0;
        self.decoder_ptr = 0;
        self.decoder_size = 0;
        
        self.draw_src = 0x2000;
        self.draw_dst = 0x0200;
        self.obj.fill(0);
        
        self.echo_l.fill(0);
        self.echo_r.fill(0);
        self.echo_ptr = 0;
        self.echo_pan = 0;
        
        self.music_track = 0;
        self.mp3_ptr = 0;
        self.music_tick = 0;
        
        self.skip_boot1 = true;
        self.cmd_count = 0;
        
        // Recarrega da ROM
        if let Err(e) = self.init() {
            error!("Erro ao reinicializar Paprium ASIC: {:?}", e);
        }
    }
    
    /// Processa um comando do Paprium
    pub fn process_command(&mut self, data: u16) {
        let cmd = (data >> 8) as u8;
        let param = (data & 0xFF) as u8;
        
        self.cmd_count = 0;
        
        match cmd {
            0x84 => self.cmd_mapper(param),
            0x88 => self.cmd_audio_setting(param),
            0x8C => self.cmd_music(param),
            0x8D => self.cmd_music_setting(param),
            0xAD => self.cmd_sprite(param),
            0xAE => self.cmd_sprite_start(param),
            0xAF => self.cmd_sprite_stop(param),
            0xB0 => self.cmd_sprite_init(param),
            0xB1 => self.cmd_sprite_pause(param),
            0xC6 => self.cmd_boot(param),
            0xC9 => self.cmd_music_volume(param),
            0xCA => self.cmd_sfx_volume(param),
            0xD1 => self.cmd_sfx_play(param),
            0xD2 => self.cmd_sfx_off(param),
            0xD3 => self.cmd_sfx_loop(param),
            0xD6 => self.cmd_music_special(param),
            0xDA => self.cmd_decoder(param),
            0xDB => self.cmd_decoder_copy(param),
            0xDF => self.cmd_sram_read(param),
            0xE0 => self.cmd_sram_write(param),
            0xF4 => self.cmd_scaler_init(param),
            0xF5 => self.cmd_scaler(param),
            
            _ => {
                trace!("Comando Paprium não implementado: {:02X} {:02X}", cmd, param);
            }
        }
        
        // Limpa flag de comando pendente
        let addr = 0x1FEA;
        let current = u16::from_be_bytes([self.ram[addr], self.ram[addr + 1]]);
        self.ram[addr] = (current & 0x7FFF).to_be_bytes()[0];
        self.ram[addr + 1] = (current & 0x7FFF).to_be_bytes()[1];
    }
    
    /// Lê um byte da memória do Paprium
    pub fn read_byte(&self, address: u32) -> u8 {
        let addr = address as usize;
        
        if addr < self.ram.len() {
            self.ram[addr]
        } else {
            0xFF
        }
    }
    
    /// Lê uma word da memória do Paprium
    pub fn read_word(&self, address: u32) -> u16 {
        let addr = address as usize;
        
        if addr + 1 < self.ram.len() {
            u16::from_be_bytes([self.ram[addr], self.ram[addr + 1]])
        } else {
            0xFFFF
        }
    }
    
    /// Escreve um byte na memória do Paprium
    pub fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address as usize;
        
        if addr < self.ram.len() {
            self.ram[addr] = value;
            
            // Se escrever no registrador de comando, processa
            if addr == 0x1FEA || addr == 0x1FEB {
                if addr == 0x1FEA {
                    let word = u16::from_be_bytes([value, self.ram[0x1FEB]]);
                    self.process_command(word);
                } else {
                    let word = u16::from_be_bytes([self.ram[0x1FEA], value]);
                    self.process_command(word);
                }
            }
        }
    }
    
    /// Escreve uma word na memória do Paprium
    pub fn write_word(&mut self, address: u32, value: u16) {
        let addr = address as usize;
        let bytes = value.to_be_bytes();
        
        if addr + 1 < self.ram.len() {
            self.ram[addr] = bytes[0];
            self.ram[addr + 1] = bytes[1];
            
            // Se escrever no registrador de comando, processa
            if addr == 0x1FEA {
                self.process_command(value);
            }
        }
    }
    
    /// Processa áudio do Paprium
    pub fn process_audio(&mut self, cycles: u32) {
        // Implementação simplificada
        // O áudio real é muito complexo
        
        // Atualiza tick de áudio
        self.audio_tick = self.audio_tick.wrapping_add(1);
        
        // Processa música
        if self.music_track != 0 {
            self.music_tick = self.music_tick.wrapping_add(0x10000);
            if self.music_tick >= 0x10000 {
                self.music_tick -= 0x10000;
                self.mp3_ptr = self.mp3_ptr.wrapping_add(2);
            }
        }
        
        // Processa partitura musical
        if (self.audio_tick % 4) == 0 && self.music_segment != -1 {
            self.process_music_sheet();
        }
    }
    
    /// Processa partitura musical
    fn process_music_sheet(&mut self) {
        // Implementação simplificada
        // A implementação real é extremamente complexa
        
        if self.music_segment == -1 {
            self.music_track = 0;
            return;
        }
        
        // Incrementa seção
        self.music_section = self.music_section.wrapping_add(1);
        
        // Verifica fim da seção
        if self.music_section >= 0x100 {  // Valor simplificado
            self.music_section = 0;
            self.music_segment = self.music_segment.wrapping_add(1);
            
            if self.music_segment >= 0x10 {  // Valor simplificado
                self.music_segment = 0;
            }
        }
    }
    
    // Implementações dos comandos (simplificadas)
    
    fn cmd_mapper(&mut self, _param: u8) {
        trace!("Paprium: Comando Mapper");
        // Implementação real copiaria dados da ROM
    }
    
    fn cmd_audio_setting(&mut self, param: u8) {
        trace!("Paprium: Configuração de áudio: {:02X}", param);
        self.audio_flags = param as i32;
    }
    
    fn cmd_music(&mut self, param: u8) {
        trace!("Paprium: Música track: {:02X}", param);
        self.music_track = param as i32;
        self.mp3_ptr = 0;
        self.music_tick = 0;
    }
    
    fn cmd_music_setting(&mut self, param: u8) {
        trace!("Paprium: Configuração de música: {:02X}", param);
        if param == 8 || param == 0 {
            self.music_segment = -1;
        }
    }
    
    fn cmd_sprite(&mut self, param: u8) {
        trace!("Paprium: Sprite index: {:02X}", param);
        // Processamento complexo de sprites
    }
    
    fn cmd_sprite_start(&mut self, _param: u8) {
        trace!("Paprium: Início de sprites");
        self.draw_src = 0x2000;
        self.draw_dst = 0x0200;
    }
    
    fn cmd_sprite_stop(&mut self, _param: u8) {
        trace!("Paprium: Parada de sprites");
    }
    
    fn cmd_sprite_init(&mut self, _param: u8) {
        trace!("Paprium: Inicialização de sprites");
        // Limpa lista de sprites estendidos
        self.exps_ram.fill(0);
    }
    
    fn cmd_sprite_pause(&mut self, _param: u8) {
        trace!("Paprium: Pausa de sprites");
    }
    
    fn cmd_boot(&mut self, _param: u8) {
        trace!("Paprium: Boot");
        // Configura ponteiros iniciais
        self.cmd_count = 0;
    }
    
    fn cmd_music_volume(&mut self, param: u8) {
        trace!("Paprium: Volume música: {:02X}", param);
        self.music_volume = param as i32;
    }
    
    fn cmd_sfx_volume(&mut self, param: u8) {
        trace!("Paprium: Volume SFX: {:02X}", param);
        self.sfx_volume = param as i32;
    }
    
    fn cmd_sfx_play(&mut self, param: u8) {
        trace!("Paprium: Play SFX: {:02X}", param);
        // Configura uma voz SFX
        let chan = self.read_word(0x1E10) as usize;
        let vol = self.read_word(0x1E12) as i32;
        let pan = self.read_word(0x1E14) as i32;
        let flags = self.read_word(0x1E16) as i32;
        
        if chan < self.sfx.len() {
            self.sfx[chan].num = param as i32;
            self.sfx[chan].volume = vol;
            self.sfx[chan].panning = pan;
            self.sfx[chan].flags = flags;
            self.sfx[chan].size = 1;  // Ativa a voz
        }
    }
    
    fn cmd_sfx_off(&mut self, param: u8) {
        trace!("Paprium: Off SFX: {:02X}", param);
        for (i, voice) in self.sfx.iter_mut().enumerate() {
            if (param & (1 << i)) != 0 {
                voice.size = 0;  // Desativa a voz
            }
        }
    }
    
    fn cmd_sfx_loop(&mut self, param: u8) {
        trace!("Paprium: Loop SFX: {:02X}", param);
        for (i, voice) in self.sfx.iter_mut().enumerate() {
            if (param & (1 << i)) != 0 {
                voice.loop_flag = 1;
                voice.volume = self.read_word(0x1E10) as i32;
                voice.panning = self.read_word(0x1E12) as i32;
                voice.decay = self.read_word(0x1E14) as i32;
            }
        }
    }
    
    fn cmd_music_special(&mut self, param: u8) {
        trace!("Paprium: Música especial: {:02X}", param);
        // Comandos especiais de música
    }
    
    fn cmd_decoder(&mut self, param: u8) {
        trace!("Paprium: Decodificador mode: {:02X}", param);
        self.decoder_mode = param as i32;
    }
    
    fn cmd_decoder_copy(&mut self, param: u8) {
        trace!("Paprium: Cópia do decodificador: {:02X}", param);
        let offset = self.read_word(0x1E12) as i32;
        let size = self.read_word(0x1E14) as i32;
        
        self.decoder_ptr = offset;
        self.decoder_size = size;
    }
    
    fn cmd_sram_read(&mut self, param: u8) {
        trace!("Paprium: Leitura SRAM bank: {:02X}", param);
        // Implementação real leria da SRAM
    }
    
    fn cmd_sram_write(&mut self, param: u8) {
        trace!("Paprium: Escrita SRAM bank: {:02X}", param);
        // Implementação real escreveria na SRAM
    }
    
    fn cmd_scaler_init(&mut self, param: u8) {
        trace!("Paprium: Inicialização do scaler: {:02X}", param);
        // Inicializa o scaler de vídeo
    }
    
    fn cmd_scaler(&mut self, param: u8) {
        trace!("Paprium: Scaler: {:02X}", param);
        // Processa scaler de vídeo
    }
    
    /// Conecta ao sistema de som
    pub fn connect_sound(&mut self, sound: Arc<Mutex<Sound>>) {
        self.sound = Some(sound);
        debug!("Paprium ASIC conectado ao sistema de som");
    }
    
    /// Conecta ao barramento de memória
    pub fn connect_bus(&mut self, bus: Arc<Mutex<MemoryBus>>) {
        self.bus = Some(bus);
        debug!("Paprium ASIC conectado ao barramento");
    }
    
    /// Conecta ao VDP
    pub fn connect_vdp(&mut self, vdp: Arc<Mutex<Vdp>>) {
        self.vdp = Some(vdp);
        debug!("Paprium ASIC conectado ao VDP");
    }
}

// Implementação da trait CartridgeChip
impl crate::core::cartridge::chips::CartridgeChip for PapriumAsic {
    fn init(&mut self) {
        if let Err(e) = self.init() {
            error!("Falha ao inicializar Paprium ASIC: {:?}", e);
        }
    }
    
    fn reset(&mut self) {
        self.reset();
    }
    
    fn update(&mut self, cycles: u32) {
        self.process_audio(cycles);
    }
    
    fn save_state(&self) -> Vec<u8> {
        // Serialização simplificada
        let mut state = Vec::new();
        
        // Salva estado básico
        state.push(self.enabled as u8);
        state.extend_from_slice(&self.music_track.to_le_bytes());
        state.extend_from_slice(&self.sfx_volume.to_le_bytes());
        state.extend_from_slice(&self.music_volume.to_le_bytes());
        
        // Salva RAM
        state.extend_from_slice(&self.ram);
        
        state
    }
    
    fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 1 + 4 + 4 + 4 + self.ram.len() {
            return false;
        }
        
        let mut offset = 0;
        
        // Carrega estado básico
        self.enabled = data[offset] != 0;
        offset += 1;
        
        self.music_track = i32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        offset += 4;
        
        self.sfx_volume = i32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        offset += 4;
        
        self.music_volume = i32::from_le_bytes([data[offset], data[offset+1], data[offset+2], data[offset+3]]);
        offset += 4;
        
        // Carrega RAM
        let ram_len = self.ram.len();
        self.ram.copy_from_slice(&data[offset..offset + ram_len]);
        
        true
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
        self.process_audio(samples);
        
        // Aqui integraríamos com o sistema de som real
        // Por enquanto, implementação simplificada
        if let Some(blip) = sound.blips.get_mut(3) {
            // Adiciona silêncio como placeholder
            blip.add_delta_fast(0, -self.out_l as i16, -self.out_r as i16);
            blip.end_frame(samples as i32);
        }
    }
}