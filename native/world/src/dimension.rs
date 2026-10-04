#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum DimensionId {
    Overworld = 0,
    Nether = 1,
}

impl DimensionId {
    pub const fn has_sky(self) -> bool {
        matches!(self, Self::Overworld)
    }
}

impl From<DimensionId> for u8 {
    fn from(value: DimensionId) -> Self {
        value as u8
    }
}

impl TryFrom<u8> for DimensionId {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Overworld),
            1 => Ok(Self::Nether),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DimensionId;

    #[test]
    fn fixed_target_dimension_domain_is_exact() {
        assert_eq!(u8::from(DimensionId::Overworld), 0);
        assert_eq!(u8::from(DimensionId::Nether), 1);
        assert!(DimensionId::Overworld.has_sky());
        assert!(!DimensionId::Nether.has_sky());
        assert_eq!(DimensionId::try_from(0), Ok(DimensionId::Overworld));
        assert_eq!(DimensionId::try_from(1), Ok(DimensionId::Nether));
        assert_eq!(DimensionId::try_from(2), Err(2));
    }
}
