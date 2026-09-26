use std::io::{Read, Write};

use cobblestone_core::NativeBuffer;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;

use crate::binary::{Reader, Writer};
use crate::frame::{RawPacket, check_limit};
use crate::{CodecError, CodecLimits, LimitKind};

/// Protocol-84 Batch packet contents after packet ID 0x06.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct BatchPacket {
    packets: Vec<RawPacket>,
}

impl BatchPacket {
    /// Creates a batch from raw protocol-84 packets.
    pub fn new(packets: Vec<RawPacket>) -> Self {
        Self { packets }
    }

    /// Raw packets carried by this batch.
    pub fn packets(&self) -> &[RawPacket] {
        &self.packets
    }

    pub(crate) fn decode_body(body: &[u8], limits: CodecLimits) -> Result<Self, CodecError> {
        let mut reader = Reader::new(body);
        let compressed_len =
            usize::try_from(reader.read_u32_be()?).map_err(|_| CodecError::LengthOutOfRange {
                field: "batch compressed length",
                value: usize::MAX,
                max: limits.max_batch_compressed_bytes(),
            })?;
        check_limit(
            LimitKind::BatchCompressed,
            compressed_len,
            limits.max_batch_compressed_bytes(),
        )?;
        let compressed = reader.read_exact(compressed_len)?;
        reader.finish()?;

        let decompressed = decompress_zlib_limited(
            compressed,
            limits.max_batch_decompressed_bytes(),
            LimitKind::BatchDecompressed,
        )?;
        let mut inner = Reader::new(&decompressed);
        let mut packets = Vec::new();

        while inner.remaining() > 0 {
            let next_count = packets.len() + 1;
            check_limit(
                LimitKind::InnerPacketCount,
                next_count,
                limits.max_inner_packets(),
            )?;

            let packet_len = usize::try_from(inner.read_u32_be()?).map_err(|_| {
                CodecError::LengthOutOfRange {
                    field: "batch inner packet length",
                    value: usize::MAX,
                    max: limits.max_inner_packet_bytes(),
                }
            })?;
            if packet_len == 0 {
                return Err(CodecError::EmptyPacket);
            }
            check_limit(
                LimitKind::InnerPacket,
                packet_len,
                limits.max_inner_packet_bytes(),
            )?;
            packets.push(RawPacket::from_packet_bytes(inner.read_exact(packet_len)?)?);
        }

        Ok(Self { packets })
    }

    pub(crate) fn encode_body(&self, limits: CodecLimits) -> Result<NativeBuffer, CodecError> {
        check_limit(
            LimitKind::InnerPacketCount,
            self.packets.len(),
            limits.max_inner_packets(),
        )?;

        let mut uncompressed = Vec::new();
        for packet in &self.packets {
            let packet_len = packet.encoded_len();
            check_limit(
                LimitKind::InnerPacket,
                packet_len,
                limits.max_inner_packet_bytes(),
            )?;
            let packet_len_u32 =
                u32::try_from(packet_len).map_err(|_| CodecError::LengthOutOfRange {
                    field: "batch inner packet length",
                    value: packet_len,
                    max: u32::MAX as usize,
                })?;
            uncompressed.extend_from_slice(&packet_len_u32.to_be_bytes());
            packet.write_packet_bytes(&mut uncompressed);
            check_limit(
                LimitKind::BatchDecompressed,
                uncompressed.len(),
                limits.max_batch_decompressed_bytes(),
            )?;
        }

        let compressed = compress_zlib(&uncompressed)?;
        check_limit(
            LimitKind::BatchCompressed,
            compressed.len(),
            limits.max_batch_compressed_bytes(),
        )?;
        let compressed_len =
            u32::try_from(compressed.len()).map_err(|_| CodecError::LengthOutOfRange {
                field: "batch compressed length",
                value: compressed.len(),
                max: u32::MAX as usize,
            })?;

        let mut writer = Writer::with_capacity(4 + compressed.len());
        writer.put_u32_be(compressed_len);
        writer.put_bytes(&compressed);
        Ok(NativeBuffer::from_vec(writer.into_vec()))
    }
}

pub(crate) fn compress_zlib(input: &[u8]) -> Result<Vec<u8>, CodecError> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::new(7));
    encoder
        .write_all(input)
        .map_err(|error| CodecError::Compression {
            message: error.to_string(),
        })?;
    encoder.finish().map_err(|error| CodecError::Compression {
        message: error.to_string(),
    })
}

pub(crate) fn decompress_zlib_limited(
    input: &[u8],
    limit: usize,
    kind: LimitKind,
) -> Result<Vec<u8>, CodecError> {
    let decoder = ZlibDecoder::new(input);
    let read_limit = u64::try_from(limit).unwrap_or(u64::MAX).saturating_add(1);
    let mut limited = decoder.take(read_limit);
    let mut output = Vec::new();
    limited
        .read_to_end(&mut output)
        .map_err(|error| CodecError::Compression {
            message: error.to_string(),
        })?;
    check_limit(kind, output.len(), limit)?;
    Ok(output)
}
