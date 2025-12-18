//! Gerenciamento de Save RAM (bateria).
//! Baseado em `sram.c` do Genesis Plus GX.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use log::{info, warn, error};

/// Save RAM com suporte a persistência
pub struct SaveRam {
    pub data: Vec<u8>,
    pub size: usize,
    pub enabled: bool,
    pub write_protect: bool,
    pub dirty: bool,
    pub file_path: Option<String>,
}

impl SaveRam {
    /// Cria uma nova Save RAM
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
            size,
            enabled: false,
            write_protect: false,
            dirty: false,
            file_path: None,
        }
    }
    
    /// Carrega Save RAM de um arquivo
    pub fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let mut file = File::open(&path)?;
        file.read_to_end(&mut self.data)?;
        self.file_path = Some(path.as_ref().to_string_lossy().into_owned());
        self.dirty = false;
        info!("Save RAM carregada: {} bytes", self.data.len());
        Ok(())
    }
    
    /// Salva Save RAM em um arquivo
    pub fn save_to_file<P: AsRef<Path>>(&mut self, path: P) -> std::io::Result<()> {
        let path = path.as_ref();
        let mut file = File::create(path)?;
        file.write_all(&self.data[..self.size])?;
        self.file_path = Some(path.to_string_lossy().into_owned());
        self.dirty = false;
        info!("Save RAM salva: {} bytes", self.size);
        Ok(())
    }
    
    /// Salva automaticamente se suja
    pub fn auto_save(&mut self) {
        if self.dirty && self.enabled && !self.write_protect {
            if let Some(ref path) = self.file_path {
                if let Err(e) = self.save_to_file(path) {
                    error!("Falha ao salvar Save RAM: {}", e);
                }
            }
        }
    }
    
    /// Lê um byte
    pub fn read_byte(&self, addr: u32) -> u8 {
        if self.enabled {
            let addr = addr as usize % self.size;
            self.data[addr]
        } else {
            0xFF
        }
    }
    
    /// Escreve um byte
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        if self.enabled && !self.write_protect {
            let addr = addr as usize % self.size;
            self.data[addr] = value;
            self.dirty = true;
        }
    }
    
    /// Habilita/desabilita
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    
    /// Habilita/desabilita proteção contra escrita
    pub fn set_write_protect(&mut self, protect: bool) {
        self.write_protect = protect;
    }
}