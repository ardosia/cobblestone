pub const MAX_LEGACY_STATE_ID: u16 = 0x0fff;

pub const BLOCK_IDS: &[u8] = &[
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50,
    51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74,
    75, 76, 77, 78, 79, 80, 81, 82, 83, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99,
    100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118,
    120, 121, 123, 124, 125, 126, 127, 128, 129, 131, 132, 133, 134, 135, 136, 139, 140, 141, 142,
    143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 161, 162,
    163, 164, 165, 167, 170, 171, 172, 173, 174, 175, 178, 179, 180, 181, 182, 183, 184, 185, 186,
    187, 193, 194, 195, 196, 197, 198, 199, 243, 244, 245, 246, 247, 248, 249, 250, 255,
];

pub const fn block_id_is_supported(id: u8) -> bool {
    matches!(
        id,
        0..=35
            | 37..=83
            | 85..=118
            | 120..=121
            | 123..=129
            | 131..=136
            | 139..=159
            | 161..=165
            | 167
            | 170..=175
            | 178..=187
            | 193..=199
            | 243..=250
            | 255
    )
}

pub const fn block_state_id_is_supported(state: u16) -> bool {
    state <= MAX_LEGACY_STATE_ID && block_id_is_supported((state >> 4) as u8)
}

#[cfg(test)]
mod tests {
    use super::{
        BLOCK_IDS, MAX_LEGACY_STATE_ID, block_id_is_supported, block_state_id_is_supported,
    };

    #[test]
    fn fixed_target_block_id_catalog_is_exact() {
        assert_eq!(BLOCK_IDS.len(), 191);
        for id in 0_u8..=u8::MAX {
            assert_eq!(
                block_id_is_supported(id),
                BLOCK_IDS.binary_search(&id).is_ok(),
                "block id {id} support mismatch"
            );
        }
        assert!(!block_id_is_supported(36));
        assert!(!block_id_is_supported(84));
        assert!(!block_id_is_supported(251));
        assert!(block_id_is_supported(255));
    }

    #[test]
    fn fixed_target_state_validation_keeps_legacy_layout() {
        assert!(block_state_id_is_supported(0));
        assert!(block_state_id_is_supported((50 << 4) | 15));
        assert!(block_state_id_is_supported((255 << 4) | 15));
        assert!(!block_state_id_is_supported(36 << 4));
        assert!(!block_state_id_is_supported(251 << 4));
        assert!(!block_state_id_is_supported(MAX_LEGACY_STATE_ID + 1));
    }
}
