use tracing::debug;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::client::Client;
use chrono::{DateTime, Utc};

#[derive(Clone)]
pub struct Channel {
    pub(crate) name: String,
    topic: Option<String>,
    topic_setter: Option<String>,
    topic_time: DateTime<Utc>,
    members: HashSet<u32>, // ClientIds
    pub(crate) modes: ChannelModes,
    created_at: u64,
    bans: Vec<Ban>,
}

#[derive(Clone)]
pub struct ChannelMember {
    client_id: u32,
    modes: Vec<char>,
}

#[derive(Clone)]
pub struct ChannelModes {
    pub(crate) invite_only: bool,
    pub(crate) moderated: bool,
    pub(crate) no_external_messages: bool,
    pub(crate) secret: bool,
    pub(crate) topic_protection: bool,
    pub(crate) key: Option<String>,
    pub(crate) limit: Option<usize>,
}

#[derive(Clone)]
pub struct Ban {
    pub mask: String,
    pub set_by: String,
    pub set_time: DateTime<Utc>,
}

impl Channel {
    pub fn new(name: String) -> Self {
        debug!("Creating new channel: {}", name);
        Self {
            name,
            topic: None,
            topic_setter: None,
            topic_time: Utc::now(),
            members: HashSet::new(),
            modes: ChannelModes::default(),
            created_at: crate::ts6::generate_ts(),
            bans: Vec::new(),
        }
    }

    pub async fn broadcast_message(&self, message: &str, exclude_client: Option<u32>) {
        // Implementation for broadcasting messages to channel members
    }

    pub fn add_member(&mut self, client_id: u32) {
        debug!("Adding client {} to channel {}", client_id, self.name);
        self.members.insert(client_id);
        debug!("Channel {} now has {} members", self.name, self.members.len());
    }

    pub fn remove_member(&mut self, client_id: u32) {
        debug!("Removing client {} from channel {}", client_id, self.name);
        self.members.remove(&client_id);
        debug!("Channel {} now has {} members", self.name, self.members.len());
    }

    pub fn get_members(&self) -> &HashSet<u32> {
        &self.members
    }

    pub fn get_modes_string(&self) -> String {
        let mut modes = String::from("+");
        if self.modes.invite_only { modes.push('i'); }
        if self.modes.moderated { modes.push('m'); }
        if self.modes.no_external_messages { modes.push('n'); }
        if self.modes.secret { modes.push('s'); }
        if self.modes.topic_protection { modes.push('t'); }
        
        let mut params = Vec::new();
        if let Some(limit) = self.modes.limit {
            modes.push('l');
            params.push(limit.to_string());
        }
        if let Some(ref key) = self.modes.key {
            modes.push('k');
            params.push(key.clone());
        }

        if !params.is_empty() {
            modes.push(' ');
            modes.push_str(&params.join(" "));
        }
        modes
    }

    pub fn get_bans(&self) -> &[Ban] {
        &self.bans
    }

    pub fn add_ban(&mut self, mask: String, set_by: String) {
        debug!("Adding ban {} to channel {} (set by {})", mask, self.name, set_by);
        self.bans.push(Ban {
            mask,
            set_by,
            set_time: Utc::now(),
        });
        debug!("Channel {} now has {} bans", self.name, self.bans.len());
    }

    pub fn remove_ban(&mut self, mask: &str) {
        debug!("Removing ban {} from channel {}", mask, self.name);
        let before_len = self.bans.len();
        self.bans.retain(|ban| ban.mask != mask);
        let removed = before_len - self.bans.len();
        debug!("Removed {} ban(s) from channel {}", removed, self.name);
    }

    pub fn get_topic(&self) -> Option<String> {
        self.topic.clone()
    }

    pub fn set_topic(&mut self, topic: String, setter: String) {
        debug!("Setting topic for channel {} to: {}", self.name, topic);
        self.topic = Some(topic);
        self.topic_setter = Some(setter);
        self.topic_time = Utc::now();
    }

    pub fn get_topic_details(&self) -> (Option<String>, Option<String>, DateTime<Utc>) {
        (self.topic.clone(), self.topic_setter.clone(), self.topic_time)
    }
}

impl Default for ChannelModes {
    fn default() -> Self {
        Self {
            invite_only: false,
            moderated: false,
            no_external_messages: true,
            secret: false,
            topic_protection: true,
            key: None,
            limit: None,
        }
    }
} 