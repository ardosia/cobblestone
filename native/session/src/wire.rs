use std::collections::VecDeque;

use cobblestone_wire::{
    BootstrapPacket, CodecError, CodecLimits, decode_bootstrap_frame, decode_game_frame, packet_id,
};

use crate::SessionPacket;

pub(crate) fn decode_connected_payload(
    payload: &[u8],
    limits: CodecLimits,
) -> Result<VecDeque<SessionPacket>, CodecError> {
    let raw = decode_game_frame(payload, limits)?;
    if raw.id() != packet_id::BATCH {
        return Ok(VecDeque::from([SessionPacket::from_raw(&raw)]));
    }

    let BootstrapPacket::Batch(batch) = decode_bootstrap_frame(payload, limits)? else {
        return Err(CodecError::UnsupportedPacket { id: raw.id() });
    };

    if batch.packets().is_empty() {
        return Err(CodecError::EmptyPacket);
    }

    Ok(batch
        .packets()
        .iter()
        .map(SessionPacket::from_raw)
        .collect())
}

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use cobblestone_wire::{BatchPacket, CodecLimits, RawPacket, encode_bootstrap_frame};

    use super::decode_connected_payload;
    use crate::SessionPacket;

    fn limits() -> CodecLimits {
        CodecLimits::new(4096, 4096, 4096, 4096, 1024, 16)
    }

    #[test]
    fn batch_is_flattened_before_reaching_owner() {
        let batch = cobblestone_wire::BootstrapPacket::Batch(BatchPacket::new(vec![
            RawPacket::new(0x10, Bytes::from(vec![1, 2])),
            RawPacket::new(0x20, Bytes::from(vec![3, 4, 5])),
        ]));
        let frame = encode_bootstrap_frame(&batch, limits()).expect("encode batch");

        let packets = decode_connected_payload(frame.as_ref(), limits()).expect("decode batch");
        assert_eq!(
            packets.into_iter().collect::<Vec<_>>(),
            vec![
                SessionPacket::new(0x10, Bytes::from(vec![1, 2])),
                SessionPacket::new(0x20, Bytes::from(vec![3, 4, 5])),
            ]
        );
    }
}
