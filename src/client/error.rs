use crate::client::Client;
use crate::error::IrcResult;

impl Client {
    pub async fn send_error(&self, msg: &str) -> IrcResult<()> {
        let error_msg = format!("ERROR :{}\r\n", msg);
        self.write_raw(error_msg.as_bytes()).await
    }
}