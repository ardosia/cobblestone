use cobblestone_protocol84::{
    BatchPacket, BootstrapPacket, CodecLimits, NamedNbt, NbtDocument, NbtLimits, NbtValue,
    RawPacket, decode_bootstrap_frame, decode_game_frame, encode_bootstrap_frame,
    encode_game_frame,
};
use cobblestone_runtime::NativeBuffer;
use proptest::prelude::*;

fn codec_limits() -> CodecLimits {
    CodecLimits::new(64 * 1024, 64 * 1024, 64 * 1024, 64 * 1024, 4 * 1024, 64)
}

fn nbt_limits() -> NbtLimits {
    NbtLimits::new(16 * 1024, 8, 64, 512)
}

proptest! {
    #[test]
    fn raw_game_frame_round_trips(id in any::<u8>(), body in prop::collection::vec(any::<u8>(), 0..1024)) {
        let packet = RawPacket::new(id, NativeBuffer::from_vec(body));
        let encoded = encode_game_frame(&packet, codec_limits()).expect("frame should encode");
        let decoded = decode_game_frame(encoded.as_slice(), codec_limits()).expect("frame should decode");
        prop_assert_eq!(decoded, packet);
    }

    #[test]
    fn batch_round_trips(
        packets in prop::collection::vec(
            (any::<u8>(), prop::collection::vec(any::<u8>(), 0..256)),
            0..16,
        )
    ) {
        let packets = packets
            .into_iter()
            .map(|(id, body)| RawPacket::new(id, NativeBuffer::from_vec(body)))
            .collect::<Vec<_>>();
        let expected = BootstrapPacket::Batch(BatchPacket::new(packets));
        let encoded =
            encode_bootstrap_frame(&expected, codec_limits()).expect("batch should encode");
        let decoded =
            decode_bootstrap_frame(encoded.as_slice(), codec_limits()).expect("batch should decode");
        prop_assert_eq!(decoded, expected);
    }

    #[test]
    fn little_endian_nbt_scalar_compound_round_trips(
        number in any::<i32>(),
        text in ".{0,128}",
    ) {
        let expected = NbtDocument::new(NamedNbt::new(
            "",
            NbtValue::Compound(vec![
                NamedNbt::new("number", NbtValue::Int(number)),
                NamedNbt::new("text", NbtValue::String(text)),
            ]),
        ));
        let encoded = expected.encode_le(nbt_limits()).expect("NBT should encode");
        let decoded = NbtDocument::decode_le(encoded.as_slice(), nbt_limits())
            .expect("NBT should decode");
        prop_assert_eq!(decoded, expected);
    }
}
