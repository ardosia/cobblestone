use cobblestone_core::NativeBuffer;

use crate::binary::{Reader, Writer};
use crate::{CodecError, LimitKind};

/// Standard NBT tag IDs used by MCPE 0.15.10 little-endian network NBT.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(u8)]
pub enum NbtTag {
    /// Compound/list terminator.
    End = 0,
    /// Signed byte.
    Byte = 1,
    /// Signed 16-bit integer.
    Short = 2,
    /// Signed 32-bit integer.
    Int = 3,
    /// Signed 64-bit integer.
    Long = 4,
    /// 32-bit float.
    Float = 5,
    /// 64-bit float.
    Double = 6,
    /// Length-prefixed byte array.
    ByteArray = 7,
    /// Length-prefixed UTF-8 string.
    String = 8,
    /// Homogeneous list.
    List = 9,
    /// Named tag compound.
    Compound = 10,
    /// Length-prefixed signed 32-bit integer array.
    IntArray = 11,
}

impl NbtTag {
    /// Numeric NBT tag ID.
    pub const fn id(self) -> u8 {
        self as u8
    }

    fn from_id(id: u8) -> Result<Self, CodecError> {
        match id {
            0 => Ok(Self::End),
            1 => Ok(Self::Byte),
            2 => Ok(Self::Short),
            3 => Ok(Self::Int),
            4 => Ok(Self::Long),
            5 => Ok(Self::Float),
            6 => Ok(Self::Double),
            7 => Ok(Self::ByteArray),
            8 => Ok(Self::String),
            9 => Ok(Self::List),
            10 => Ok(Self::Compound),
            11 => Ok(Self::IntArray),
            _ => Err(CodecError::InvalidNbtTag { id }),
        }
    }
}

/// One named NBT value.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedNbt {
    name: String,
    value: NbtValue,
}

impl NamedNbt {
    /// Creates one named value.
    pub fn new(name: impl Into<String>, value: NbtValue) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    /// Tag name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tag payload.
    pub const fn value(&self) -> &NbtValue {
        &self.value
    }
}

/// Little-endian NBT value supported by the protocol-84 network format.
#[derive(Debug, Clone, PartialEq)]
pub enum NbtValue {
    /// TAG_Byte.
    Byte(i8),
    /// TAG_Short.
    Short(i16),
    /// TAG_Int.
    Int(i32),
    /// TAG_Long.
    Long(i64),
    /// TAG_Float.
    Float(f32),
    /// TAG_Double.
    Double(f64),
    /// TAG_Byte_Array.
    ByteArray(NativeBuffer),
    /// TAG_String.
    String(String),
    /// TAG_List with an explicit homogeneous element type.
    List {
        /// Declared list element type. TAG_End is valid only for an empty list.
        element_type: NbtTag,
        /// List elements.
        values: Vec<NbtValue>,
    },
    /// TAG_Compound.
    Compound(Vec<NamedNbt>),
    /// TAG_Int_Array.
    IntArray(Vec<i32>),
}

impl NbtValue {
    /// Tag type corresponding to this value.
    pub const fn tag(&self) -> NbtTag {
        match self {
            Self::Byte(_) => NbtTag::Byte,
            Self::Short(_) => NbtTag::Short,
            Self::Int(_) => NbtTag::Int,
            Self::Long(_) => NbtTag::Long,
            Self::Float(_) => NbtTag::Float,
            Self::Double(_) => NbtTag::Double,
            Self::ByteArray(_) => NbtTag::ByteArray,
            Self::String(_) => NbtTag::String,
            Self::List { .. } => NbtTag::List,
            Self::Compound(_) => NbtTag::Compound,
            Self::IntArray(_) => NbtTag::IntArray,
        }
    }
}

/// One complete named-root little-endian NBT document.
#[derive(Debug, Clone, PartialEq)]
pub struct NbtDocument {
    root: NamedNbt,
}

impl NbtDocument {
    /// Creates a document from a named root tag.
    pub fn new(root: NamedNbt) -> Self {
        Self { root }
    }

    /// Root named tag.
    pub const fn root(&self) -> &NamedNbt {
        &self.root
    }

    /// Decodes one complete MCPE little-endian NBT document.
    pub fn decode_le(input: &[u8], limits: NbtLimits) -> Result<Self, CodecError> {
        check_limit(LimitKind::NbtBytes, input.len(), limits.max_bytes)?;
        let mut reader = Reader::new(input);
        let tag = NbtTag::from_id(reader.read_u8()?)?;
        if tag == NbtTag::End {
            return Err(CodecError::InvalidNbtTag {
                id: NbtTag::End.id(),
            });
        }
        let name = read_string(&mut reader, limits)?;
        let value = read_payload(&mut reader, tag, limits, 0)?;
        reader.finish()?;
        Ok(Self::new(NamedNbt::new(name, value)))
    }

    /// Encodes one complete MCPE little-endian NBT document.
    pub fn encode_le(&self, limits: NbtLimits) -> Result<NativeBuffer, CodecError> {
        let mut writer = Writer::new();
        writer.put_u8(self.root.value.tag().id());
        write_string(&mut writer, "NBT root name", &self.root.name, limits)?;
        write_payload(&mut writer, &self.root.value, limits, 0)?;
        check_limit(LimitKind::NbtBytes, writer.len(), limits.max_bytes)?;
        Ok(NativeBuffer::from_vec(writer.into_vec()))
    }
}

/// Explicit resource limits for untrusted little-endian NBT.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct NbtLimits {
    max_bytes: usize,
    max_depth: usize,
    max_collection_len: usize,
    max_string_bytes: usize,
}

impl NbtLimits {
    /// Creates explicit NBT resource bounds.
    pub const fn new(
        max_bytes: usize,
        max_depth: usize,
        max_collection_len: usize,
        max_string_bytes: usize,
    ) -> Self {
        Self {
            max_bytes,
            max_depth,
            max_collection_len,
            max_string_bytes,
        }
    }
}

fn read_payload(
    reader: &mut Reader<'_>,
    tag: NbtTag,
    limits: NbtLimits,
    depth: usize,
) -> Result<NbtValue, CodecError> {
    check_limit(LimitKind::NbtDepth, depth, limits.max_depth)?;
    match tag {
        NbtTag::End => Err(CodecError::InvalidNbtTag { id: tag.id() }),
        NbtTag::Byte => Ok(NbtValue::Byte(reader.read_i8()?)),
        NbtTag::Short => Ok(NbtValue::Short(reader.read_i16_le()?)),
        NbtTag::Int => Ok(NbtValue::Int(reader.read_i32_le()?)),
        NbtTag::Long => Ok(NbtValue::Long(reader.read_i64_le()?)),
        NbtTag::Float => Ok(NbtValue::Float(reader.read_f32_le()?)),
        NbtTag::Double => Ok(NbtValue::Double(reader.read_f64_le()?)),
        NbtTag::ByteArray => {
            let len = read_collection_len(reader, "NBT byte array length", limits)?;
            Ok(NbtValue::ByteArray(NativeBuffer::copy_from_slice(
                reader.read_exact(len)?,
            )))
        }
        NbtTag::String => Ok(NbtValue::String(read_string(reader, limits)?)),
        NbtTag::List => {
            let element_type = NbtTag::from_id(reader.read_u8()?)?;
            let len = read_collection_len(reader, "NBT list length", limits)?;
            if element_type == NbtTag::End && len != 0 {
                return Err(CodecError::InvalidNbtList { length: len });
            }
            let mut values = Vec::new();
            for _ in 0..len {
                values.push(read_payload(reader, element_type, limits, depth + 1)?);
            }
            Ok(NbtValue::List {
                element_type,
                values,
            })
        }
        NbtTag::Compound => {
            let mut entries = Vec::new();
            loop {
                let child_type = NbtTag::from_id(reader.read_u8()?)?;
                if child_type == NbtTag::End {
                    break;
                }
                check_limit(
                    LimitKind::NbtCollection,
                    entries.len() + 1,
                    limits.max_collection_len,
                )?;
                let name = read_string(reader, limits)?;
                let value = read_payload(reader, child_type, limits, depth + 1)?;
                entries.push(NamedNbt::new(name, value));
            }
            Ok(NbtValue::Compound(entries))
        }
        NbtTag::IntArray => {
            let len = read_collection_len(reader, "NBT int array length", limits)?;
            let mut values = Vec::new();
            for _ in 0..len {
                values.push(reader.read_i32_le()?);
            }
            Ok(NbtValue::IntArray(values))
        }
    }
}

fn write_payload(
    writer: &mut Writer,
    value: &NbtValue,
    limits: NbtLimits,
    depth: usize,
) -> Result<(), CodecError> {
    check_limit(LimitKind::NbtDepth, depth, limits.max_depth)?;
    match value {
        NbtValue::Byte(value) => writer.put_i8(*value),
        NbtValue::Short(value) => writer.put_i16_le(*value),
        NbtValue::Int(value) => writer.put_i32_le(*value),
        NbtValue::Long(value) => writer.put_i64_le(*value),
        NbtValue::Float(value) => writer.put_f32_le(*value),
        NbtValue::Double(value) => writer.put_f64_le(*value),
        NbtValue::ByteArray(value) => {
            write_collection_len(writer, "NBT byte array length", value.len(), limits)?;
            ensure_output_room(writer, value.len(), limits)?;
            writer.put_bytes(value.as_slice());
        }
        NbtValue::String(value) => write_string(writer, "NBT string", value, limits)?,
        NbtValue::List {
            element_type,
            values,
        } => {
            check_limit(
                LimitKind::NbtCollection,
                values.len(),
                limits.max_collection_len,
            )?;
            if *element_type == NbtTag::End && !values.is_empty() {
                return Err(CodecError::InvalidNbtList {
                    length: values.len(),
                });
            }
            writer.put_u8(element_type.id());
            write_i32_len(writer, "NBT list length", values.len())?;
            for value in values {
                let actual = value.tag();
                if actual != *element_type {
                    return Err(CodecError::NbtListTypeMismatch {
                        expected: element_type.id(),
                        actual: actual.id(),
                    });
                }
                write_payload(writer, value, limits, depth + 1)?;
            }
        }
        NbtValue::Compound(entries) => {
            check_limit(
                LimitKind::NbtCollection,
                entries.len(),
                limits.max_collection_len,
            )?;
            for entry in entries {
                writer.put_u8(entry.value.tag().id());
                write_string(writer, "NBT tag name", &entry.name, limits)?;
                write_payload(writer, &entry.value, limits, depth + 1)?;
            }
            writer.put_u8(NbtTag::End.id());
        }
        NbtValue::IntArray(values) => {
            write_collection_len(writer, "NBT int array length", values.len(), limits)?;
            for value in values {
                writer.put_i32_le(*value);
            }
        }
    }
    check_limit(LimitKind::NbtBytes, writer.len(), limits.max_bytes)
}

fn read_string(reader: &mut Reader<'_>, limits: NbtLimits) -> Result<String, CodecError> {
    let len = usize::from(reader.read_u16_le()?);
    check_limit(LimitKind::NbtString, len, limits.max_string_bytes)?;
    let bytes = reader.read_exact(len)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| CodecError::InvalidUtf8)
}

fn write_string(
    writer: &mut Writer,
    field: &'static str,
    value: &str,
    limits: NbtLimits,
) -> Result<(), CodecError> {
    check_limit(LimitKind::NbtString, value.len(), limits.max_string_bytes)?;
    let len = u16::try_from(value.len()).map_err(|_| CodecError::LengthOutOfRange {
        field,
        value: value.len(),
        max: usize::from(u16::MAX),
    })?;
    let additional = 2_usize
        .checked_add(value.len())
        .ok_or(CodecError::LimitExceeded {
            kind: LimitKind::NbtBytes,
            limit: limits.max_bytes,
            actual: usize::MAX,
        })?;
    ensure_output_room(writer, additional, limits)?;
    writer.put_u16_le(len);
    writer.put_bytes(value.as_bytes());
    Ok(())
}

fn read_collection_len(
    reader: &mut Reader<'_>,
    field: &'static str,
    limits: NbtLimits,
) -> Result<usize, CodecError> {
    let value = reader.read_i32_le()?;
    let len = usize::try_from(value).map_err(|_| CodecError::NegativeLength { field, value })?;
    check_limit(LimitKind::NbtCollection, len, limits.max_collection_len)?;
    Ok(len)
}

fn write_collection_len(
    writer: &mut Writer,
    field: &'static str,
    len: usize,
    limits: NbtLimits,
) -> Result<(), CodecError> {
    check_limit(
        LimitKind::NbtCollection,
        len,
        limits.max_collection_len,
    )?;
    write_i32_len(writer, field, len)
}

fn write_i32_len(writer: &mut Writer, field: &'static str, len: usize) -> Result<(), CodecError> {
    let value = i32::try_from(len).map_err(|_| CodecError::LengthOutOfRange {
        field,
        value: len,
        max: i32::MAX as usize,
    })?;
    writer.put_i32_le(value);
    Ok(())
}

fn ensure_output_room(
    writer: &Writer,
    additional: usize,
    limits: NbtLimits,
) -> Result<(), CodecError> {
    let actual = writer
        .len()
        .checked_add(additional)
        .ok_or(CodecError::LimitExceeded {
            kind: LimitKind::NbtBytes,
            limit: limits.max_bytes,
            actual: usize::MAX,
        })?;
    check_limit(LimitKind::NbtBytes, actual, limits.max_bytes)
}

fn check_limit(kind: LimitKind, actual: usize, limit: usize) -> Result<(), CodecError> {
    if actual > limit {
        Err(CodecError::LimitExceeded {
            kind,
            limit,
            actual,
        })
    } else {
        Ok(())
    }
}
