pub const BIOME_COLOR_MASK: u32 = 0x00ff_ffff;

const DEFAULT_BIOMES: &[(u8, u32)] = &[
    (0, 0x8e_b8_71),
    (1, 0x92_bc_59),
    (2, 0xbf_b6_55),
    (3, 0x8a_b5_89),
    (4, 0x7a_c0_5b),
    (5, 0x87_b6_84),
    (6, 0x6a_70_39),
    (7, 0x8e_b8_71),
    (8, 0x00_00_00),
    (10, 0x80_b4_97),
    (11, 0x80_b4_97),
    (12, 0x80_b4_97),
    (13, 0x80_b4_97),
    (14, 0x56_cb_40),
    (15, 0x56_cb_40),
    (16, 0x92_bc_59),
    (17, 0xbf_b6_55),
    (18, 0x7a_c0_5b),
    (19, 0x87_b6_84),
    (20, 0x8a_b5_89),
    (21, 0x59_cb_3c),
    (22, 0x59_cb_3c),
    (23, 0x64_c9_40),
    (24, 0x8e_b8_71),
    (25, 0x8a_b5_89),
    (26, 0x83_b4_93),
    (27, 0x89_bb_67),
    (28, 0x89_bb_67),
    (29, 0x517a32),
    (30, 0x80_b4_97),
    (31, 0x80_b4_97),
    (32, 0x87_b7_80),
    (33, 0x87_b7_80),
    (34, 0x8a_b5_89),
    (35, 0xbf_b6_55),
    (36, 0xbf_b6_55),
    (37, 0x90_81_4d),
    (38, 0x90_81_4d),
    (39, 0x90_81_4d),
    (129, 0x92_bc_59),
    (130, 0xbf_b6_55),
    (131, 0x8a_b5_89),
    (132, 0x7a_c0_5b),
    (133, 0x87_b6_84),
    (134, 0x6a_70_39),
    (140, 0x80_b4_97),
    (149, 0x59_cb_3c),
    (151, 0x64_c9_40),
    (155, 0x7a_c0_5b),
    (156, 0x7a_c0_5b),
    (157, 0x517a32),
    (158, 0x80_b4_97),
    (160, 0x87_b6_84),
    (161, 0x87_b7_80),
    (162, 0x8a_b5_89),
    (163, 0x83_c3_44),
    (164, 0x83_c3_44),
    (165, 0x90_81_4d),
    (166, 0x90_81_4d),
    (167, 0x90_81_4d),
];

pub fn biome_id_is_supported(id: u8) -> bool {
    DEFAULT_BIOMES
        .binary_search_by_key(&id, |(candidate, _)| *candidate)
        .is_ok()
}

pub fn default_biome_word(id: u8) -> Option<u32> {
    DEFAULT_BIOMES
        .binary_search_by_key(&id, |(candidate, _)| *candidate)
        .ok()
        .map(|index| (u32::from(id) << 24) | DEFAULT_BIOMES[index].1)
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
    use super::*;

    #[test]
    fn exact_fixed_target_catalog_has_expected_boundaries_and_defaults() {
        assert_eq!(DEFAULT_BIOMES.len(), 60);
        assert!(biome_id_is_supported(0));
        assert!(biome_id_is_supported(167));
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
