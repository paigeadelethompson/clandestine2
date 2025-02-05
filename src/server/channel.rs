use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{debug, warn};

use crate::channel::Channel;
use crate::error::{IrcError, IrcResult};
use crate::server::Server;
use crate::ts6::TS6Message;

impl Server {
    pub async fn get_client_channels(&self, client_id: u32) -> Vec<String> {
        let mut client_channels = Vec::new();
        let channels = self.channels.read().await;

        for (channel_name, channel) in channels.iter() {
            let channel = channel.read().await;
            if channel.get_members().contains(&client_id) {
                client_channels.push(channel_name.clone());
            }
        }

        client_channels
    }

    pub async fn remove_from_channel(&self, channel_name: &str, client_id: u32) -> IrcResult<()> {
        debug!("Server removing client {} from channel {}", client_id, channel_name);

        if let Some(channel) = self.get_channel(channel_name).await {
            let mut channel = channel.write().await;
            channel.remove_member(client_id);
            debug!("Successfully removed client {} from channel {}", client_id, channel_name);
            Ok(())
        } else {
            debug!("Channel {} not found when removing client {}", channel_name, client_id);
            Err(IrcError::Protocol("No such channel".into()))
        }
    }

    pub async fn get_or_create_channel(&self, name: &str) -> Arc<RwLock<Channel>> {
        let mut channels = self.channels.write().await;
        if let Some(channel) = channels.get(name) {
            channel.clone()
        } else {
            let channel = Arc::new(RwLock::new(Channel::new(name.to_string())));
            channels.insert(name.to_string(), channel.clone());
            channel
        }
    }

    pub async fn get_channel(&self, name: &str) -> Option<Arc<RwLock<Channel>>> {
        let channels = self.channels.read().await;
        channels.get(name).cloned()
    }

    pub async fn broadcast_to_channel(&self, channel_name: &str, message: &TS6Message, skip_client: Option<u32>) -> IrcResult<()> {
        let member_ids = {
            let channels = self.channels.read().await;
            if let Some(channel) = channels.get(channel_name) {
                let channel = channel.read().await;
                channel.get_members().iter().cloned().collect::<Vec<_>>()
            } else {
                return Ok(());
            }
        };

        for client_id in member_ids {
            if Some(client_id) == skip_client {
                continue;
            }
            if let Some(client) = self.get_client(client_id).await {
                // Don't hold lock across await point
                let client = client.lock().await;
                debug!("Broadcasting to client {}: {:?}", client_id, message);
                if let Err(e) = client.send_message(message).await {
                    warn!("Failed to send message to client {}: {}", client_id, e);
                }
            }
        }

        Ok(())
    }

    pub async fn check_channel_membership(&self, channel_name: &str, client_id: u32) -> bool {
        let channels = self.channels.read().await;
        if let Some(channel) = channels.get(channel_name) {
            let channel = channel.read().await;
            channel.get_members().contains(&client_id)
        } else {
            false
        }
    }
}