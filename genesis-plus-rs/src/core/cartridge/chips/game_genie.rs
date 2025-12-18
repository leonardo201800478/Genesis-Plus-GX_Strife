//! Game Genie (and Pro Action Replay) hardware support for Sega Genesis/Mega Drive
//! Based on original code by Eke-Eke and documentation from Charles McDonald
//! (http://cgfm2.emuviews.com/txt/genie.txt)

use crate::core::memory::{MemoryBus, MemoryError, MemoryResult};
use crate::core::cartridge::Cartridge;
use log::{info, warn, debug, trace};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Game Genie hardware state
#[derive(Debug, Clone)]
pub struct GameGenie {
    enabled: bool,
    regs: [u16; 0x20],      // 32 registers
    old_values: [u16; 6],   // Original ROM values for 6 patches
    patch_data: [u16; 6],   // Patch data for 6 patches
    patch_addrs: [u32; 6],  // Patch addresses (ROM offsets)
    lockrom: Vec<u8>,       // Game Genie ROM (32KB mirrored to 64KB)
    patches_enabled: bool,  // Are patches currently applied?
    mode_bit: bool,         // MODE bit (bit 10)
    read_enable_bit: bool,  // READ_ENABLE bit (bit 9)
    lock_bit: bool,         // LOCK bit (bit 8)
}

impl GameGenie {
    /// Creates a new Game Genie instance
    pub fn new() -> Self {
        Self {
            enabled: false,
            regs: [0; 0x20],
            old_values: [0; 6],
            patch_data: [0; 6],
            patch_addrs: [0; 6],
            lockrom: Vec::new(),
            patches_enabled: false,
            mode_bit: false,
            read_enable_bit: false,
            lock_bit: false,
        }
    }
    
    /// Initializes Game Genie hardware
    pub fn init(&mut self) -> bool {
        self.enabled = false;
        
        // Try to load Game Genie ROM file (32KB)
        if let Ok(rom) = self.load_rom("ggenie.bin") {
            if rom.len() == 0x8000 {
                // Byteswap ROM if little-endian
                let mut byteswapped = rom.clone();
                
                #[cfg(target_endian = "little")]
                {
                    for i in (0..0x8000).step_by(2) {
                        byteswapped.swap(i, i + 1);
                    }
                }
                
                // $0000-$7fff mirrored into $8000-$ffff
                let mut full_rom = byteswapped.clone();
                full_rom.extend_from_slice(&byteswapped);
                
                self.lockrom = full_rom;
                self.enabled = true;
                
                info!("Game Genie initialized (32KB ROM loaded)");
                return true;
            } else {
                warn!("Game Genie ROM must be 32KB (got {} bytes)", rom.len());
            }
        } else {
            debug!("Game Genie ROM not found, hardware disabled");
        }
        
        false
    }
    
    /// Loads Game Genie ROM from file
    fn load_rom<P: AsRef<Path>>(&self, path: P) -> std::io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        Ok(buffer)
    }
    
    /// Shuts down Game Genie hardware
    pub fn shutdown(&mut self) {
        if self.enabled {
            self.switch_patches(false);
            self.enabled = false;
            info!("Game Genie shutdown");
        }
    }
    
    /// Resets Game Genie state
    pub fn reset(&mut self, hard_reset: bool) {
        if !self.enabled {
            return;
        }
        
        // Reset any existing patches
        self.switch_patches(false);
        
        if hard_reset {
            // Reset internal state
            self.regs = [0; 0x20];
            self.old_values = [0; 6];
            self.patch_data = [0; 6];
            self.patch_addrs = [0; 6];
            self.patches_enabled = false;
            self.mode_bit = false;
            self.read_enable_bit = false;
            self.lock_bit = false;
            
            info!("Game Genie hard reset");
        } else {
            debug!("Game Genie soft reset");
        }
    }
    
    /// Enables or disables patches
    fn switch_patches(&mut self, enable: bool) {
        if enable == self.patches_enabled {
            return;
        }
        
        if enable {
            // Enable cheats
            for i in 0..6 {
                // Patch enabled?
                if (self.regs[0] & (1 << i)) != 0 {
                    // Save old value and patch ROM
                    // Note: Actual patching happens in memory read/write handlers
                    debug!("Game Genie: Enabling patch {} (addr: {:06X}, data: {:04X})", 
                           i, self.patch_addrs[i], self.patch_data[i]);
                }
            }
        } else {
            // Disable cheats in reverse order (in case same address used by multiple patches)
            for i in (0..6).rev() {
                if (self.regs[0] & (1 << i)) != 0 {
                    debug!("Game Genie: Disabling patch {}", i);
                }
            }
        }
        
        self.patches_enabled = enable;
    }
    
    /// Reads a byte from Game Genie registers
    pub fn read_byte(&self, address: u32) -> u8 {
        if !self.enabled {
            return 0xFF;
        }
        
        let offset = ((address >> 1) & 0x1F) as usize;
        let data = self.regs[offset];
        
        if (address & 1) != 0 {
            // Low byte
            (data & 0xFF) as u8
        } else {
            // High byte
            ((data >> 8) & 0xFF) as u8
        }
    }
    
    /// Reads a word from Game Genie registers
    pub fn read_word(&self, address: u32) -> u16 {
        if !self.enabled {
            return 0xFFFF;
        }
        
        let offset = ((address >> 1) & 0x1F) as usize;
        self.regs[offset]
    }
    
    /// Writes a byte to Game Genie registers
    pub fn write_byte(&mut self, address: u32, data: u8) {
        if !self.enabled {
            return;
        }
        
        let offset = ((address >> 1) & 0x1F) as usize;
        let current = self.regs[offset];
        
        let new_value = if (address & 1) != 0 {
            // Low byte write
            (current & 0xFF00) | (data as u16)
        } else {
            // High byte write
            (current & 0x00FF) | ((data as u16) << 8)
        };
        
        self.write_register(offset, new_value);
    }
    
    /// Writes a word to Game Genie registers
    pub fn write_word(&mut self, address: u32, data: u16) {
        if !self.enabled {
            return;
        }
        
        let offset = ((address >> 1) & 0x1F) as usize;
        self.write_register(offset, data);
    }
    
    /// Writes to a Game Genie register
    fn write_register(&mut self, offset: usize, data: u16) {
        // Update internal register
        self.regs[offset] = data;
        
        // Mode Register (offset 0)
        if offset == 0 {
            let old_mode = self.mode_bit;
            let old_read_enable = self.read_enable_bit;
            let old_lock = self.lock_bit;
            
            self.mode_bit = (data & 0x400) != 0;
            self.read_enable_bit = (data & 0x200) != 0;
            self.lock_bit = (data & 0x100) != 0;
            
            trace!("Game Genie: MODE={}, READ_ENABLE={}, LOCK={}", 
                   self.mode_bit as u8, self.read_enable_bit as u8, self.lock_bit as u8);
            
            // LOCK bit changed
            if self.lock_bit != old_lock {
                if self.lock_bit {
                    // LOCK bit set: decode patches and disable register writes
                    self.decode_patches();
                    self.switch_patches(true);
                } else {
                    // LOCK bit clear: enable register writes
                    self.switch_patches(false);
                }
            }
            
            // MODE or READ_ENABLE bits changed
            if (self.mode_bit != old_mode) || (self.read_enable_bit != old_read_enable) {
                // Memory mapping needs to be updated
                // This will be handled by the memory bus
                trace!("Game Genie: Memory mapping changed");
            }
        }
        // RESET register (offset 1)
        else if offset == 1 {
            self.regs[1] |= 1; // Set bit 0 on any write
        }
    }
    
    /// Decodes patch addresses and data from registers
    fn decode_patches(&mut self) {
        // Decode patch addresses (ROM area only)
        // Note: Charles's doc is wrong, first register holds bits 23-16 of patch address
        self.patch_addrs[0] = ((self.regs[2] as u32 & 0x3F) << 16) | self.regs[3] as u32;
        self.patch_addrs[1] = ((self.regs[5] as u32 & 0x3F) << 16) | self.regs[6] as u32;
        self.patch_addrs[2] = ((self.regs[8] as u32 & 0x3F) << 16) | self.regs[9] as u32;
        self.patch_addrs[3] = ((self.regs[11] as u32 & 0x3F) << 16) | self.regs[12] as u32;
        self.patch_addrs[4] = ((self.regs[14] as u32 & 0x3F) << 16) | self.regs[15] as u32;
        self.patch_addrs[5] = ((self.regs[17] as u32 & 0x3F) << 16) | self.regs[18] as u32;
        
        // Decode patch data
        self.patch_data[0] = self.regs[4];
        self.patch_data[1] = self.regs[7];
        self.patch_data[2] = self.regs[10];
        self.patch_data[3] = self.regs[13];
        self.patch_data[4] = self.regs[16];
        self.patch_data[5] = self.regs[19];
        
        debug!("Game Genie: Patches decoded");
        for i in 0..6 {
            if (self.regs[0] & (1 << i)) != 0 {
                debug!("  Patch {}: Addr={:06X}, Data={:04X}", 
                       i, self.patch_addrs[i], self.patch_data[i]);
            }
        }
    }
    
    /// Checks if an address should be patched by Game Genie
    pub fn check_patch(&self, address: u32, original_data: u16) -> Option<u16> {
        if !self.enabled || !self.patches_enabled {
            return None;
        }
        
        // Check each patch
        for i in 0..6 {
            // Patch enabled?
            if (self.regs[0] & (1 << i)) != 0 {
                // Address matches?
                if address == self.patch_addrs[i] {
                    trace!("Game Genie: Patching {:06X} from {:04X} to {:04X}", 
                           address, original_data, self.patch_data[i]);
                    return Some(self.patch_data[i]);
                }
            }
        }
        
        None
    }
    
    /// Returns the Game Genie ROM data
    pub fn get_rom(&self) -> &[u8] {
        &self.lockrom
    }
    
    /// Returns whether Game Genie is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    
    /// Returns whether patches are currently applied
    pub fn patches_active(&self) -> bool {
        self.patches_enabled
    }
    
    /// Returns the MODE bit state
    pub fn mode_bit(&self) -> bool {
        self.mode_bit
    }
    
    /// Returns the READ_ENABLE bit state
    pub fn read_enable_bit(&self) -> bool {
        self.read_enable_bit
    }
    
    /// Returns the LOCK bit state
    pub fn lock_bit(&self) -> bool {
        self.lock_bit
    }
    
    /// Saves Game Genie state for save states
    pub fn save_state(&self) -> Vec<u8> {
        let mut state = Vec::new();
        
        // Save enabled flag
        state.push(self.enabled as u8);
        state.push(self.patches_enabled as u8);
        state.push(self.mode_bit as u8);
        state.push(self.read_enable_bit as u8);
        state.push(self.lock_bit as u8);
        
        // Save registers
        for reg in self.regs.iter() {
            state.extend_from_slice(&reg.to_le_bytes());
        }
        
        // Save patch data
        for data in self.patch_data.iter() {
            state.extend_from_slice(&data.to_le_bytes());
        }
        
        // Save patch addresses
        for addr in self.patch_addrs.iter() {
            state.extend_from_slice(&addr.to_le_bytes());
        }
        
        // Save old values
        for old in self.old_values.iter() {
            state.extend_from_slice(&old.to_le_bytes());
        }
        
        state
    }
    
    /// Loads Game Genie state from save state
    pub fn load_state(&mut self, data: &[u8]) -> bool {
        if data.len() < 1 + (0x20 * 2) + (6 * 2) + (6 * 4) + (6 * 2) {
            return false;
        }
        
        let mut offset = 0;
        
        // Load flags
        self.enabled = data[offset] != 0; offset += 1;
        self.patches_enabled = data[offset] != 0; offset += 1;
        self.mode_bit = data[offset] != 0; offset += 1;
        self.read_enable_bit = data[offset] != 0; offset += 1;
        self.lock_bit = data[offset] != 0; offset += 1;
        
        // Load registers
        for i in 0..0x20 {
            self.regs[i] = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
        }
        
        // Load patch data
        for i in 0..6 {
            self.patch_data[i] = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
        }
        
        // Load patch addresses
        for i in 0..6 {
            let bytes = [
                data[offset], data[offset + 1], 
                data[offset + 2], data[offset + 3]
            ];
            self.patch_addrs[i] = u32::from_le_bytes(bytes);
            offset += 4;
        }
        
        // Load old values
        for i in 0..6 {
            self.old_values[i] = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;
        }
        
        true
    }
}

/// Integration with the memory system
pub struct GameGenieMemoryHandler {
    game_genie: GameGenie,
    cart: Option<Box<dyn Cartridge>>,
}

impl GameGenieMemoryHandler {
    pub fn new() -> Self {
        Self {
            game_genie: GameGenie::new(),
            cart: None,
        }
    }
    
    pub fn init(&mut self, cart: Box<dyn Cartridge>) -> bool {
        self.cart = Some(cart);
        self.game_genie.init()
    }
    
    pub fn read_byte(&self, address: u32) -> u8 {
        if self.game_genie.is_enabled() {
            // Check if address is in Game Genie register space
            if address < 0x20 {
                return self.game_genie.read_byte(address);
            }
            
            // Check if MODE bit is clear (Game Genie ROM mapped)
            if !self.game_genie.mode_bit() && address < 0x8000 {
                // Game Genie ROM mapped at $0000-$7FFF
                let rom = self.game_genie.get_rom();
                let rom_addr = address as usize;
                if rom_addr < rom.len() {
                    return rom[rom_addr];
                }
            }
        }
        
        // Fall back to cartridge
        if let Some(cart) = &self.cart {
            cart.read_byte(address)
        } else {
            0xFF
        }
    }
    
    pub fn read_word(&self, address: u32) -> u16 {
        if self.game_genie.is_enabled() {
            // Check if address is in Game Genie register space
            if address < 0x20 {
                return self.game_genie.read_word(address);
            }
            
            // Check if MODE bit is clear (Game Genie ROM mapped)
            if !self.game_genie.mode_bit() && address < 0x8000 {
                // Game Genie ROM mapped at $0000-$7FFF
                let rom = self.game_genie.get_rom();
                let rom_addr = address as usize;
                if rom_addr + 1 < rom.len() {
                    let low = rom[rom_addr] as u16;
                    let high = rom[rom_addr + 1] as u16;
                    return (high << 8) | low;
                }
            }
            
            // Check for patches
            if address >= 0x000000 && address <= 0x3FFFFF {
                // ROM area
                if let Some(cart) = &self.cart {
                    let original = cart.read_word(address);
                    if let Some(patched) = self.game_genie.check_patch(address, original) {
                        return patched;
                    }
                    return original;
                }
            }
        }
        
        // Fall back to cartridge
        if let Some(cart) = &self.cart {
            cart.read_word(address)
        } else {
            0xFFFF
        }
    }
    
    pub fn write_byte(&mut self, address: u32, value: u8) {
        if self.game_genie.is_enabled() {
            // Game Genie registers are writeable
            if address < 0x20 {
                self.game_genie.write_byte(address, value);
                return;
            }
        }
        
        // Pass through to cartridge
        if let Some(cart) = &mut self.cart {
            cart.write_byte(address, value);
        }
    }
    
    pub fn write_word(&mut self, address: u32, value: u16) {
        if self.game_genie.is_enabled() {
            // Game Genie registers are writeable
            if address < 0x20 {
                self.game_genie.write_word(address, value);
                return;
            }
        }
        
        // Pass through to cartridge
        if let Some(cart) = &mut self.cart {
            cart.write_word(address, value);
        }
    }
    
    pub fn reset(&mut self, hard_reset: bool) {
        self.game_genie.reset(hard_reset);
    }
    
    pub fn shutdown(&mut self) {
        self.game_genie.shutdown();
    }
    
    pub fn get_game_genie(&self) -> &GameGenie {
        &self.game_genie
    }
    
    pub fn get_game_genie_mut(&mut self) -> &mut GameGenie {
        &mut self.game_genie
    }
}

// Implementation for the CartridgeChip trait
impl crate::core::cartridge::chips::CartridgeChip for GameGenieMemoryHandler {
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
        self.reset(true);
    }
    
    fn get_type(&self) -> crate::core::cartridge::chips::ChipType {
        crate::core::cartridge::chips::ChipType::GameGenie
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_game_genie_new() {
        let gg = GameGenie::new();
        assert!(!gg.is_enabled());
        assert!(!gg.patches_active());
    }
    
    #[test]
    fn test_game_genie_registers() {
        let mut gg = GameGenie::new();
        gg.enabled = true;
        
        // Test byte writes/reads
        gg.write_byte(0x00, 0x12); // High byte of register 0
        gg.write_byte(0x01, 0x34); // Low byte of register 0
        assert_eq!(gg.read_word(0x00), 0x1234);
        
        // Test word write/read
        gg.write_word(0x02, 0x5678);
        assert_eq!(gg.read_word(0x02), 0x5678);
        assert_eq!(gg.read_byte(0x02), 0x56); // High byte
        assert_eq!(gg.read_byte(0x03), 0x78); // Low byte
    }
    
    #[test]
    fn test_patch_decoding() {
        let mut gg = GameGenie::new();
        gg.enabled = true;
        
        // Set up a patch in registers
        // Patch 0: address = 0x123456, data = 0xABCD
        gg.regs[2] = 0x0012; // Bits 23-16 (masked to 0x3F)
        gg.regs[3] = 0x3456; // Bits 15-0
        gg.regs[4] = 0xABCD; // Patch data
        
        // Enable patch 0
        gg.regs[0] = 0x0001;
        
        // Set LOCK bit to decode patches
        gg.write_word(0x00, 0x0100);
        
        assert!(gg.lock_bit());
        assert_eq!(gg.patch_addrs[0], 0x123456);
        assert_eq!(gg.patch_data[0], 0xABCD);
    }
}