// Adapted for Cobblestone from Ardosia network transport code under Apache-2.0.
// See docs/provenance/ARDOSIA_REUSE.md for exact source revisions and modifications.

use raknet_rust::server::RaknetServer;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::backend::{BackendCommand, COMMAND_QUEUE_CAPACITY, run_backend};
use crate::connection::Connection;
use crate::{NetworkConfig, NetworkError};

/// Asynchronous fixed-target RakNet listener.
pub struct NetworkServer {
    accept_rx: mpsc::Receiver<Result<Connection, NetworkError>>,
    commands: mpsc::Sender<BackendCommand>,
    backend: JoinHandle<()>,
}

impl NetworkServer {
    /// Binds and starts a protocol-8 network listener.
    pub async fn bind(config: NetworkConfig) -> Result<Self, NetworkError> {
        let transport = config.to_transport_config()?;
        let mut builder = RaknetServer::builder().transport_config(transport);
        if let Some(worker_shards) = config.worker_shards() {
            builder = builder.shard_count(worker_shards.get());
        }
        let server = builder.start().await?;

        let (accept_tx, accept_rx) = mpsc::channel(config.max_connections().get());
        let (command_tx, command_rx) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let backend = tokio::spawn(run_backend(
            server,
            command_rx,
            accept_tx,
            command_tx.clone(),
        ));

        Ok(Self {
            accept_rx,
            commands: command_tx,
            backend,
        })
    }

    /// Waits for the next accepted RakNet connection.
    pub async fn accept(&mut self) -> Result<Connection, NetworkError> {
        self.accept_rx
            .recv()
            .await
            .ok_or(NetworkError::BackendStopped)?
    }

    /// Gracefully shuts down the listener and waits for the backend task to exit.
    pub async fn shutdown(self) -> Result<(), NetworkError> {
        let Self {
            accept_rx: _,
            commands,
            backend,
        } = self;

        let (response_tx, response_rx) = oneshot::channel();
        commands
            .send(BackendCommand::Shutdown {
                response: response_tx,
            })
            .await
            .map_err(|_| NetworkError::BackendStopped)?;

        let shutdown_result = response_rx
            .await
            .map_err(|_| NetworkError::BackendStopped)?;

        backend
            .await
            .map_err(|error| NetworkError::BackendFailure {
                message: format!("backend task join failed: {error}"),
            })?;

        shutdown_result
    }
}
