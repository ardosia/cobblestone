use cobblestone_codec::{
    BatchPacket, BootstrapPacket, CodecError, CodecLimits, DisconnectPacket, LimitKind,
    LoginPacket, PlayStatusPacket, RawPacket, decode_bootstrap_frame, encode_bootstrap_frame,
    packet_id,
};
use cobblestone_core::NativeBuffer;

fn limits() -> CodecLimits {
    CodecLimits::new(
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        2 * 1024 * 1024,
        1024 * 1024,
        1024,
    )
}

fn hex(input: &str) -> Vec<u8> {
    assert!(input.len().is_multiple_of(2));
    input
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16).expect("valid hex")
        })
        .collect()
}

#[test]
fn fixed_packet_ids_match_protocol84_oracle() {
    assert_eq!(packet_id::LOGIN, 0x01);
    assert_eq!(packet_id::PLAY_STATUS, 0x02);
    assert_eq!(packet_id::SERVER_TO_CLIENT_HANDSHAKE, 0x03);
    assert_eq!(packet_id::CLIENT_TO_SERVER_HANDSHAKE, 0x04);
    assert_eq!(packet_id::DISCONNECT, 0x05);
    assert_eq!(packet_id::BATCH, 0x06);
    assert_eq!(packet_id::TEXT, 0x07);
    assert_eq!(packet_id::SET_TIME, 0x08);
    assert_eq!(packet_id::START_GAME, 0x09);
}

#[test]
fn exact_play_status_fixture_round_trips() {
    let fixture = hex("fe0200000000");
    let decoded = decode_bootstrap_frame(&fixture, limits()).expect("decode fixture");
    assert_eq!(
        decoded,
        BootstrapPacket::PlayStatus(PlayStatusPacket::new(
            PlayStatusPacket::LOGIN_SUCCESS
        ))
    );
    assert_eq!(
        encode_bootstrap_frame(&decoded, limits())
            .expect("encode fixture")
            .as_slice(),
        fixture
    );
}

#[test]
fn exact_disconnect_fixture_round_trips() {
    let fixture = hex("fe050003627965");
    let decoded = decode_bootstrap_frame(&fixture, limits()).expect("decode fixture");
    assert_eq!(
        decoded,
        BootstrapPacket::Disconnect(DisconnectPacket::new("bye"))
    );
    assert_eq!(
        encode_bootstrap_frame(&decoded, limits())
            .expect("encode fixture")
            .as_slice(),
        fixture
    );
}

#[test]
fn exact_login_fixture_decodes_protocol84_envelope() {
    let fixture = hex(
        "fe01000000540000001f78dae3616060a8564ace48cccc53b28a8ead6505f213f592f4920146c205c5",
    );
    let decoded = decode_bootstrap_frame(&fixture, limits()).expect("decode login fixture");
    let BootstrapPacket::Login(login) = decoded else {
        panic!("expected login packet");
    };
    assert_eq!(login.protocol(), 84);
    assert_eq!(login.chain_data().as_slice(), br#"{"chain":[]}"#);
    assert_eq!(login.skin_jwt().as_slice(), b"a.b.c");

    let encoded = encode_bootstrap_frame(
        &BootstrapPacket::Login(LoginPacket::protocol84(
            NativeBuffer::copy_from_slice(br#"{"chain":[]}"#),
            NativeBuffer::copy_from_slice(b"a.b.c"),
        )),
        limits(),
    )
    .expect("encode login");
    let reparsed = decode_bootstrap_frame(encoded.as_slice(), limits()).expect("reparse login");
    assert!(matches!(reparsed, BootstrapPacket::Login(_)));
}

#[test]
fn exact_batch_fixture_unpacks_length_prefixed_inner_packet() {
    let fixture = hex("fe060000000f78da63606060656200020000310008");
    let decoded = decode_bootstrap_frame(&fixture, limits()).expect("decode batch fixture");
    let BootstrapPacket::Batch(batch) = decoded else {
        panic!("expected batch packet");
    };
    assert_eq!(
        batch.packets(),
        &[RawPacket::new(
            packet_id::PLAY_STATUS,
            NativeBuffer::copy_from_slice(&[0, 0, 0, 0]),
        )]
    );

    let encoded = encode_bootstrap_frame(
        &BootstrapPacket::Batch(BatchPacket::new(batch.packets().to_vec())),
        limits(),
    )
    .expect("encode batch");
    let reparsed = decode_bootstrap_frame(encoded.as_slice(), limits()).expect("reparse batch");
    assert_eq!(reparsed, BootstrapPacket::Batch(batch));
}

#[test]
fn malformed_or_wrong_target_input_is_rejected() {
    assert!(matches!(
        decode_bootstrap_frame(&[], limits()),
        Err(CodecError::UnexpectedEof { .. })
    ));
    assert_eq!(
        decode_bootstrap_frame(&[0xfd, 0x02, 0, 0, 0, 0], limits()),
        Err(CodecError::InvalidGameMarker { actual: 0xfd })
    );

    let wrong_protocol = hex(
        "fe01000000550000001f78dae3616060a8564ace48cccc53b28a8ead6505f213f592f4920146c205c5",
    );
    assert_eq!(
        decode_bootstrap_frame(&wrong_protocol, limits()),
        Err(CodecError::UnsupportedProtocol {
            expected: 84,
            actual: 85,
        })
    );
}

#[test]
fn decompression_limit_is_enforced_before_batch_expands_unboundedly() {
    let fixture = hex("fe060000000f78da63606060656200020000310008");
    let tight = CodecLimits::new(
        1024,
        1024,
        1024,
        4,
        1024,
        16,
    );
    assert!(matches!(
        decode_bootstrap_frame(&fixture, tight),
        Err(CodecError::LimitExceeded {
            kind: LimitKind::BatchDecompressed,
            ..
        })
    ));
}
