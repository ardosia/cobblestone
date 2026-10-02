use cobblestone_core::NativeBuffer;
use cobblestone_protocol84::RawPacket;

/// One decoded fixed-target packet delivered to the owning runtime.
///
/// The outer game marker and Batch/compression envelope are already removed. Packet bodies remain
/// wire data and must not be exposed as ordinary gameplay/plugin APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionPacket {
    id: u8,
    body: NativeBuffer,
}

impl SessionPacket {
    /// Creates one packet from its packet ID and body bytes.
    #[must_use]
    pub fn new(id: u8, body: NativeBuffer) -> Self {
        Self { id, body }
    }

    /// Returns the packet ID.
    #[must_use]
    pub const fn id(&self) -> u8 {
        self.id
    }

    /// Returns the bytes after the packet ID.
    #[must_use]
    pub const fn body(&self) -> &NativeBuffer {
        &self.body
    }

    pub(crate) fn from_raw(raw: &RawPacket) -> Self {
        Self {
            id: raw.id(),
            body: raw.body().clone(),
        }
    }

    pub(crate) fn to_raw(&self) -> RawPacket {
        RawPacket::new(self.id, self.body.clone())
    }
}
