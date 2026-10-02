use crate::{SessionDelivery, SessionPacket};

pub(super) enum SessionCommand {
    Send {
        packet: SessionPacket,
        delivery: SessionDelivery,
    },
    Disconnect,
}
