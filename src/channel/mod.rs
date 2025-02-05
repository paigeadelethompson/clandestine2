use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::Mutex;
use tracing::debug;

use crate::channel::list::Ban;
use crate::client::Client;

#[cfg(test)]
mod tests;
mod mode;
mod list;
mod member;
mod topic;

#[derive(Clone)]
pub struct Channel {
    pub(crate) name: String,
    topic: Option<String>,
    topic_setter: Option<String>,
    topic_time: DateTime<Utc>,
    members: HashSet<u32>, // ClientIds
    pub(crate) modes: HashSet<char>,  // Channel modes like +n, +t, etc
    mode_params: HashMap<char, String>, // For modes that take parameters like +k (key)
    created_at: u64,
    bans: Vec<Ban>,
    operators: HashSet<u32>,
    voices: HashSet<u32>,
}

impl Channel {
    pub fn new(name: String) -> Self {
        debug!("Creating new channel: {}", name);
        let mut channel = Self {
            name,
            topic: None,
            topic_setter: None,
            topic_time: Utc::now(),
            members: HashSet::new(),
            modes: HashSet::new(),
            mode_params: HashMap::new(),
            created_at: crate::ts6::generate_ts(),
            bans: Vec::new(),
            operators: HashSet::new(),
            voices: HashSet::new(),
        };

        // Set default modes +nt
        channel.set_mode('n', None, true);
        channel.set_mode('t', None, true);

        channel
    }
}

