use std::collections::HashSet;

use tracing::debug;

use crate::channel::Channel;

#[derive(Clone)]
pub struct ChannelMember {
    client_id: u32,
    modes: Vec<char>,
}

impl Channel {
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
}