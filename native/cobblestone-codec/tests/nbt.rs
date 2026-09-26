use cobblestone_codec::{
    CodecError, LimitKind, NamedNbt, NbtDocument, NbtLimits, NbtTag, NbtValue,
};

fn limits() -> NbtLimits {
    NbtLimits::new(4096, 16, 64, 256)
}

#[test]
fn exact_empty_little_endian_compound_round_trips() {
    let fixture = [0x0a, 0x00, 0x00, 0x00];
    let decoded = NbtDocument::decode_le(&fixture, limits()).expect("decode empty compound");
    assert_eq!(decoded.root().name(), "");
    assert_eq!(decoded.root().value(), &NbtValue::Compound(Vec::new()));
    assert_eq!(
        decoded.encode_le(limits()).expect("encode empty compound").as_slice(),
        fixture
    );
}

#[test]
fn exact_named_values_use_little_endian_lengths_and_numbers() {
    let fixture = [
        0x0a, 0x00, 0x00, // root compound with empty name
        0x03, 0x01, 0x00, b'x', 0x2a, 0x00, 0x00, 0x00, // int x = 42
        0x08, 0x03, 0x00, b'm', b's', b'g', 0x03, 0x00, b'h', b'e', b'y', // string
        0x00, // compound end
    ];
    let decoded = NbtDocument::decode_le(&fixture, limits()).expect("decode compound");
    let expected = NbtDocument::new(NamedNbt::new(
        "",
        NbtValue::Compound(vec![
            NamedNbt::new("x", NbtValue::Int(42)),
            NamedNbt::new("msg", NbtValue::String("hey".to_owned())),
        ]),
    ));
    assert_eq!(decoded, expected);
    assert_eq!(
        expected.encode_le(limits()).expect("encode compound").as_slice(),
        fixture
    );
}

#[test]
fn exact_little_endian_int_list_round_trips() {
    let fixture = [
        0x0a, 0x00, 0x00, // root compound
        0x09, 0x04, 0x00, b'n', b'u', b'm', b's', // named list
        0x03, 0x02, 0x00, 0x00, 0x00, // int type, count 2
        0x01, 0x00, 0x00, 0x00, // 1
        0x02, 0x00, 0x00, 0x00, // 2
        0x00,
    ];
    let decoded = NbtDocument::decode_le(&fixture, limits()).expect("decode list");
    assert_eq!(
        decoded,
        NbtDocument::new(NamedNbt::new(
            "",
            NbtValue::Compound(vec![NamedNbt::new(
                "nums",
                NbtValue::List {
                    element_type: NbtTag::Int,
                    values: vec![NbtValue::Int(1), NbtValue::Int(2)],
                },
            )]),
        ))
    );
    assert_eq!(
        decoded.encode_le(limits()).expect("encode list").as_slice(),
        fixture
    );
}

#[test]
fn nbt_limits_reject_hostile_sizes_before_large_allocation() {
    let empty = [0x0a, 0x00, 0x00, 0x00];
    assert_eq!(
        NbtDocument::decode_le(&empty, NbtLimits::new(3, 16, 64, 256)),
        Err(CodecError::LimitExceeded {
            kind: LimitKind::NbtBytes,
            limit: 3,
            actual: 4,
        })
    );

    let oversized_list = [
        0x09, 0x00, 0x00, // root list
        0x03, 0x02, 0x00, 0x00, 0x00, // int list of two
        0x01, 0x00, 0x00, 0x00,
        0x02, 0x00, 0x00, 0x00,
    ];
    assert_eq!(
        NbtDocument::decode_le(&oversized_list, NbtLimits::new(128, 8, 1, 32)),
        Err(CodecError::LimitExceeded {
            kind: LimitKind::NbtCollection,
            limit: 1,
            actual: 2,
        })
    );
}

#[test]
fn malformed_nbt_type_and_list_mismatch_are_typed_errors() {
    assert_eq!(
        NbtDocument::decode_le(&[0x0c, 0x00, 0x00], limits()),
        Err(CodecError::InvalidNbtTag { id: 0x0c })
    );

    let document = NbtDocument::new(NamedNbt::new(
        "",
        NbtValue::List {
            element_type: NbtTag::Int,
            values: vec![NbtValue::String("wrong".to_owned())],
        },
    ));
    assert_eq!(
        document.encode_le(limits()),
        Err(CodecError::NbtListTypeMismatch {
            expected: NbtTag::Int.id(),
            actual: NbtTag::String.id(),
        })
    );
}
