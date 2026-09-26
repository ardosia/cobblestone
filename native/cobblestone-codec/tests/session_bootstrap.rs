use cobblestone_codec::{
    AdventureSettingsPacket, BootstrapPacket, CodecLimits, SetDifficultyPacket,
    SetSpawnPositionPacket, SetTimePacket, StartGamePacket, decode_bootstrap_frame,
    encode_bootstrap_frame,
};

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

#[test]
fn exact_set_time_fixture_round_trips() {
    let fixture = [0xfe, 0x08, 0x00, 0x00, 0x17, 0x70, 0x01];
    let packet = BootstrapPacket::SetTime(SetTimePacket::new(6000, true));
    assert_eq!(
        encode_bootstrap_frame(&packet, limits())
            .expect("encode set time")
            .as_slice(),
        fixture
    );
    assert_eq!(
        decode_bootstrap_frame(&fixture, limits()).expect("decode set time"),
        packet
    );
}

#[test]
fn exact_spawn_difficulty_and_adventure_fixtures_round_trip() {
    let spawn = [
        0xfe, 0x26, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0xff, 0xff, 0xff, 0xfe,
    ];
    let spawn_packet =
        BootstrapPacket::SetSpawnPosition(SetSpawnPositionPacket::new(1, 64, -2));
    assert_eq!(
        encode_bootstrap_frame(&spawn_packet, limits())
            .expect("encode spawn")
            .as_slice(),
        spawn
    );
    assert_eq!(
        decode_bootstrap_frame(&spawn, limits()).expect("decode spawn"),
        spawn_packet
    );

    let difficulty = [0xfe, 0x35, 0x00, 0x00, 0x00, 0x02];
    let difficulty_packet = BootstrapPacket::SetDifficulty(SetDifficultyPacket::new(2));
    assert_eq!(
        encode_bootstrap_frame(&difficulty_packet, limits())
            .expect("encode difficulty")
            .as_slice(),
        difficulty
    );
    assert_eq!(
        decode_bootstrap_frame(&difficulty, limits()).expect("decode difficulty"),
        difficulty_packet
    );

    let adventure = [
        0xfe, 0x31, 0x00, 0x00, 0x00, 0x4e, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02,
    ];
    let adventure_packet =
        BootstrapPacket::AdventureSettings(AdventureSettingsPacket::new(0x4e, 1, 2));
    assert_eq!(
        encode_bootstrap_frame(&adventure_packet, limits())
            .expect("encode adventure settings")
            .as_slice(),
        adventure
    );
    assert_eq!(
        decode_bootstrap_frame(&adventure, limits()).expect("decode adventure settings"),
        adventure_packet
    );
}

#[test]
fn exact_start_game_fixture_round_trips() {
    let fixture = [
        0xfe, 0x09, // marker + StartGame
        0x00, 0x00, 0x00, 0x7b, // seed 123
        0x00, // dimension
        0x00, 0x00, 0x00, 0x01, // generator
        0x00, 0x00, 0x00, 0x00, // gamemode
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x05, // eid
        0x00, 0x00, 0x00, 0x01, // spawn x
        0x00, 0x00, 0x00, 0x40, // spawn y
        0xff, 0xff, 0xff, 0xfe, // spawn z
        0x3f, 0xc0, 0x00, 0x00, // x 1.5
        0x42, 0x82, 0x00, 0x00, // y 65.0
        0xc0, 0x10, 0x00, 0x00, // z -2.25
        0x01, 0x01, 0x00, // fixed protocol-84 flags
        0x00, 0x05, b'w', b'o', b'r', b'l', b'd',
    ];
    let packet = BootstrapPacket::StartGame(StartGamePacket {
        seed: 123,
        dimension: 0,
        generator: 1,
        gamemode: 0,
        entity_id: 5,
        spawn: [1, 64, -2],
        position: [1.5, 65.0, -2.25],
        level_id: "world".to_owned(),
    });
    assert_eq!(
        encode_bootstrap_frame(&packet, limits())
            .expect("encode start game")
            .as_slice(),
        fixture
    );
    assert_eq!(
        decode_bootstrap_frame(&fixture, limits()).expect("decode start game"),
        packet
    );
}

#[test]
fn start_game_rejects_changed_fixed_target_flag() {
    let mut fixture = [
        0xfe, 0x09, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0,
        0, 0,
    ];
    let fixed_flag_offset = fixture.len() - 5;
    fixture[fixed_flag_offset] = 2;
    assert!(decode_bootstrap_frame(&fixture, limits()).is_err());
}
