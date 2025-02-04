use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::config::{ALine, DLine, GLine, ILine, KLine, OLine, ULine};

#[derive(Default, Serialize, Deserialize)]
pub struct DatabaseContent {
    klines: Vec<KLine>,
    dlines: Vec<DLine>,
    glines: Vec<GLine>,
    ilines: Vec<ILine>,
    olines: Vec<OLine>,
    ulines: Vec<ULine>,
    alines: Vec<ALine>,
}

pub struct Database {
    path: PathBuf,
    content: Arc<RwLock<DatabaseContent>>,
}

impl Database {
    pub async fn new<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let path = path.as_ref().to_path_buf();
        let content = if path.exists() {
            let data = fs::read_to_string(&path)?;
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            DatabaseContent::default()
        };

        Ok(Self {
            path,
            content: Arc::new(RwLock::new(content)),
        })
    }

    async fn save(&self) -> Result<(), std::io::Error> {
        let content = self.content.read().await;
        let data = serde_json::to_string_pretty(&*content)?;
        fs::write(&self.path, data)?;
        Ok(())
    }

    pub async fn add_kline(&self, kline: KLine) -> Result<(), std::io::Error> {
        let mut content = self.content.write().await;
        content.klines.push(kline);
        drop(content);
        self.save().await
    }

    pub async fn get_klines(&self) -> Vec<KLine> {
        self.content.read().await.klines.clone()
    }

    pub async fn remove_kline(&self, mask: &str) -> Result<(), std::io::Error> {
        let mut content = self.content.write().await;
        content.klines.retain(|k| k.mask != mask);
        drop(content);
        self.save().await
    }

    // Similar methods for other line types...
}

// ... rest of the implementation ... 