// Este é o ponto de entrada principal da biblioteca.
// O atributo `no_std` é necessário para compatibilidade com a API libretro.
#![cfg_attr(feature = "libretro", no_std)]
#![cfg_attr(feature = "libretro", feature(lang_items))]

// Para compilação como core libretro, precisamos do allocator.
#[cfg(feature = "libretro")]
extern crate alloc;

// Módulos principais do projeto.
pub mod core;
pub mod utils;
pub mod simd;
pub mod gpu;

// A implementação da API libretro.
#[cfg(feature = "libretro")]
pub mod libretro;

// Re-exportações para facilitar o uso.
pub use core::system::GenesisSystem;
pub use core::cpu::{M68K, Z80};
pub use core::vdp::VDP;
pub use core::audio::{YM2612, SN76489};

/// Versão do emulador.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Função conveniente para criar uma nova instância do sistema.
pub fn create_system() -> GenesisSystem {
    GenesisSystem::new()
}