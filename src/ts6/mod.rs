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
        let mut parts = Vec::new();
        
        // Add source if present
        if let Some(ref source) = self.source {
            parts.push(format!(":{}", source));
        }
        
        // Add command
        parts.push(self.command.clone());
        
        // Add parameters
        if !self.params.is_empty() {
            // Add all parameters except the last one
            if self.params.len() > 1 {
                parts.extend(self.params[..self.params.len()-1].iter().cloned());
            }
            
            // Add last parameter with colon if it contains spaces or is empty
            let last_param = &self.params[self.params.len()-1];
            if last_param.contains(' ') || last_param.is_empty() {
                parts.push(format!(":{}", last_param));
            } else {
                parts.push(format!(":{}", last_param)); // Always add colon for trailing parameter
            }
        }
        
        parts.join(" ")
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests; 