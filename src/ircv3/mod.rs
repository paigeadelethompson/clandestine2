use std::collections::HashSet;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Capability {
    MultiPrefix,
    ExtendedJoin,
    AccountNotify,
    AwayNotify,
    ChgHost,
    ServerTime,
    MessageTags,
    Batch,
    LabeledResponse,
    // Add more capabilities as needed
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::MultiPrefix => "multi-prefix",
            Capability::ExtendedJoin => "extended-join",
            Capability::AccountNotify => "account-notify",
            Capability::AwayNotify => "away-notify",
            Capability::ChgHost => "chghost",
            Capability::ServerTime => "server-time",
            Capability::MessageTags => "message-tags",
            Capability::Batch => "batch",
            Capability::LabeledResponse => "labeled-response",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "multi-prefix" => Some(Capability::MultiPrefix),
            "extended-join" => Some(Capability::ExtendedJoin),
            "account-notify" => Some(Capability::AccountNotify),
            "away-notify" => Some(Capability::AwayNotify),
            "chghost" => Some(Capability::ChgHost),
            "server-time" => Some(Capability::ServerTime),
            "message-tags" => Some(Capability::MessageTags),
            "batch" => Some(Capability::Batch),
            "labeled-response" => Some(Capability::LabeledResponse),
            _ => None,
        }
    }
} 