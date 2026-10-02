use cobblestone_runtime::NativeBuffer;

use crate::binary::{Reader, Writer};
use crate::{CodecError, LimitKind};

mod codec;

use codec::{check_limit, read_payload, read_string, write_payload, write_string};

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
