#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Fixed-target RakNet transport facade for Cobblestone.
//!
//! This crate owns only RakNet/UDP transport lifecycle and opaque connected payload delivery.
//! MCPE protocol-84 packets, gameplay semantics, sessions above transport, players, worlds, and
//! plugins do not belong here.

mod backend;
mod config;
mod connection;
mod error;
mod reliability;
mod server;

pub use config::{NetworkConfig, NetworkConfigError};
pub use connection::Connection;
pub use error::NetworkError;
pub use reliability::Reliability;
pub use server::NetworkServer;
