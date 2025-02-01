use super::TS6Message;
use std::collections::HashMap;
use tracing::debug;

pub fn parse_message(line: &str) -> Option<TS6Message> {
    debug!("Attempting to parse message: {:?}", line);
    let mut tags = HashMap::new();
    let mut rest = line;

    // Parse tags if present
    if line.starts_with('@') {
        let parts: Vec<_> = line[1..].splitn(2, ' ').collect();
        if parts.len() != 2 {
            debug!("Failed to parse message tags");
            return None;
        }
        
        for tag in parts[0].split(';') {
            if let Some((key, value)) = tag.split_once('=') {
                tags.insert(key.to_string(), value.to_string());
            }
        }
        
        rest = parts[1];
    }

    let mut parts = rest.splitn(2, ' ');
    
    let (source, rest) = if rest.starts_with(':') {
        let source = parts.next()?[1..].to_string();
        debug!("Found message source: {}", source);
        (Some(source), parts.next()?)
    } else {
        (None, rest)
    };

    let mut parts = rest.splitn(2, ' ');
    let command = parts.next()?.to_string();
    debug!("Found command: {}", command);
    
    let params = if let Some(param_str) = parts.next() {
        parse_params(param_str)
    } else {
        Vec::new()
    };

    debug!("Parsed message: command={}, params={:?}", command, params);

    Some(TS6Message {
        tags,
        source,
        command,
        params,
    })
}

fn parse_params(param_str: &str) -> Vec<String> {
    let mut params = Vec::new();
    let mut parts = param_str.split(' ');
    let mut trailing = String::new();
    let mut in_trailing = false;
    
    while let Some(part) = parts.next() {
        if part.starts_with(':') {
            in_trailing = true;
            trailing.push_str(&part[1..]);
        } else if in_trailing {
            trailing.push(' ');
            trailing.push_str(part);
        } else {
            params.push(part.to_string());
        }
    }
    
    if in_trailing {
        params.push(trailing);
    }
    
    params
} 