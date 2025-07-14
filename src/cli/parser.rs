// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

#[derive(Debug)]
pub enum ParsedCommand {
    Set { address: String, key: String, value: String },
    Get { address: String, key: String },
    Del { address: String, key: String },
    Keys { address: String },
    Error(String),
}

pub fn parse_command_line(input: &str) -> Option<ParsedCommand> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let parts: Vec<&str> = input.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        _ => {
            let address = parts[0].to_string();
            if parts.len() < 2 {
                return None;
            }

            let command = parts[1];
            match command {
                "set" => {
                    if parts.len() < 4 {
                        return Some(ParsedCommand::Error(format!("Usage: {} set <key> <value>", address)));
                    }

                    let key = parts[2];

                    let value = if input.contains('"') {
                        parse_quoted_value(input)?
                    } else {
                        parts[3..].join(" ")
                    };

                    Some(ParsedCommand::Set { address, key: key.to_string(), value })
                }
                "get" => {
                    if parts.len() != 3 {
                        return Some(ParsedCommand::Error(format!("Usage: {} get <key>", address)));
                    }
                    let key = parts[2];
                    Some(ParsedCommand::Get { address, key: key.to_string() })
                }
                "del" => {
                    if parts.len() != 3 {
                        return Some(ParsedCommand::Error(format!("Usage: {} del <key>", address)));
                    }
                    let key = parts[2];
                    Some(ParsedCommand::Del { address, key: key.to_string() })
                }
                "keys" => {
                    if parts.len() != 2 {
                        return Some(ParsedCommand::Error(format!("Usage: {} keys", address)));
                    }
                    Some(ParsedCommand::Keys { address })
                }
                _ => Some(ParsedCommand::Error(format!("Unknown command: {}", command))),
            }
        }
    }
}



fn parse_quoted_value(input: &str) -> Option<String> {
    if let Some(start) = input.find('"') {
        if let Some(end) = input.rfind('"') {
            if start != end {
                return Some(input[start+1..end].to_string());
            }
        }
    }
    None
} 