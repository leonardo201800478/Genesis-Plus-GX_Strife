//! MegaSD flashcart hardware emulation
//! CD hardware interface overlay & enhanced ROM mappers
//! Based on original code by Eke-Eke (Genesis Plus GX)

use crate::core::memory::{MemoryBus, MemoryError, MemoryResult};
use crate::core::cartridge::Cartridge;
use crate::core::cartridge::memory::Sram;
use log::{info, warn, debug, trace, error};
use std::time::{SystemTime, UNIX_EPOCH};

/// MegaSD hardware state
#[derive(Debug, Clone)]
pub struct MegaSD {
    // Control registers
    unlock: bool,                    // Bank 0 access unlocked
    bank0: u8,                       // Bank 0 register value
    special: u8,                     // Special bank register
    write_enable: bool,              // ROM write enable bit
    overlay_enable: bool,            // CD hardware overlay enabled
    playback_loop: u8,               // CDDA playback loop flag
    playback_loop_track: u8,         // Loop track index
    playback_end_track: u8,          // End track index
    result: u16,                     // Command result
    
    // Audio playback state
    fadeout_start_volume: u16,       // Volume before fadeout
    fadeout_samples_total: i32,      // Total fadeout samples
    fadeout_samples_count: i32,      // Remaining fadeout samples
    playback_samples_count: i32,      // Remaining playback samples
    playback_loop_sector: i32,       // Loop sector LBA
    playback_end_sector: i32,        // End sector LBA
    
    // Internal buffer (2KB)
    buffer: [u8; 0x800],
    
    // CD hardware emulation state (simplified)
    cd_loaded: bool,                 // Disc loaded flag
    cd_status: u8,                   // CD status (0=play, 1=stop/pause)
    cd_current_track: u8,            // Current track
    cd_current_sector: i32,          // Current sector LBA
    
    // Version & serial
    version: [u8; 16],               // MegaSD version string
    
    // System references
    cart: Option<Box<dyn Cartridge>>, // Cartridge reference
    sram: Option<Box<dyn Sram>>,      // SRAM reference
    memory_bus: Option<*mut MemoryBus>, // Memory bus reference (raw pointer for flexibility)
}

impl MegaSD {
    /// Creates a new MegaSD instance
    pub fn new() -> Self {
        // Default MegaSD version & serial number
        let version = [
            b'M', b'E', b'G', b'A', b'S', b'D', // "MEGASD"
            0x01, 0x04, 0x07, 0x00,             // Version 1.4.7
            0xFF, 0xFF,                         // Reserved
            0x12, 0x34, 0x56, 0x78,             // Serial number
        ];
        
        Self {
            unlock: false,
            bank0: 0,
            special: 0x07, // Default to bank 7
            write_enable: false,
            overlay_enable: false,
            playback_loop: 0,
            playback_loop_track: 0,
            playback_end_track: 0,
            result: 0,
            
            fadeout_start_volume: 0x400, // Max volume
            fadeout_samples_total: 0,
            fadeout_samples_count: 0,
            playback_samples_count: 0,
            playback_loop_sector: 0,
            playback_end_sector: 0,
            
            buffer: [0; 0x800],
            
            cd_loaded: false,
            cd_status: 0x01, // Stopped
            cd_current_track: 0,
            cd_current_sector: 0,
            
            version,
            
            cart: None,
            sram: None,
            memory_bus: None,
        }
    }
    
    /// Initializes MegaSD hardware
    pub fn init(&mut self, cart: Box<dyn Cartridge>, sram: Option<Box<dyn Sram>>) {
        self.cart = Some(cart);
        self.sram = sram;
        
        info!("MegaSD hardware initialized");
    }
    
    /// Connects to memory bus
    pub fn connect_memory_bus(&mut self, bus: *mut MemoryBus) {
        self.memory_bus = Some(bus);
    }
    
    /// Resets MegaSD hardware
    pub fn reset(&mut self) {
        *self = Self::new();
        
        // Reset CD hardware state
        self.cd_status = 0x01; // Stopped
        
        info!("MegaSD hardware reset");
    }
    
    /// Enhanced "SSF2" mapper write handler
    pub fn enhanced_ssf2_mapper_write(&mut self, address: u32, data: u8) -> bool {
        match address & 0xF {
            0x0 => {
                // Check protect bit
                if (data & 0x80) != 0 {
                    // Access to bank #0 register and ROM write enable bit is unlocked
                    self.unlock = true;
                    
                    // ROM write enable bit
                    self.write_enable = (data & 0x20) != 0;
                    
                    trace!("MegaSD: Bank 0 unlocked, write_enable={}", self.write_enable);
                } else {
                    // Access to bank #0 register and ROM write enable bit is locked
                    self.unlock = false;
                    
                    // Disable ROM write enable access
                    self.write_enable = false;
                    
                    trace!("MegaSD: Bank 0 locked");
                }
                
                // Update last bank mapping
                self.update_last_bank_mapping();
                true
            }
            
            0xF => {
                // Special bank register
                self.special = data;
                
                // Update last bank mapping
                self.update_last_bank_mapping();
                true
            }
            
            _ => {
                // LWR only
                if (address & 1) != 0 {
                    // 512K ROM paging (max. 8MB)
                    let page = (data & 0x0F) as u32;
                    
                    // Cartridge area ($000000-$3FFFFF) is divided into 8 x 512K banks
                    let bank = ((address << 2) & 0x38) as u8;
                    
                    // Check selected bank is not locked
                    if bank != 0x00 || self.unlock {
                        trace!("MegaSD: Mapping page {} to bank {}", page, bank);
                        // Actual mapping would be done by memory system
                        return true;
                    }
                }
                false
            }
        }
    }
    
    /// ROM write access mapper
    pub fn rom_mapper_write(&mut self, address: u32, data: u8) -> bool {
        if (address & 0xFF) == 0xFF {
            if data == b'W' {
                // Enable write access to cartridge ROM area
                self.write_enable = true;
                trace!("MegaSD: ROM write access enabled");
            } else {
                // Disable write access to cartridge ROM area
                self.write_enable = false;
                
                // Enable CD hardware overlay access
                self.overlay_enable = true;
                trace!("MegaSD: ROM write access disabled, overlay enabled");
            }
            true
        } else {
            false
        }
    }
    
    /// Updates CDDA samples playback
    pub fn update_cdda(&mut self, samples: u32) {
        let mut remaining = samples as i32;
        
        while remaining > 0 {
            // Check if audio playback is paused or stopped
            if self.cd_status == 0x01 {
                // Clear remaining samples without updating counters
                break;
            }
            
            // Calculate samples to process this iteration
            let mut count = remaining;
            
            // Check against fade out remaining samples
            if self.fadeout_samples_count > 0 && count > self.fadeout_samples_count {
                count = self.fadeout_samples_count;
            }
            
            // Check against playback remaining samples
            if self.playback_samples_count > 0 && count > self.playback_samples_count {
                count = self.playback_samples_count;
            }
            
            // Adjust remaining samples
            remaining -= count;
            
            // Update fade out
            if self.fadeout_samples_count > 0 {
                self.fadeout_samples_count -= count;
                
                // Check end of fade out
                if self.fadeout_samples_count <= 0 {
                    // Pause audio playback
                    self.cd_status = 0x01;
                    
                    // Restore initial volume
                    // (volume restoration would happen in audio system)
                    trace!("MegaSD: Fade out complete, playback paused");
                }
            }
            
            // Update playback
            if self.playback_samples_count > 0 {
                self.playback_samples_count -= count;
                
                // Check end of current track
                if self.playback_samples_count <= 0 {
                    // Handle track transitions
                    self.handle_track_transition();
                }
            }
        }
    }
    
    /// Handles track transition logic
    fn handle_track_transition(&mut self) {
        // Check playback end track
        if (self.cd_current_track as u8) < self.playback_end_track {
            // Seek to next track
            self.cd_current_track += 1;
            
            // Check if last track is being played
            if self.cd_current_track as u8 == self.playback_end_track {
                // Update samples count for partial last track
                // In real implementation, would use actual sector counts
                self.playback_samples_count = 44100; // 1 second default
            } else {
                // Full track playback
                self.playback_samples_count = 176400; // 4 seconds default
            }
        }
        // Check track loop
        else if self.playback_loop > 0 {
            // Loop back to start
            self.cd_current_track = self.playback_loop_track;
            
            // Update samples count for loop
            if self.cd_current_track as u8 == self.playback_end_track {
                // Single track loop
                self.playback_samples_count = 176400; // 4 seconds default
            } else {
                // Multiple track loop
                self.playback_samples_count = 176400; // 4 seconds default
            }
            
            trace!("MegaSD: Track loop to track {}", self.cd_current_track);
        } else {
            // Stop audio playback
            self.cd_status = 0x01;
            trace!("MegaSD: Playback stopped");
        }
    }
    
    /// CD hardware overlay interface - byte write
    pub fn ctrl_write_byte(&mut self, address: u32, data: u8) -> bool {
        // Check if overlay area access is enabled
        if self.overlay_enable {
            // 2KB buffer area
            if address >= 0x03F800 {
                let buffer_addr = (address & 0x7FF) as usize;
                if buffer_addr < self.buffer.len() {
                    self.buffer[buffer_addr] = data;
                    return true;
                }
            }
        }
        false
    }
    
    /// CD hardware overlay interface - word write
    pub fn ctrl_write_word(&mut self, address: u32, data: u16) -> bool {
        // Overlay port (word write only)
        if address == 0x03F7FA {
            // Enable/disable CD hardware overlay access
            self.overlay_enable = data == 0xCD54;
            trace!("MegaSD: Overlay {}", if self.overlay_enable { "enabled" } else { "disabled" });
            return true;
        }
        
        // Check if overlay area access is enabled
        if self.overlay_enable {
            // Command port (word write only)
            if address == 0x03F7FE {
                let command = (data >> 8) as u8;
                let param = (data & 0xFF) as u8;
                
                match command {
                    0x10 => { // Get MegaSD version & serial number
                        self.buffer[0..16].copy_from_slice(&self.version);
                        return true;
                    }
                    
                    0x11 | 0x12 | 0x1A => { // Play CDDA track
                        self.handle_cdda_play_command(command, param);
                        return true;
                    }
                    
                    0x13 => { // Pause CDDA track
                        self.handle_cdda_pause_command(param);
                        return true;
                    }
                    
                    0x14 => { // Resume CDDA track
                        if self.cd_status == 0x01 { // Paused
                            self.cd_status = 0x00; // Playing
                            trace!("MegaSD: Playback resumed");
                        }
                        return true;
                    }
                    
                    0x15 => { // Set CDDA volume (0-255)
                        let volume = ((param as u16) * 0x400) / 255;
                        
                        if self.fadeout_samples_count > 0 {
                            // Update default volume to restore after fadeout
                            self.fadeout_start_volume = volume;
                        } else {
                            // Update current volume
                            // Would be applied to audio system
                        }
                        trace!("MegaSD: Volume set to {}", param);
                        return true;
                    }
                    
                    0x16 => { // Get CDDA playback status
                        self.result = if self.cd_status == 0x00 { 0x01 } else { 0x00 };
                        return true;
                    }
                    
                    0x17 => { // Request CD sector read
                        if self.cd_loaded {
                            // Get LBA from buffer (big-endian)
                            let lba = self.read_big_endian_u32(0) as i32 - 150;
                            if lba >= 0 {
                                self.cd_current_sector = lba;
                                self.cd_status = 0x00; // Playing (data)
                                trace!("MegaSD: Sector read requested at LBA {}", lba);
                            }
                        }
                        return true;
                    }
                    
                    0x18 => { // Transfer last read sector
                        if self.cd_loaded && self.cd_status == 0x00 {
                            // Read sector data to buffer
                            // In real implementation, would read from disc image
                            trace!("MegaSD: Sector data transferred to buffer");
                        }
                        return true;
                    }
                    
                    0x19 => { // Request read of next sector
                        if self.cd_loaded && self.cd_status == 0x00 {
                            self.cd_current_sector += 1;
                            trace!("MegaSD: Next sector requested");
                        }
                        return true;
                    }
                    
                    0x1B => { // Play CDDA from specific sector
                        self.handle_cdda_play_from_sector_command(data);
                        return true;
                    }
                    
                    0x1C..=0x21 => { // Unsupported commands
                        self.result = 0;
                        trace!("MegaSD: Unsupported command 0x{:02X}", command);
                        return true;
                    }
                    
                    _ => {
                        // Invalid command
                        return false;
                    }
                }
            }
            
            // 2KB buffer area
            if address >= 0x03F800 {
                let buffer_addr = (address & 0x7FE) as usize;
                if buffer_addr + 1 < self.buffer.len() {
                    // Write word in little-endian format (target is little-endian)
                    self.buffer[buffer_addr] = data as u8;
                    self.buffer[buffer_addr + 1] = (data >> 8) as u8;
                    return true;
                }
            }
        }
        
        false
    }
    
    /// CD hardware overlay interface - byte read
    pub fn ctrl_read_byte(&self, address: u32) -> Option<u8> {
        // Check if overlay area access is enabled
        if self.overlay_enable {
            // ID port
            if (0x03F7F6..=0x03F7F9).contains(&address) {
                let id = [0x42, 0x41, 0x54, 0x45]; // "BATE"
                return Some(id[(address - 0x03F7F6) as usize]);
            }
            
            // Overlay port
            if (0x03F7FA..=0x03F7FB).contains(&address) {
                return Some(if (address & 1) != 0 { 0x54 } else { 0xCD });
            }
            
            // Result port
            if (0x03F7FC..=0x03F7FD).contains(&address) {
                return Some(if (address & 1) != 0 {
                    (self.result & 0xFF) as u8
                } else {
                    (self.result >> 8) as u8
                });
            }
            
            // Command port
            if (0x03F7FE..=0x03F7FF).contains(&address) {
                // Commands processing time is not emulated
                return Some(0x00);
            }
            
            // 2KB buffer area
            if address >= 0x03F800 {
                let buffer_addr = (address & 0x7FF) as usize;
                if buffer_addr < self.buffer.len() {
                    return Some(self.buffer[buffer_addr]);
                }
            }
        }
        
        None
    }
    
    /// CD hardware overlay interface - word read
    pub fn ctrl_read_word(&self, address: u32) -> Option<u16> {
        // Check if overlay area access is enabled
        if self.overlay_enable {
            // ID port
            if address == 0x03F7F6 || address == 0x03F7F8 {
                let id = [0x42, 0x41, 0x54, 0x45]; // "BATE"
                let idx = ((address - 0x03F7F6) / 2) as usize;
                return Some(((id[idx * 2] as u16) << 8) | id[idx * 2 + 1] as u16);
            }
            
            // Overlay port
            if address == 0x03F7FA {
                return Some(0xCD54);
            }
            
            // Result port
            if address == 0x03F7FC {
                return Some(self.result);
            }
            
            // Command port
            if address == 0x03F7FE {
                // Commands processing time is not emulated
                return Some(0x0000);
            }
            
            // 2KB buffer area
            if address >= 0x03F800 {
                let buffer_addr = (address & 0x7FE) as usize;
                if buffer_addr + 1 < self.buffer.len() {
                    // Read as little-endian (target is little-endian)
                    return Some((self.buffer[buffer_addr] as u16) | ((self.buffer[buffer_addr + 1] as u16) << 8));
                }
            }
        }
        
        None
    }
    
    /// PCM sound chip interface - byte write
    pub fn pcm_write_byte(&mut self, address: u32, data: u8, cycles: u32) -> bool {
        // /LDS only (odd addresses)
        if (address & 1) != 0 {
            let pcm_addr = ((address >> 1) & 0x1FFF) as u16;
            self.write_pcm_register(pcm_addr, data, cycles);
            return true;
        }
        false
    }
    
    /// PCM sound chip interface - word write
    pub fn pcm_write_word(&mut self, address: u32, data: u16, cycles: u32) -> bool {
        // /LDS only writes low byte
        let pcm_addr = ((address >> 1) & 0x1FFF) as u16;
        self.write_pcm_register(pcm_addr, data as u8, cycles);
        true
    }
    
    /// PCM sound chip interface - byte read
    pub fn pcm_read_byte(&self, address: u32, cycles: u32) -> Option<u8> {
        // /LDS only (odd addresses)
        if (address & 1) != 0 {
            let pcm_addr = ((address >> 1) & 0x1FFF) as u16;
            return Some(self.read_pcm_register(pcm_addr, cycles));
        }
        None
    }
    
    /// PCM sound chip interface - word read
    pub fn pcm_read_word(&self, address: u32, cycles: u32) -> Option<u16> {
        // /LDS only reads return byte in low part of word
        let pcm_addr = ((address >> 1) & 0x1FFF) as u16;
        Some(self.read_pcm_register(pcm_addr, cycles) as u16)
    }
    
    /// Helper: Write to PCM register (simplified)
    fn write_pcm_register(&mut self, _addr: u16, _data: u8, _cycles: u32) {
        // Simplified PCM emulation
        // In full implementation, would update actual PCM chip state
        trace!("MegaSD: PCM write to 0x{:04X} = 0x{:02X}", _addr, _data);
    }
    
    /// Helper: Read from PCM register (simplified)
    fn read_pcm_register(&self, _addr: u16, _cycles: u32) -> u8 {
        // Simplified PCM emulation
        // In full implementation, would read actual PCM chip state
        trace!("MegaSD: PCM read from 0x{:04X}", _addr);
        0x00
    }
    
    /// Helper: Handle CDDA play command
    fn handle_cdda_play_command(&mut self, command: u8, track_param: u8) {
        if self.cd_loaded {
            let track = (track_param as i32) - 1;
            
            if track >= 0 {
                // Initialize playback
                self.cd_current_track = track as u8;
                self.playback_end_track = track as u8;
                self.cd_status = 0x00; // Playing
                
                // Reset fadeout if in progress
                if self.fadeout_samples_count > 0 {
                    self.fadeout_samples_count = 0;
                }
                
                // Initialize samples count (simplified)
                self.playback_samples_count = 176400; // 4 seconds default
                
                // Track loop settings
                self.playback_loop = if command == 0x12 || command == 0x1A { 1 } else { 0 };
                
                if command == 0x1A {
                    // Command 1Ah specifies track loop offset
                    self.playback_loop_sector = self.read_big_endian_u32(0) as i32;
                } else {
                    self.playback_loop_sector = 0;
                }
                
                self.playback_loop_track = track as u8;
                
                trace!("MegaSD: Playing track {} (loop={})", track + 1, self.playback_loop);
            }
        }
    }
    
    /// Helper: Handle CDDA pause command
    fn handle_cdda_pause_command(&mut self, fade_param: u8) {
        if self.cd_status == 0x00 {
            // Get fade out samples count
            self.fadeout_samples_count = (fade_param as i32) * 588;
            
            if self.fadeout_samples_count > 0 {
                // Save fade out parameters
                self.fadeout_samples_total = self.fadeout_samples_count;
                // fadeout_start_volume would be saved from audio system
                trace!("MegaSD: Fade out started ({} samples)", self.fadeout_samples_count);
            } else {
                // Pause immediately
                self.cd_status = 0x01;
                trace!("MegaSD: Playback paused immediately");
            }
        }
    }
    
    /// Helper: Handle CDDA play from sector command
    fn handle_cdda_play_from_sector_command(&mut self, _data: u16) {
        if self.cd_loaded {
            // Get playback parameters from buffer
            let start_sector = self.read_big_endian_u32(0) as i32 - 150;
            let end_sector = self.read_big_endian_u32(4) as i32 - 150;
            let loop_flag = (_data & 0x01) as u8;
            
            if start_sector >= 0 && end_sector > start_sector {
                // Initialize playback from sector
                self.cd_current_sector = start_sector;
                self.playback_end_sector = end_sector;
                self.cd_status = 0x00;
                self.playback_loop = loop_flag;
                
                if loop_flag != 0 {
                    self.playback_loop_sector = self.read_big_endian_u32(8) as i32 - 150;
                }
                
                // Calculate samples count
                self.playback_samples_count = (end_sector - start_sector) * 588;
                
                trace!("MegaSD: Playing from sector {} to {} (loop={})", 
                       start_sector, end_sector, loop_flag);
            }
        }
    }
    
    /// Helper: Read big-endian u32 from buffer
    fn read_big_endian_u32(&self, offset: usize) -> u32 {
        if offset + 3 < self.buffer.len() {
            ((self.buffer[offset] as u32) << 24) |
            ((self.buffer[offset + 1] as u32) << 16) |
            ((self.buffer[offset + 2] as u32) << 8) |
            (self.buffer[offset + 3] as u32)
        } else {
            0
        }
    }
    
    /// Helper: Update last bank mapping
    fn update_last_bank_mapping(&mut self) {
        // Update mapping for $380000-$3fffff based on special register
        match self.special {
            0x80 => {
                // SRAM mapped in $380000-$3fffff
                trace!("MegaSD: Mapping SRAM to $380000-$3FFFFF");
            }
            0x81 => {
                // PCM hardware mapped in $380000-$3fffff
                trace!("MegaSD: Mapping PCM hardware to $380000-$3FFFFF");
            }
            _ => {
                // ROM mapping
                trace!("MegaSD: Mapping ROM page {} to $380000-$3FFFFF", self.special & 0x0F);
            }
        }
    }
    
    /// Loads a disc image (simplified)
    pub fn load_disc(&mut self) -> bool {
        self.cd_loaded = true;
        info!("MegaSD: Disc loaded");
        true
    }
    
    /// Ejects current disc
    pub fn eject_disc(&mut self) {
        self.cd_loaded = false;
        self.cd_status = 0x01;
        info!("MegaSD: Disc ejected");
    }
    
    /// Saves MegaSD state
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        // Save basic state
        state.push(self.unlock as u8);
        state.push(self.bank0);
        state.push(self.special);
        state.push(self.write_enable as u8);
        state.push(self.overlay_enable as u8);
        state.push(self.playback_loop);
        state.push(self.playback_loop_track);
        state.push(self.playback_end_track);
        state.extend_from_slice(&self.result.to_le_bytes());
        
        // Save audio state
        state.extend_from_slice(&self.fadeout_start_volume.to_le_bytes());
        state.extend_from_slice(&self.fadeout_samples_total.to_le_bytes());
        state.extend_from_slice(&self.fadeout_samples_count.to_le_bytes());
        state.extend_from_slice(&self.playback_samples_count.to_le_bytes());
        state.extend_from_slice(&self.playback_loop_sector.to_le_bytes());
        state.extend_from_slice(&self.playback_end_sector.to_le_bytes());
        
        // Save buffer
        state.extend_from_slice(&self.buffer);
        
        // Save CD state
        state.push(self.cd_loaded as u8);
        state.push(self.cd_status);
        state.push(self.cd_current_track);
        state.extend_from_slice(&self.cd_current_sector.to_le_bytes());
        
        state
    }
    
    /// Loads MegaSD state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 8 + 12 + 0x800 + 1 + 1 + 1 + 4 {
            return false;
        }
        
        let mut offset = 0;
        
        // Load basic state
        self.unlock = data[offset] != 0; offset += 1;
        self.bank0 = data[offset]; offset += 1;
        self.special = data[offset]; offset += 1;
        self.write_enable = data[offset] != 0; offset += 1;
        self.overlay_enable = data[offset] != 0; offset += 1;
        self.playback_loop = data[offset]; offset += 1;
        self.playback_loop_track = data[offset]; offset += 1;
        self.playback_end_track = data[offset]; offset += 1;
        
        self.result = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        
        // Load audio state
        self.fadeout_start_volume = u16::from_le_bytes([data[offset], data[offset + 1]]);
        offset += 2;
        
        let mut bytes = [0; 4];
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.fadeout_samples_total = i32::from_le_bytes(bytes);
        offset += 4;
        
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.fadeout_samples_count = i32::from_le_bytes(bytes);
        offset += 4;
        
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.playback_samples_count = i32::from_le_bytes(bytes);
        offset += 4;
        
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.playback_loop_sector = i32::from_le_bytes(bytes);
        offset += 4;
        
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.playback_end_sector = i32::from_le_bytes(bytes);
        offset += 4;
        
        // Load buffer
        self.buffer.copy_from_slice(&data[offset..offset+0x800]);
        offset += 0x800;
        
        // Load CD state
        self.cd_loaded = data[offset] != 0; offset += 1;
        self.cd_status = data[offset]; offset += 1;
        self.cd_current_track = data[offset]; offset += 1;
        
        bytes.copy_from_slice(&data[offset..offset+4]);
        self.cd_current_sector = i32::from_le_bytes(bytes);
        
        true
    }
    
    /// Returns whether MegaSD is enabled
    pub fn is_enabled(&self) -> bool {
        self.cart.is_some()
    }
    
    /// Returns whether overlay is enabled
    pub fn overlay_enabled(&self) -> bool {
        self.overlay_enable
    }
    
    /// Returns whether ROM writes are enabled
    pub fn write_enabled(&self) -> bool {
        self.write_enable
    }
}

// Memory handler for MegaSD integration
pub struct MegaSDMemoryHandler {
    megasd: MegaSD,
}

impl MegaSDMemoryHandler {
    pub fn new() -> Self {
        Self {
            megasd: MegaSD::new(),
        }
    }
    
    pub fn init(&mut self, cart: Box<dyn Cartridge>, sram: Option<Box<dyn Sram>>) {
        self.megasd.init(cart, sram);
    }
    
    pub fn reset(&mut self) {
        self.megasd.reset();
    }
    
    pub fn read_byte(&self, address: u32) -> u8 {
        // Try overlay first
        if let Some(value) = self.megasd.ctrl_read_byte(address) {
            return value;
        }
        
        // Fall back to cartridge
        if let Some(cart) = &self.megasd.cart {
            cart.read_byte(address)
        } else {
            0xFF
        }
    }
    
    pub fn read_word(&self, address: u32) -> u16 {
        // Try overlay first
        if let Some(value) = self.megasd.ctrl_read_word(address) {
            return value;
        }
        
        // Fall back to cartridge
        if let Some(cart) = &self.megasd.cart {
            cart.read_word(address)
        } else {
            0xFFFF
        }
    }
    
    pub fn write_byte(&mut self, address: u32, value: u8) {
        // Try enhanced mapper
        if self.megasd.enhanced_ssf2_mapper_write(address, value) {
            return;
        }
        
        // Try ROM mapper
        if self.megasd.rom_mapper_write(address, value) {
            return;
        }
        
        // Try overlay
        if self.megasd.ctrl_write_byte(address, value) {
            return;
        }
        
        // Fall back to cartridge if writes enabled
        if self.megasd.write_enabled() {
            if let Some(cart) = &mut self.megasd.cart {
                cart.write_byte(address, value);
            }
        }
    }
    
    pub fn write_word(&mut self, address: u32, value: u16) {
        // Try overlay
        if self.megasd.ctrl_write_word(address, value) {
            return;
        }
        
        // Fall back to cartridge if writes enabled
        if self.megasd.write_enabled() {
            if let Some(cart) = &mut self.megasd.cart {
                cart.write_word(address, value);
            }
        }
    }
    
    pub fn pcm_write_byte(&mut self, address: u32, value: u8, cycles: u32) {
        self.megasd.pcm_write_byte(address, value, cycles);
    }
    
    pub fn pcm_write_word(&mut self, address: u32, value: u16, cycles: u32) {
        self.megasd.pcm_write_word(address, value, cycles);
    }
    
    pub fn pcm_read_byte(&self, address: u32, cycles: u32) -> u8 {
        self.megasd.pcm_read_byte(address, cycles).unwrap_or(0)
    }
    
    pub fn pcm_read_word(&self, address: u32, cycles: u32) -> u16 {
        self.megasd.pcm_read_word(address, cycles).unwrap_or(0)
    }
    
    pub fn update_cdda(&mut self, samples: u32) {
        self.megasd.update_cdda(samples);
    }
    
    pub fn get_megasd(&self) -> &MegaSD {
        &self.megasd
    }
    
    pub fn get_megasd_mut(&mut self) -> &mut MegaSD {
        &mut self.megasd
    }
}

// Implementation for CartridgeChip trait
impl crate::core::cartridge::chips::CartridgeChip for MegaSDMemoryHandler {
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
    
    fn reset(&mut self) {
        self.reset();
    }
    
    fn get_type(&self) -> crate::core::cartridge::chips::ChipType {
        crate::core::cartridge::chips::ChipType::MegaSD
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_megasd_new() {
        let megasd = MegaSD::new();
        assert_eq!(megasd.special, 0x07);
        assert!(!megasd.unlock);
        assert!(!megasd.write_enable);
    }
    
    #[test]
    fn test_megasd_ssf2_mapper() {
        let mut megasd = MegaSD::new();
        
        // Test unlock
        assert!(megasd.enhanced_ssf2_mapper_write(0x0, 0x80));
        assert!(megasd.unlock);
        
        // Test special bank register
        assert!(megasd.enhanced_ssf2_mapper_write(0xF, 0x80));
        assert_eq!(megasd.special, 0x80);
    }
    
    #[test]
    fn test_megasd_rom_mapper() {
        let mut megasd = MegaSD::new();
        
        // Test enable write
        assert!(megasd.rom_mapper_write(0xFF, b'W'));
        assert!(megasd.write_enable);
        
        // Test disable write
        assert!(megasd.rom_mapper_write(0xFF, b'X'));
        assert!(!megasd.write_enable);
        assert!(megasd.overlay_enable);
    }
    
    #[test]
    fn test_megasd_overlay_control() {
        let mut megasd = MegaSD::new();
        
        // Test overlay enable
        assert!(megasd.ctrl_write_word(0x03F7FA, 0xCD54));
        assert!(megasd.overlay_enable);
        
        // Test buffer write
        assert!(megasd.ctrl_write_byte(0x03F800, 0x42));
        assert_eq!(megasd.buffer[0], 0x42);
        
        // Test buffer read
        assert_eq!(megasd.ctrl_read_byte(0x03F800), Some(0x42));
    }
}