// Adicione ao arquivo:
pub mod svp;

// No enum ChipType:
SVP, // Sega Virtua Processor

// Na função de criação de chips:
pub fn create_chip(chip_type: ChipType) -> Box<dyn CartridgeChip> {
    match chip_type {
        ChipType::SVP => Box::new(svp::SVP::new()),
        // ... outros chips
    }
}