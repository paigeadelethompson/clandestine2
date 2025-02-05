use crate::channel::Channel;

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

impl Channel {
    pub fn has_mode(&self, mode: char, target: Option<&str>) -> bool {
        match target {
            Some(nick) => {
                if let Some(param) = self.mode_params.get(&mode) {
                    param == nick
                } else {
                    false
                }
            }
            None => self.modes.contains(&mode)
        }
    }

    pub fn get_modes(&self) -> String {
        let mut modes = String::new();
        for mode in &self.modes {
            modes.push(*mode);
        }
        modes
    }

    pub fn set_mode(&mut self, mode: char, param: Option<String>, adding: bool) {
        if adding {
            self.modes.insert(mode);
            if let Some(param) = param {
                self.mode_params.insert(mode, param);
            }
        } else {
            self.modes.remove(&mode);
            self.mode_params.remove(&mode);
        }
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