#![deny(missing_docs)]

//! Production fixed-target session mechanism above Cobblestone's RakNet transport and wire codec.
//!
//! This crate hides RakNet connection objects and Batch/compression framing from the owning
//! runtime. It does not own gameplay semantics, players, worlds, plugins, or PHP/Zend state.

mod delivery;
mod error;
mod host;
mod id;
mod packet;
mod server;
mod session;
mod wire;

pub use delivery::SessionDelivery;
pub use error::SessionError;
pub use host::{SessionHost, SessionHostConfig, SessionHostError, SessionHostEvent};
pub use id::SessionId;
pub use packet::SessionPacket;
pub use server::SessionServer;
pub use session::Session;
