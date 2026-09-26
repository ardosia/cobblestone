use crate::CodecError;

pub(crate) struct Reader<'a> {
    input: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    pub(crate) const fn new(input: &'a [u8]) -> Self {
        Self { input, offset: 0 }
    }

    pub(crate) fn remaining(&self) -> usize {
        self.input.len().saturating_sub(self.offset)
    }

    pub(crate) fn read_exact(&mut self, len: usize) -> Result<&'a [u8], CodecError> {
        if self.remaining() < len {
            return Err(CodecError::UnexpectedEof {
                needed: len,
                remaining: self.remaining(),
            });
        }
        let start = self.offset;
        self.offset += len;
        Ok(&self.input[start..self.offset])
    }

    pub(crate) fn read_u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.read_exact(1)?[0])
    }

    pub(crate) fn read_i8(&mut self) -> Result<i8, CodecError> {
        Ok(i8::from_ne_bytes([self.read_u8()?]))
    }

    pub(crate) fn read_u16_be(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_be_bytes(self.read_array()?))
    }

    pub(crate) fn read_u16_le(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.read_array()?))
    }

    pub(crate) fn read_i16_le(&mut self) -> Result<i16, CodecError> {
        Ok(i16::from_le_bytes(self.read_array()?))
    }

    pub(crate) fn read_u32_be(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_be_bytes(self.read_array()?))
    }

    pub(crate) fn read_i32_be(&mut self) -> Result<i32, CodecError> {
        Ok(i32::from_be_bytes(self.read_array()?))
    }

    pub(crate) fn read_i64_be(&mut self) -> Result<i64, CodecError> {
        Ok(i64::from_be_bytes(self.read_array()?))
    }

    pub(crate) fn read_f32_be(&mut self) -> Result<f32, CodecError> {
        Ok(f32::from_bits(u32::from_be_bytes(self.read_array()?)))
    }

    pub(crate) fn read_i32_le(&mut self) -> Result<i32, CodecError> {
        Ok(i32::from_le_bytes(self.read_array()?))
    }

    pub(crate) fn read_i64_le(&mut self) -> Result<i64, CodecError> {
        Ok(i64::from_le_bytes(self.read_array()?))
    }

    pub(crate) fn read_f32_le(&mut self) -> Result<f32, CodecError> {
        Ok(f32::from_bits(u32::from_le_bytes(self.read_array()?)))
    }

    pub(crate) fn read_f64_le(&mut self) -> Result<f64, CodecError> {
        Ok(f64::from_bits(u64::from_le_bytes(self.read_array()?)))
    }

    pub(crate) fn read_string_u16(&mut self) -> Result<String, CodecError> {
        let len = usize::from(self.read_u16_be()?);
        let bytes = self.read_exact(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|_| CodecError::InvalidUtf8)
    }

    pub(crate) fn finish(self) -> Result<(), CodecError> {
        if self.remaining() == 0 {
            Ok(())
        } else {
            Err(CodecError::TrailingBytes {
                remaining: self.remaining(),
            })
        }
    }

    fn read_array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let bytes = self.read_exact(N)?;
        let mut array = [0_u8; N];
        array.copy_from_slice(bytes);
        Ok(array)
    }
}

pub(crate) struct Writer {
    output: Vec<u8>,
}

impl Writer {
    pub(crate) fn new() -> Self {
        Self { output: Vec::new() }
    }

    pub(crate) fn len(&self) -> usize {
        self.output.len()
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            output: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn put_u8(&mut self, value: u8) {
        self.output.push(value);
    }

    pub(crate) fn put_i8(&mut self, value: i8) {
        self.output.extend_from_slice(&value.to_ne_bytes());
    }

    pub(crate) fn put_u16_be(&mut self, value: u16) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn put_u16_le(&mut self, value: u16) {
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_i16_le(&mut self, value: i16) {
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_u32_be(&mut self, value: u32) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn put_i32_be(&mut self, value: i32) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn put_i64_be(&mut self, value: i64) {
        self.output.extend_from_slice(&value.to_be_bytes());
    }

    pub(crate) fn put_f32_be(&mut self, value: f32) {
        self.output
            .extend_from_slice(&value.to_bits().to_be_bytes());
    }

    pub(crate) fn put_i32_le(&mut self, value: i32) {
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_i64_le(&mut self, value: i64) {
        self.output.extend_from_slice(&value.to_le_bytes());
    }

    pub(crate) fn put_f32_le(&mut self, value: f32) {
        self.output
            .extend_from_slice(&value.to_bits().to_le_bytes());
    }

    pub(crate) fn put_f64_le(&mut self, value: f64) {
        self.output
            .extend_from_slice(&value.to_bits().to_le_bytes());
    }

    pub(crate) fn put_bytes(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    pub(crate) fn put_string_u16(
        &mut self,
        field: &'static str,
        value: &str,
    ) -> Result<(), CodecError> {
        let len = u16::try_from(value.len()).map_err(|_| CodecError::LengthOutOfRange {
            field,
            value: value.len(),
            max: usize::from(u16::MAX),
        })?;
        self.put_u16_be(len);
        self.put_bytes(value.as_bytes());
        Ok(())
    }

    pub(crate) fn into_vec(self) -> Vec<u8> {
        self.output
    }
}
