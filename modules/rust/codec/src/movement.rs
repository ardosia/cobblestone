use crate::CodecError;
use crate::binary::Reader;

/// Protocol-84 MovePlayer movement mode.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MovePlayerMode {
    Normal,
    Reset,
    Rotation,
}

impl MovePlayerMode {
    pub const fn wire_value(self) -> u8 {
        match self {
            Self::Normal => 0,
            Self::Reset => 1,
            Self::Rotation => 2,
        }
    }

    fn decode(value: u8) -> Result<Self, CodecError> {
        match value {
            0 => Ok(Self::Normal),
            1 => Ok(Self::Reset),
            2 => Ok(Self::Rotation),
            _ => Err(CodecError::InvalidFixedTargetValue {
                field: "MovePlayer mode",
                value: u64::from(value),
                max: 2,
            }),
        }
    }
}

/// Decoded MCPE 0.15.10 / protocol-84 MovePlayer body.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct MovePlayerPacket {
    entity_id: i64,
    position: [f32; 3],
    yaw: f32,
    body_yaw: f32,
    pitch: f32,
    mode: MovePlayerMode,
    on_ground: bool,
}

impl MovePlayerPacket {
    pub const fn entity_id(self) -> i64 {
        self.entity_id
    }

    pub const fn position(self) -> [f32; 3] {
        self.position
    }

    pub const fn yaw(self) -> f32 {
        self.yaw
    }

    pub const fn body_yaw(self) -> f32 {
        self.body_yaw
    }

    pub const fn pitch(self) -> f32 {
        self.pitch
    }

    pub const fn mode(self) -> MovePlayerMode {
        self.mode
    }

    pub const fn on_ground(self) -> bool {
        self.on_ground
    }
}

/// Decodes a protocol-84 MovePlayer packet body.
///
/// PocketMine's pinned 0.15.10 oracle reads the entity id and six floats big-endian,
/// followed by one movement-mode byte and one on-ground byte.
pub fn decode_protocol84_move_player(body: &[u8]) -> Result<MovePlayerPacket, CodecError> {
    let mut reader = Reader::new(body);
    let packet = MovePlayerPacket {
        entity_id: reader.read_i64_be()?,
        position: [
            reader.read_f32_be()?,
            reader.read_f32_be()?,
            reader.read_f32_be()?,
        ],
        yaw: reader.read_f32_be()?,
        body_yaw: reader.read_f32_be()?,
        pitch: reader.read_f32_be()?,
        mode: MovePlayerMode::decode(reader.read_u8()?)?,
        on_ground: reader.read_u8()? != 0,
    };
    reader.finish()?;
    Ok(packet)
}
