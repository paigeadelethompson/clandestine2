use chrono::{DateTime, Utc};
use tracing::debug;

use crate::channel::Channel;

impl Channel {
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