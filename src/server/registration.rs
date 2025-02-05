use std::sync::Arc;

use tracing::debug;

use crate::error::{IrcError, IrcResult};
use crate::server::{ClientId, Server};
use crate::ts6::TS6Message;

impl Server {
    pub async fn register_nickname(&self, nickname: &str, client_id: ClientId) -> IrcResult<()> {
        let nickname_lower = nickname.to_lowercase();
        let mut nicknames = self.nickname_map.write().await;

        if nicknames.contains_key(&nickname_lower) {
            return Err(IrcError::Protocol("Nickname is already in use".into()));
        }

        debug!("Registering nickname {} for client {}", nickname, client_id);
        nicknames.insert(nickname_lower, client_id);
        Ok(())
    }

    pub async fn unregister_nickname(&self, nickname: &str) {
        let nickname_lower = nickname.to_lowercase();
        let mut nicknames = self.nickname_map.write().await;
        nicknames.remove(&nickname_lower);
    }

    async fn check_registration_timeout(&self, client_id: ClientId) {
        if let Some(client) = self.get_client(client_id).await {
            let mut client = client.lock().await;
            if !client.is_registered() {
                client.send_message(&TS6Message::new(
                    "ERROR".to_string(),
                    vec!["Registration timeout".to_string()],
                )).await.ok();
                // Disconnect client
                self.remove_client(client_id).await;
            }
        }
    }

    pub(crate) async fn handle_server_capab(&self, msg: TS6Message) -> IrcResult<()> {
        // CAPAB capabilities...
        if msg.params.is_empty() {
            return Err(IrcError::Protocol("No capabilities specified".into()));
        }

        // TODO: Process capabilities
        Ok(())
    }
}