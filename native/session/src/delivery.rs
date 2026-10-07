use cobblestone_raknet::Reliability;

/// Delivery semantics requested by the session layer.
///
/// This mirrors the transport choices without exposing the RakNet dependency itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionDelivery {
    /// Best-effort delivery without retransmission or ordering.
    Unreliable,
    /// Best-effort sequenced delivery where newer messages supersede older ones.
    UnreliableSequenced,
    /// Retransmit until acknowledged without ordering.
    Reliable,
    /// Retransmit and deliver in order.
    ReliableOrdered,
    /// Retransmit with sequenced supersession.
    ReliableSequenced,
}

impl SessionDelivery {
    pub(crate) const fn reliability(self) -> Reliability {
        match self {
            Self::Unreliable => Reliability::Unreliable,
            Self::UnreliableSequenced => Reliability::UnreliableSequenced,
            Self::Reliable => Reliability::Reliable,
            Self::ReliableOrdered => Reliability::ReliableOrdered,
            Self::ReliableSequenced => Reliability::ReliableSequenced,
        }
    }
}
