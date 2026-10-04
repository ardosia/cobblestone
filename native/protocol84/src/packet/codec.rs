use cobblestone_runtime::NativeBuffer;

use crate::batch::{BatchPacket, compress_zlib, decompress_zlib_limited};
use crate::binary::{Reader, Writer};
use crate::frame::{RawPacket, check_limit};
use crate::{
    CodecError, CodecLimits, LOGIN_MAX_DECOMPRESSED_BYTES, LimitKind, decode_game_frame,
    encode_game_frame,
};

use super::*;

/// Decodes one outer 0xfe game frame into the implemented bootstrap subset.
pub fn decode_bootstrap_frame(
    input: &[u8],
    limits: CodecLimits,
) -> Result<BootstrapPacket, CodecError> {
    let raw = decode_game_frame(input, limits)?;
    decode_bootstrap_packet(raw, limits)
}

/// Encodes one implemented bootstrap packet to its raw protocol-84 packet representation.
pub fn encode_bootstrap_packet(
    packet: &BootstrapPacket,
    limits: CodecLimits,
) -> Result<RawPacket, CodecError> {
    let raw = match packet {
        BootstrapPacket::Login(packet) => {
            RawPacket::new(packet_id::LOGIN, encode_login(packet, limits)?)
        }
        BootstrapPacket::PlayStatus(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.status());
            RawPacket::new(
                packet_id::PLAY_STATUS,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::Disconnect(packet) => {
            let mut writer = Writer::new();
            writer.put_string_u16("disconnect message", packet.message())?;
            RawPacket::new(
                packet_id::DISCONNECT,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::SetTime(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.time());
            writer.put_u8(u8::from(packet.started()));
            RawPacket::new(
                packet_id::SET_TIME,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::StartGame(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.seed);
            writer.put_u8(packet.dimension.into());
            writer.put_i32_be(packet.generator);
            writer.put_i32_be(packet.gamemode);
            writer.put_i64_be(packet.entity_id);
            for coordinate in packet.spawn {
                writer.put_i32_be(coordinate);
            }
            for coordinate in packet.position {
                writer.put_f32_be(coordinate);
            }
            writer.put_u8(1);
            writer.put_u8(1);
            writer.put_u8(0);
            writer.put_string_u16("start-game level id", &packet.level_id)?;
            RawPacket::new(
                packet_id::START_GAME,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::SetSpawnPosition(packet) => {
            let mut writer = Writer::new();
            for coordinate in packet.position() {
                writer.put_i32_be(coordinate);
            }
            RawPacket::new(
                packet_id::SET_SPAWN_POSITION,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::AdventureSettings(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.flags());
            writer.put_i32_be(packet.user_permission());
            writer.put_i32_be(packet.global_permission());
            RawPacket::new(
                packet_id::ADVENTURE_SETTINGS,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::SetDifficulty(packet) => {
            let mut writer = Writer::new();
            writer.put_i32_be(packet.difficulty());
            RawPacket::new(
                packet_id::SET_DIFFICULTY,
                NativeBuffer::from_vec(writer.into_vec()),
            )
        }
        BootstrapPacket::Batch(packet) => {
            RawPacket::new(packet_id::BATCH, packet.encode_body(limits)?)
        }
    };

    Ok(raw)
}

/// Encodes one implemented bootstrap packet as an outer 0xfe game frame.
pub fn encode_bootstrap_frame(
    packet: &BootstrapPacket,
    limits: CodecLimits,
) -> Result<NativeBuffer, CodecError> {
    let raw = encode_bootstrap_packet(packet, limits)?;
    encode_game_frame(&raw, limits)
}

pub fn decode_bootstrap_packet(
    raw: RawPacket,
    limits: CodecLimits,
) -> Result<BootstrapPacket, CodecError> {
    match raw.id() {
        packet_id::LOGIN => decode_login(raw.body().as_slice(), limits).map(BootstrapPacket::Login),
        packet_id::PLAY_STATUS => {
            let mut reader = Reader::new(raw.body().as_slice());
            let status = reader.read_i32_be()?;
            reader.finish()?;
            Ok(BootstrapPacket::PlayStatus(PlayStatusPacket::new(status)))
        }
        packet_id::DISCONNECT => {
            let mut reader = Reader::new(raw.body().as_slice());
            let message = reader.read_string_u16()?;
            reader.finish()?;
            Ok(BootstrapPacket::Disconnect(DisconnectPacket::new(message)))
        }
        packet_id::SET_TIME => {
            let mut reader = Reader::new(raw.body().as_slice());
            let time = reader.read_i32_be()?;
            let started = reader.read_u8()? != 0;
            reader.finish()?;
            Ok(BootstrapPacket::SetTime(SetTimePacket::new(time, started)))
        }
        packet_id::START_GAME => decode_start_game(raw.body().as_slice()),
        packet_id::SET_SPAWN_POSITION => {
            let mut reader = Reader::new(raw.body().as_slice());
            let packet = SetSpawnPositionPacket::new(
                reader.read_i32_be()?,
                reader.read_i32_be()?,
                reader.read_i32_be()?,
            );
            reader.finish()?;
            Ok(BootstrapPacket::SetSpawnPosition(packet))
        }
        packet_id::ADVENTURE_SETTINGS => {
            let mut reader = Reader::new(raw.body().as_slice());
            let packet = AdventureSettingsPacket::new(
                reader.read_i32_be()?,
                reader.read_i32_be()?,
                reader.read_i32_be()?,
            );
            reader.finish()?;
            Ok(BootstrapPacket::AdventureSettings(packet))
        }
        packet_id::SET_DIFFICULTY => {
            let mut reader = Reader::new(raw.body().as_slice());
            let difficulty = reader.read_i32_be()?;
            reader.finish()?;
            Ok(BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(
                difficulty,
            )))
        }
        packet_id::BATCH => {
            BatchPacket::decode_body(raw.body().as_slice(), limits).map(BootstrapPacket::Batch)
        }
        id => Err(CodecError::UnsupportedPacket { id }),
    }
}

fn decode_start_game(body: &[u8]) -> Result<BootstrapPacket, CodecError> {
    let mut reader = Reader::new(body);
    let seed = reader.read_i32_be()?;
    let dimension_raw = reader.read_u8()?;
    let dimension =
        DimensionId::try_from(dimension_raw).map_err(|_| CodecError::InvalidFixedTargetValue {
            field: "dimension",
            value: u64::from(dimension_raw),
            max: u64::from(u8::from(DimensionId::Nether)),
        })?;
    let generator = reader.read_i32_be()?;
    let gamemode = reader.read_i32_be()?;
    let entity_id = reader.read_i64_be()?;
    let spawn = [
        reader.read_i32_be()?,
        reader.read_i32_be()?,
        reader.read_i32_be()?,
    ];
    let position = [
        reader.read_f32_be()?,
        reader.read_f32_be()?,
        reader.read_f32_be()?,
    ];
    require_fixed_byte(&mut reader, "start-game flag 1", 1)?;
    require_fixed_byte(&mut reader, "start-game flag 2", 1)?;
    require_fixed_byte(&mut reader, "start-game flag 3", 0)?;
    let level_id = reader.read_string_u16()?;
    reader.finish()?;

    Ok(BootstrapPacket::StartGame(StartGamePacket {
        seed,
        dimension,
        generator,
        gamemode,
        entity_id,
        spawn,
        position,
        level_id,
    }))
}

fn require_fixed_byte(
    reader: &mut Reader<'_>,
    field: &'static str,
    expected: u8,
) -> Result<(), CodecError> {
    let actual = reader.read_u8()?;
    if actual == expected {
        Ok(())
    } else {
        Err(CodecError::InvalidFixedByte {
            field,
            expected,
            actual,
        })
    }
}

fn decode_login(body: &[u8], limits: CodecLimits) -> Result<LoginPacket, CodecError> {
    let mut reader = Reader::new(body);
    let protocol = reader.read_i32_be()?;
    if protocol != PROTOCOL_VERSION {
        return Err(CodecError::UnsupportedProtocol {
            expected: PROTOCOL_VERSION,
            actual: protocol,
        });
    }

    let compressed_len = nonnegative_len("login compressed length", reader.read_i32_be()?)?;
    check_limit(
        LimitKind::LoginCompressed,
        compressed_len,
        limits.max_login_compressed_bytes(),
    )?;
    let compressed = reader.read_exact(compressed_len)?;
    reader.finish()?;

    let decompressed = decompress_zlib_limited(
        compressed,
        LOGIN_MAX_DECOMPRESSED_BYTES,
        LimitKind::LoginDecompressed,
    )?;
    let mut inner = Reader::new(&decompressed);
    let chain_len = nonnegative_len("login chain length", inner.read_i32_le()?)?;
    let chain_data = NativeBuffer::copy_from_slice(inner.read_exact(chain_len)?);
    let skin_len = nonnegative_len("login skin JWT length", inner.read_i32_le()?)?;
    let skin_jwt = NativeBuffer::copy_from_slice(inner.read_exact(skin_len)?);
    inner.finish()?;

    Ok(LoginPacket {
        protocol,
        chain_data,
        skin_jwt,
    })
}

fn encode_login(packet: &LoginPacket, limits: CodecLimits) -> Result<NativeBuffer, CodecError> {
    if packet.protocol != PROTOCOL_VERSION {
        return Err(CodecError::UnsupportedProtocol {
            expected: PROTOCOL_VERSION,
            actual: packet.protocol,
        });
    }

    let chain_len =
        i32::try_from(packet.chain_data.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login chain length",
            value: packet.chain_data.len(),
            max: i32::MAX as usize,
        })?;
    let skin_len =
        i32::try_from(packet.skin_jwt.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login skin JWT length",
            value: packet.skin_jwt.len(),
            max: i32::MAX as usize,
        })?;

    let mut inner = Writer::new();
    inner.put_i32_le(chain_len);
    inner.put_bytes(packet.chain_data.as_slice());
    inner.put_i32_le(skin_len);
    inner.put_bytes(packet.skin_jwt.as_slice());
    let inner = inner.into_vec();
    check_limit(
        LimitKind::LoginDecompressed,
        inner.len(),
        LOGIN_MAX_DECOMPRESSED_BYTES,
    )?;

    let compressed = compress_zlib(&inner)?;
    check_limit(
        LimitKind::LoginCompressed,
        compressed.len(),
        limits.max_login_compressed_bytes(),
    )?;
    let compressed_len =
        i32::try_from(compressed.len()).map_err(|_| CodecError::LengthOutOfRange {
            field: "login compressed length",
            value: compressed.len(),
            max: i32::MAX as usize,
        })?;

    let mut writer = Writer::with_capacity(8 + compressed.len());
    writer.put_i32_be(PROTOCOL_VERSION);
    writer.put_i32_be(compressed_len);
    writer.put_bytes(&compressed);
    Ok(NativeBuffer::from_vec(writer.into_vec()))
}

fn nonnegative_len(field: &'static str, value: i32) -> Result<usize, CodecError> {
    usize::try_from(value).map_err(|_| CodecError::NegativeLength { field, value })
}
