use crate::client::Client;
use crate::config::KLine;
use crate::database::Database;
use crate::server::Server;

impl Server {
    pub async fn has_oline(&self, client: &Client) -> bool {
        let mask = client.get_mask();
        self.config.access.olines.iter()
            .any(|oline| self.mask_match(&mask, &oline.mask))
    }

    pub async fn is_host_klined(&self, host: &str) -> bool {
        self.config.access.klines.iter()
            .any(|k| self.mask_match(host, &k.mask))
    }

    pub(crate) async fn load_persisted_lines(&self, db: &Database) -> Result<(), Box<dyn std::error::Error>> {
        // Load lines from database and merge with config
        let mut access = self.config.access.clone();

        // Load and merge K-lines
        let db_klines = db.get_klines().await;
        access.klines.extend(db_klines);

        // Load and merge other line types...
        Ok(())
    }

    pub async fn add_kline(&self, kline: KLine) -> Result<(), Box<dyn std::error::Error>> {
        let mut access = self.config.access.clone();
        access.klines.push(kline);
        Ok(())
    }

    pub async fn remove_kline(&self, mask: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut access = self.config.access.clone();
        access.klines.retain(|k| k.mask != mask);
        Ok(())
    }
}