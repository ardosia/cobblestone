use cobblestone_protocol84::{
    CodecError, MovePlayerMode, decode_protocol84_move_player, packet_id,
};

fn hex(input: &str) -> Vec<u8> {
    let (pairs, remainder) = input.as_bytes().as_chunks::<2>();
    assert!(remainder.is_empty());
    pairs
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("ASCII hex");
            u8::from_str_radix(text, 16).expect("valid hex")
        })
        .collect()
}

fn fixture() -> Vec<u8> {
    hex("01020304050607083fc0000042808000c000000042b4000042340000c1f000000201")
}

#[test]
fn move_player_matches_protocol84_oracle() {
    assert_eq!(packet_id::MOVE_PLAYER, 0x10);

    let packet = decode_protocol84_move_player(&fixture()).expect("decode MovePlayer");
    assert_eq!(packet.entity_id(), 0x0102_0304_0506_0708);
    assert_eq!(packet.position(), [1.5, 64.25, -2.0]);
    assert_eq!(packet.yaw(), 90.0);
    assert_eq!(packet.body_yaw(), 45.0);
    assert_eq!(packet.pitch(), -30.0);
    assert_eq!(packet.mode(), MovePlayerMode::Rotation);
    assert!(packet.on_ground());
}

#[test]
fn move_player_rejects_truncated_trailing_and_invalid_mode() {
    let body = fixture();

    assert!(matches!(
        decode_protocol84_move_player(&body[..body.len() - 1]),
        Err(CodecError::UnexpectedEof { .. })
    ));

    let mut trailing = body.clone();
    trailing.push(0);
    assert_eq!(
        decode_protocol84_move_player(&trailing),
        Err(CodecError::TrailingBytes { remaining: 1 })
    );

    let mut invalid_mode = body;
    invalid_mode[32] = 3;
    assert_eq!(
        decode_protocol84_move_player(&invalid_mode),
        Err(CodecError::InvalidFixedTargetValue {
            field: "MovePlayer mode",
            value: 3,
            max: 2,
        })
    );
}

#[test]
fn move_player_treats_any_nonzero_on_ground_byte_as_true() {
    let mut body = fixture();
    body[33] = 0xff;

    let packet = decode_protocol84_move_player(&body).expect("decode MovePlayer");
    assert!(packet.on_ground());
}
