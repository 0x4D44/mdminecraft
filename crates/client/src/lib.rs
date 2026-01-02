#![warn(missing_docs)]
//! Thin client façade for prediction + presentation glue.

pub mod multiplayer;

use anyhow::Result;
use mdminecraft_server::Server;

/// Placeholder singleplayer client that embeds the server.
pub struct Client {
    server: Server,
}

impl Client {
    /// Spin up a client with an embedded server for local testing.
    pub fn singleplayer() -> Self {
        Self {
            server: Server::new(),
        }
    }

    /// Advance both client + server by one tick.
    pub fn frame(&mut self) -> Result<()> {
        self.server.tick()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn singleplayer_frame_runs() {
        let mut client = Client::singleplayer();
        client.frame().expect("frame succeeds");
    }
}
