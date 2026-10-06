//! Fixed-target identities and layout for Minecraft: Windows 10 Edition Beta / MCPE 0.15.10.
//!
//! The human-authored source lives under `spec/`; generated modules stay private so consumers use
//! the flat API exported from this crate.

mod generated;

pub use generated::biomes::BIOME_IDS;
pub use generated::blocks::{INTERNAL_BLOCK_IDS, PUBLIC_BLOCK_IDS};
pub use generated::target::{
    BLOCK_DATA_BITS, ChunkShape, GAME_PROTOCOL, GAME_VERSION, GAME_VERSION_MAJOR,
    GAME_VERSION_MINOR, GAME_VERSION_PATCH, MAX_BLOCK_STATE_ID, RAKNET_PROTOCOL,
};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BlockId(u8);

impl BlockId {
    #[must_use]
    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BiomeId(u8);

impl BiomeId {
    #[must_use]
    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_target_identity_is_exact() {
        assert_eq!(GAME_VERSION, "0.15.10");
        assert_eq!(GAME_PROTOCOL, 84);
        assert_eq!(RAKNET_PROTOCOL, 8);
        assert_eq!(ChunkShape::EDGE, 16);
        assert_eq!(ChunkShape::HEIGHT, 128);
        assert_eq!(ChunkShape::BLOCK_COUNT, 32_768);
        assert_eq!(ChunkShape::NIBBLE_BYTES, 16_384);
        assert_eq!(BLOCK_DATA_BITS, 4);
        assert_eq!(MAX_BLOCK_STATE_ID, 0x0fff);
    }

    #[test]
    fn generated_catalogs_keep_public_and_internal_boundaries() {
        assert_eq!(PUBLIC_BLOCK_IDS.len(), 191);
        assert_eq!(INTERNAL_BLOCK_IDS, &[BlockId::END_PORTAL]);
        assert!(BlockId::CHEST.is_public());
        assert!(!BlockId::END_PORTAL.is_public());
        assert!(BlockId::END_PORTAL.is_world_supported());
        assert_eq!(BlockId::CHEST.asset_name(), Some("chest"));
        assert_eq!(BlockId::END_PORTAL.asset_name(), None);
        assert_eq!(BIOME_IDS.len(), 60);
        assert!(BiomeId::PLAINS.is_supported());
        assert!(!BiomeId::from_raw(9).is_supported());
        assert_eq!(BiomeId::PLAINS.default_color(), Some(0x92bc59));
    }
}
