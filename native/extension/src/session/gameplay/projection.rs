use ext_php_rs::binary::Binary;

use crate::session::view::ChunkViewDelta;

pub(super) fn encode_view_delta(
    delta: Option<&ChunkViewDelta>,
) -> Result<Binary<u8>, &'static str> {
    let Some(delta) = delta else {
        return Ok(Binary::new(Vec::new()));
    };
    let entering_count =
        u32::try_from(delta.entering.len()).map_err(|_| "chunk view delta is too large")?;
    let mut projection = Vec::with_capacity(28 + delta.entering.len().saturating_mul(8));
    for value in [
        delta.from_center.x(),
        delta.from_center.z(),
        delta.to_center.x(),
        delta.to_center.z(),
        delta.from_radius,
        delta.to_radius,
    ] {
        projection.extend_from_slice(&value.to_le_bytes());
    }
    projection.extend_from_slice(&entering_count.to_le_bytes());
    for position in &delta.entering {
        projection.extend_from_slice(&position.x().to_le_bytes());
        projection.extend_from_slice(&position.z().to_le_bytes());
    }
    Ok(Binary::new(projection))
}

#[cfg(test)]
mod tests {
    use cobblestone_core::ChunkCoord;

    use super::*;

    #[test]
    fn view_delta_projection_is_little_endian_and_ordered() {
        let delta = ChunkViewDelta {
            from_center: ChunkCoord::new(-1, 2),
            to_center: ChunkCoord::new(3, -4),
            from_radius: 2,
            to_radius: 3,
            entering: vec![ChunkCoord::new(5, 6), ChunkCoord::new(-7, 8)],
            leaving: Vec::new(),
        };

        let projection = encode_view_delta(Some(&delta)).expect("encode view delta");
        let mut expected = Vec::new();
        for value in [-1_i32, 2, 3, -4, 2, 3] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        expected.extend_from_slice(&2_u32.to_le_bytes());
        for coordinate in [5_i32, 6, -7, 8] {
            expected.extend_from_slice(&coordinate.to_le_bytes());
        }

        assert_eq!(projection.as_slice(), expected.as_slice());
        assert!(
            encode_view_delta(None)
                .expect("encode empty view delta")
                .as_slice()
                .is_empty()
        );
    }
}
