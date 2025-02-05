use crate::server::Server;

#[derive(Default)]
pub struct ServerStats {
    pub visible_users: usize,
    pub invisible_users: usize,
    pub server_count: usize,
    pub oper_count: usize,
    pub channel_count: usize,
    pub local_users: usize,
    pub local_servers: usize,
    pub max_local_users: usize,
    pub global_users: usize,
    pub max_global_users: usize,
}

impl Server {
    pub async fn get_stats(&self) -> ServerStats {
        let mut stats = ServerStats::default();

        let client_count = {
            let clients = self.clients.read().await;
            clients.len()
        };

        let channel_count = {
            let channels = self.channels.read().await;
            channels.len()
        };

        stats.local_users = client_count;
        stats.global_users = client_count;
        stats.channel_count = channel_count;
        stats.max_local_users = client_count;
        stats.max_global_users = client_count;
        stats.server_count = 1;

        stats
    }
}