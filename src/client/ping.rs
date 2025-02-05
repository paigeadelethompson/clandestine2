use std::time::{Duration, Instant};

use tracing::debug;

use crate::client::Client;

impl Client {
    pub(crate) fn start_ping_timer(&mut self) {
        let client_id = self.id;
        let tx = self.sendq_tx.clone();  // Use sendq_tx instead of tx for PINGs
        let mut pong_rx = self.pong_tx.subscribe();
        let server_name = self.server_name.clone();
        let ping_interval = self.ping_interval;
        let ping_timeout = self.ping_timeout;

        self.ping_timer = Some(tokio::spawn(async move {
            let mut interval = tokio::time::interval(ping_interval);
            interval.tick().await; // Skip first tick

            let mut last_ping = None;

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        let now = Instant::now();

                        // Only check timeout if we've sent a ping
                        if let Some(ping_time) = last_ping {
                            if now.duration_since(ping_time) > ping_timeout {
                                debug!("Client {} ping timeout - no PONG received", client_id);
                                let timeout_msg = "ERROR :Ping timeout\r\n".as_bytes().to_vec();
                                if tx.send(timeout_msg).is_err() {
                                    debug!("Failed to send timeout message - channel closed");
                                }
                                break;
                            }
                        }

                        // Send a new ping - add colon before param to match client's PONG format
                        debug!("Server sending PING to client {}", client_id);
                        let ping_msg = format!(":{} PING :{}\r\n", server_name, server_name);
                        match tx.send(ping_msg.into_bytes()) {
                            Ok(_) => {
                                debug!("Successfully queued PING message for client {}", client_id);
                                last_ping = Some(now);
                            }
                            Err(e) => {
                                debug!("Failed to send PING message: {} - channel closed", e);
                                break;
                            }
                        }
                    }

                    Ok(_) = pong_rx.recv() => {
                        debug!("Received PONG update for client {}", client_id);
                        last_ping = None; // Reset ping timer when we get a PONG
                    }
                }
            }
            debug!("Ping timer task exiting for client {}", client_id);
        }));
    }
}