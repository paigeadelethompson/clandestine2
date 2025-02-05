use chrono::{DateTime, Utc};

use crate::channel::Channel;

#[derive(Clone)]
pub struct Ban {
    pub mask: String,
    pub set_by: String,
    pub set_time: DateTime<Utc>,
}

impl Channel {}