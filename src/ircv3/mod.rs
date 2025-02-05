use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    MultiPrefix,
    ExtendedJoin,
    ServerTime,
    MessageTags,
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Capability::MultiPrefix => write!(f, "multi-prefix"),
            Capability::ExtendedJoin => write!(f, "extended-join"),
            Capability::ServerTime => write!(f, "server-time"),
            Capability::MessageTags => write!(f, "message-tags"),
        }
    }
}

impl FromStr for Capability {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "multi-prefix" => Ok(Capability::MultiPrefix),
            "extended-join" => Ok(Capability::ExtendedJoin),
            "server-time" => Ok(Capability::ServerTime),
            "message-tags" => Ok(Capability::MessageTags),
            _ => Err(()),
        }
    }
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::MultiPrefix => "multi-prefix",
            Capability::ExtendedJoin => "extended-join",
            Capability::ServerTime => "server-time",
            Capability::MessageTags => "message-tags",
        }
    }
} 
