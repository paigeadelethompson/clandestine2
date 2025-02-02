pub mod parser;

use std::time::{SystemTime, UNIX_EPOCH};
use std::collections::HashMap;

pub fn generate_ts() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[derive(Debug)]
pub struct TS6Message {
    pub tags: HashMap<String, String>,
    pub source: Option<String>,
    pub command: String,
    pub params: Vec<String>,
}

impl TS6Message {
    pub fn new(command: String, params: Vec<String>) -> Self {
        Self {
            tags: HashMap::new(),
            source: None,
            command,
            params,
        }
    }

    pub fn with_source(source: String, command: String, params: Vec<String>) -> Self {
        Self {
            tags: HashMap::new(),
            source: Some(source),
            command,
            params,
        }
    }

    pub fn to_string(&self) -> String {
        let mut result = String::new();
        
        // Add tags if present
        if !self.tags.is_empty() {
            result.push('@');
            let tags: Vec<_> = self.tags.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            result.push_str(&tags.join(";"));
            result.push(' ');
        }
        
        if let Some(src) = &self.source {
            result.push(':');
            result.push_str(src);
            result.push(' ');
        }
        
        result.push_str(&self.command);
        
        for param in &self.params {
            result.push(' ');
            if param.contains(' ') {
                result.push(':');
                result.push_str(param);
                break;
            } else {
                result.push_str(param);
            }
        }
        
        result
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests; 