//! Sistema de Memória do Genesis/Mega Drive.
//! Gerencia o barramento de endereços de 24-bit, mapeamento de cartuchos,
//! Save RAM e interface com todos os componentes do sistema.

pub mod bus;
pub mod cart;
pub mod map;
pub mod sram;

// Re-exportações para facilitar o uso
pub use bus::MemoryBus;
pub use cart::{Cartridge, MapperType};
pub use map::{MemRegion, MemoryHandler};
pub use sram::SaveRam;

/// Máscara de endereço válido para o barramento Genesis (24-bit = 16 MB)
pub const ADDRESS_MASK: u32 = 0x00FF_FFFF;

/// Tamanho máximo de ROM suportado (4MB + 1MB de backup para mappers)
pub const MAX_ROM_SIZE: usize = 5 * 1024 * 1024;

/// Tamanho máximo de Save RAM
pub const MAX_SRAM_SIZE: usize = 64 * 1024;

/// Erros do sistema de memória
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryError {
    InvalidAddress,
    RomTooLarge,
    InvalidCartridge,
    SaveError,
}

/// Tipo de resultado para operações de memória
pub type MemoryResult<T> = Result<T, MemoryError>;
