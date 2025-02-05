use crate::client::Client;
use crate::error::IrcResult;
use crate::ts6::TS6Message;

impl Client {
    pub async fn send_numeric(&self, numeric: u16, params: &[&str]) -> IrcResult<()> {
        let numeric_str = format!("{:03}", numeric);
        let mut message_params = vec![];

        if let Some(nick) = &self.nickname {
            message_params.push(nick.clone());
        } else {
            message_params.push("*".to_string());
        }

        message_params.extend(params.iter().map(|&s| s.to_string()));

        let mut message = TS6Message::new(numeric_str, message_params);
        // Add server name as source for numeric replies
        message.source = Some(self.server_name.clone());
        self.send_message(&message).await
    }
}