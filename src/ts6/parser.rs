use std::collections::HashMap;

use tracing::debug;

use super::TS6Message;

pub fn parse_message(line: &str) -> Result<TS6Message, String> {
    debug!("Attempting to parse message: {:?}", line);
    if line.is_empty() {
        return Err("Empty message".to_string());
    }

    let mut tags = HashMap::new();
    let mut rest = line;

    // Parse tags if present
    if line.starts_with('@') {
        let parts: Vec<_> = line[1..].splitn(2, ' ').collect();
        if parts.len() != 2 {
            return Err("Failed to parse message tags".to_string());
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
        let source = parts.next()
            .ok_or("Missing source")?[1..].to_string();
        let rest = parts.next()
            .ok_or("Missing command after source")?;
        (Some(source), rest)
    } else {
        (None, rest)
    };

    let mut parts = rest.splitn(2, ' ');
    let command = parts.next()
        .ok_or("Missing command")?
        .to_string();

    let params = if let Some(param_str) = parts.next() {
        parse_params(param_str)
    } else {
        Vec::new()
    };

    Ok(TS6Message {
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