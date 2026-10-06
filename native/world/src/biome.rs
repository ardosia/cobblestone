use cobblestone_target::BiomeId;

pub const BIOME_COLOR_MASK: u32 = 0x00ff_ffff;

pub fn biome_id_is_supported(id: u8) -> bool {
    BiomeId::from_raw(id).is_supported()
}

pub fn default_biome_word(id: u8) -> Option<u32> {
    BiomeId::from_raw(id)
        .default_color()
        .map(|color| (u32::from(id) << 24) | color)
}

pub const fn biome_id(word: u32) -> u8 {
    (word >> 24) as u8
}

pub const fn biome_color(word: u32) -> u32 {
    word & BIOME_COLOR_MASK
}

pub const fn with_biome_id(word: u32, id: u8) -> u32 {
    ((id as u32) << 24) | biome_color(word)
}

pub const fn with_biome_color(word: u32, color: u32) -> u32 {
    (word & 0xff00_0000) | (color & BIOME_COLOR_MASK)
}

#[cfg(test)]
mod tests {
    use cobblestone_target::{BIOME_IDS, BiomeId};

    use super::*;

    #[test]
    fn exact_fixed_target_catalog_has_expected_boundaries_and_defaults() {
        assert_eq!(BIOME_IDS.len(), 60);
        assert!(biome_id_is_supported(BiomeId::OCEAN.raw()));
        assert!(biome_id_is_supported(BiomeId::MESA_PLATEAU_M.raw()));
        assert!(!biome_id_is_supported(9));
        assert!(!biome_id_is_supported(44));
        assert!(!biome_id_is_supported(168));
        assert_eq!(default_biome_word(1), Some(0x0192_bc59));
        assert_eq!(default_biome_word(6), Some(0x066a_7039));
        assert_eq!(default_biome_word(37), Some(0x2590_814d));
    }

    #[test]
    fn id_and_color_edits_are_independent() {
        let word = 0x0412_3456;
        assert_eq!(biome_id(word), 4);
        assert_eq!(biome_color(word), 0x123456);
        assert_eq!(with_biome_id(word, 2), 0x0212_3456);
        assert_eq!(with_biome_color(word, 0xabcdef), 0x04ab_cdef);
    }
}
