use std::sync::Arc;

/// Immutable native-owned byte storage.
///
/// Constructing a buffer copies or takes ownership explicitly. Cloning the Rust value shares
/// the immutable allocation and does not duplicate the bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBuffer {
    bytes: Arc<[u8]>,
}

impl NativeBuffer {
    /// Copies bytes into immutable native storage.
    pub fn copy_from_slice(bytes: &[u8]) -> Self {
        Self {
            bytes: Arc::from(bytes),
        }
    }

    /// Takes ownership of a byte vector without retaining mutable access to its contents.
    pub fn from_vec(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::from(bytes.into_boxed_slice()),
        }
    }

    /// Number of bytes in the buffer.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Immutable bytes.
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    /// Whether two buffers share the same native allocation.
    pub fn shares_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes)
    }
}

impl AsRef<[u8]> for NativeBuffer {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl From<Vec<u8>> for NativeBuffer {
    fn from(bytes: Vec<u8>) -> Self {
        Self::from_vec(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::NativeBuffer;

    #[test]
    fn copy_from_slice_is_independent_of_source() {
        let mut source = vec![1, 2, 3];
        let buffer = NativeBuffer::copy_from_slice(&source);
        source[0] = 9;
        assert_eq!(buffer.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn clone_shares_immutable_storage() {
        let buffer = NativeBuffer::from_vec(vec![4, 5, 6]);
        let cloned = buffer.clone();
        assert!(buffer.shares_storage_with(&cloned));
        assert_eq!(cloned.as_slice(), &[4, 5, 6]);
    }
}
