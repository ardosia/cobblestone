mod runner;

use bytes::Bytes;
use raknet_rust::server::PeerId;
use tokio::sync::oneshot;

use crate::{RaknetError, Reliability};

pub(crate) const COMMAND_QUEUE_CAPACITY: usize = 4096;
pub(crate) const PER_CONNECTION_INBOUND_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseState {
    Open,
    Closed,
    Backpressure,
}

pub(crate) enum BackendCommand {
    Send {
        peer_id: PeerId,
        payload: Bytes,
        reliability: Reliability,
        response: oneshot::Sender<Result<(), RaknetError>>,
    },
    Disconnect {
        peer_id: PeerId,
        response: oneshot::Sender<Result<(), RaknetError>>,
    },
    Shutdown {
        response: oneshot::Sender<Result<(), RaknetError>>,
    },
}

pub(crate) use runner::run_backend;
