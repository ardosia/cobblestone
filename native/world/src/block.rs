use cobblestone_target::{BlockId, MAX_BLOCK_STATE_ID};

pub const MAX_LEGACY_STATE_ID: u16 = MAX_BLOCK_STATE_ID;

pub const fn block_id_is_supported(id: u8) -> bool {
    BlockId::from_raw(id).is_public()
}

pub const fn block_state_id_is_supported(state: u16) -> bool {
    state <= MAX_LEGACY_STATE_ID && block_id_is_supported((state >> 4) as u8)
}

pub const fn world_block_id_is_supported(id: u8) -> bool {
    BlockId::from_raw(id).is_world_supported()
}

pub const fn world_block_state_id_is_supported(state: u16) -> bool {
    state <= MAX_LEGACY_STATE_ID && world_block_id_is_supported((state >> 4) as u8)
}

#[cfg(test)]
mod tests {
    use cobblestone_target::{BlockId, INTERNAL_BLOCK_IDS, PUBLIC_BLOCK_IDS};

    use super::{
        MAX_LEGACY_STATE_ID, block_id_is_supported, block_state_id_is_supported,
        world_block_id_is_supported, world_block_state_id_is_supported,
    };

    #[test]
    fn fixed_target_block_id_catalog_is_exact() {
        assert_eq!(PUBLIC_BLOCK_IDS.len(), 191);
        for id in 0_u8..=u8::MAX {
            assert_eq!(
                block_id_is_supported(id),
                PUBLIC_BLOCK_IDS
                    .binary_search(&BlockId::from_raw(id))
                    .is_ok(),
                "block id {id} support mismatch"
            );
        }
        assert!(!block_id_is_supported(36));
        assert!(!block_id_is_supported(84));
        assert!(!block_id_is_supported(251));
        assert!(block_id_is_supported(255));
    }

    #[test]
    fn hidden_registered_world_blocks_do_not_expand_public_catalog() {
        assert_eq!(INTERNAL_BLOCK_IDS, &[BlockId::END_PORTAL]);
        assert!(!block_id_is_supported(BlockId::END_PORTAL.raw()));
        assert!(!block_state_id_is_supported(
            u16::from(BlockId::END_PORTAL.raw()) << 4
        ));
        assert!(world_block_id_is_supported(BlockId::END_PORTAL.raw()));
        assert!(world_block_state_id_is_supported(
            u16::from(BlockId::END_PORTAL.raw()) << 4
        ));
        assert!(!world_block_id_is_supported(122));
        assert!(!world_block_state_id_is_supported(122 << 4));
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
