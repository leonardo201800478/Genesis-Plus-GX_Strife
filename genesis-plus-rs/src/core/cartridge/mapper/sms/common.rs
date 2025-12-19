//! Funções comuns para mappers SMS

use super::*;

/// Escreve em mapeador de 8KB
pub fn mapper_8k_write(slot: &mut MemorySlot, offset: usize, data: u8, memory: &mut MemoryMap) {
    let page = (data as usize % slot.pages as usize) << 13;
    let page_ptr = unsafe { slot.rom.add(page) };
    
    slot.fcr[offset & 3] = data;
    
    match offset & 3 {
        0 => { // $8000-$9FFF
            for i in 0x20..0x28 {
                memory.read_map[i] = unsafe { page_ptr.add((i & 0x07) << 10) };
            }
        }
        1 => { // $A000-$BFFF
            for i in 0x28..0x30 {
                memory.read_map[i] = unsafe { page_ptr.add((i & 0x07) << 10) };
            }
        }
        2 => { // $4000-$5FFF
            for i in 0x10..0x18 {
                memory.read_map[i] = unsafe { page_ptr.add((i & 0x07) << 10) };
            }
        }
        3 => { // $6000-$7FFF
            for i in 0x18..0x20 {
                memory.read_map[i] = unsafe { page_ptr.add((i & 0x07) << 10) };
            }
        }
        _ => {}
    }
}

/// Escreve em mapeador de 16KB
pub fn mapper_16k_write(slot: &mut MemorySlot, offset: usize, data: u8, memory: &mut MemoryMap) {
    let mut page = (data as usize % slot.pages as usize) as u8;
    
    // Incremento de página (somente mapper SEGA oficial)
    if slot.mapper == MAPPER_SEGA && slot.fcr[0] & 0x03 != 0 {
        page = (page + ((4 - (slot.fcr[0] & 0x03)) << 3)) % slot.pages as u8;
    }
    
    slot.fcr[offset] = data;
    
    match offset {
        0 => { // Registro de controle
            if data & 0x08 != 0 {
                // RAM externa em $8000-$BFFF
            } else {
                // ROM em $8000-$BFFF
            }
            
            if data & 0x10 != 0 {
                // RAM externa em $C000-$FFFF
            } else {
                // RAM interna em $C000-$FFFF
            }
        }
        1 => { // $0000-$3FFF
            for i in 1..0x10 {
                let addr = (page as usize) << 14 | ((i & 0x0F) << 10);
                memory.read_map[i] = unsafe { slot.rom.add(addr) };
            }
        }
        2 => { // $4000-$7FFF
            for i in 0x10..0x20 {
                let addr = (page as usize) << 14 | ((i & 0x0F) << 10);
                memory.read_map[i] = unsafe { slot.rom.add(addr) };
            }
        }
        3 => { // $8000-$BFFF
            if (slot.mapper == MAPPER_SEGA || slot.mapper == MAPPER_SEGA_X) && slot.fcr[0] & 0x08 != 0 {
                return; // RAM mapeada
            }
            
            for i in 0x20..0x30 {
                let addr = (page as usize) << 14 | ((i & 0x0F) << 10);
                memory.read_map[i] = unsafe { slot.rom.add(addr) };
            }
        }
        _ => {}
    }
}

/// Escreve em mapeador de 32KB
pub fn mapper_32k_write(slot: &mut MemorySlot, data: u8, memory: &mut MemoryMap) {
    let page = (data as usize % slot.pages as usize) << 15;
    let page_ptr = unsafe { slot.rom.add(page) };
    
    slot.fcr[0] = data;
    
    if slot.mapper == MAPPER_MULTI_32K_16K {
        // Modo Multi 32K/16K
        match slot.fcr[1] & 0x0F {
            0x00 | 0x01 | 0x02 | 0x03 => {
                // Mapeamento variável
            }
            _ => {
                // Mirror da última página
            }
        }
    } else {
        // Mapeamento simples de 32KB
        for i in 0x00..0x20 {
            memory.read_map[i] = unsafe { page_ptr.add(i << 10) };
        }
        
        // Mirror em $8000-$BFFF
        for i in 0x20..0x30 {
            memory.read_map[i] = memory.read_map[i & 0x0F];
        }
    }
}