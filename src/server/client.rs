use std::sync::{Arc, mpsc};

use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::client::Client;
use crate::server::{ClientId, Server};

pub enum ServerMessage {
    WhoisLookup {
        nickname: String,
        respond_to: mpsc::Sender<Option<WhoisInfo>>,
    },
}

#[derive(Clone)]
pub struct WhoisInfo {
    pub nickname: String,
    pub username: String,
    pub hostname: String,
    pub realname: String,
}

impl Server {
    pub(crate) async fn get_client(&self, id: ClientId) -> Option<Arc<Mutex<Client>>> {
        let client_map = self.client_map.read().await;
        client_map.get(&id).cloned()
    }

    // Update add_client to store in both the list and map
    pub async fn add_client(&self, client: Arc<Mutex<Client>>) {
        let id = client.lock().await.id();
        let mut clients = self.clients.write().await;
        let mut client_map = self.client_map.write().await;

        clients.push(id);
        client_map.insert(id, client);
        debug!("Added client {} to server", id);
    }

    // Update remove_client to be more thorough
    pub async fn remove_client(&self, id: ClientId) {
        debug!("Removing client {} from server", id);

        // Remove from client list
        let mut clients = self.clients.write().await;
        if let Some(pos) = clients.iter().position(|&cid| cid == id) {
            clients.swap_remove(pos);
        }

        // Remove from client map
        let mut client_map = self.client_map.write().await;
        if client_map.remove(&id).is_some() {
            info!("Removed client {} from server", id);
        }

        // Could also clean up from channels here if needed
        debug!("Client {} cleanup completed", id);
    }

    pub async fn find_client_info(&self, nickname: &str) -> Option<WhoisInfo> {
        let (resp_tx, resp_rx) = mpsc::channel();

        self.tx.send(ServerMessage::WhoisLookup {
            nickname: nickname.to_string(),
            respond_to: resp_tx,
        }).expect("Server task died");

        resp_rx.recv().expect("Server task died")
    }

    pub async fn find_client_by_nick(&self, nickname: &str) -> Option<Arc<Mutex<Client>>> {
        let nickname_lower = nickname.to_lowercase();
        debug!("find_client_by_nick: Looking for nickname {} (lowercase: {})", nickname, nickname_lower);

        // First look up the client ID in the nickname map
        let client_id = {
            let nicknames = self.nickname_map.read().await;
            nicknames.get(&nickname_lower).copied()
        };

        // Then get the client from the client map if we found an ID
        if let Some(id) = client_id {
            let clients = self.client_map.read().await;
            return clients.get(&id).cloned();
        }

        debug!("find_client_by_nick: No match found for {}", nickname);
        None
    }
}