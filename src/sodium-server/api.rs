// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use crate::threading;
use crate::core::CacheError;
use crate::search::SearchType;
use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpStream};
use tokio::io::AsyncWriteExt;
use tracing::{info, error, warn};

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("Invalid command format: {0}")]
    InvalidCommand(String),
    #[error("Cache error: {0}")]
    CacheError(#[from] CacheError),
    #[error("Network error: {0}")]
    NetworkError(#[from] std::io::Error),
    #[error("UTF-8 decode error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone)]
pub enum Command {
    Set { key: String, value: String },
    Get { key: String },
    Delete { key: String },
    Keys,
    Search { search_type: SearchType, queries: Vec<String> },
}

impl Command {
    pub fn parse(input: &str) -> ApiResult<Self> {
        let input = input.trim();
        if input.is_empty() {
            return Err(ApiError::InvalidCommand("Empty command".to_string()));
        }

        // Special case for 'keys' without parentheses
        if input.to_lowercase() == "keys" {
            return Ok(Command::Keys);
        }

        // All other commands must use function syntax
        if !Self::is_function_syntax(input) {
            return Err(ApiError::InvalidCommand("Invalid command format".to_string()));
        }

        Self::parse_function_syntax(input)
    }

    fn is_function_syntax(input: &str) -> bool {
        input.contains('(') && input.ends_with(')')
    }

    fn parse_function_syntax(input: &str) -> ApiResult<Self> {
        let open_paren = input.find('(').ok_or_else(|| {
            ApiError::InvalidCommand("Invalid function syntax".to_string())
        })?;
        
        let function_name = input[..open_paren].trim();
        let args_str = &input[open_paren + 1..input.len() - 1];
        
        match function_name.to_lowercase().as_str() {
            "set" => {
                let (key, value) = Self::parse_function_args(args_str, 2)?;
                Self::validate_key(&key)?;
                Ok(Command::Set { key, value })
            }
            "get" => {
                let args = Self::parse_function_args_single(args_str)?;
                Self::validate_key(&args)?;
                Ok(Command::Get { key: args })
            }
            "delete" | "del" => {
                let args = Self::parse_function_args_single(args_str)?;
                Self::validate_key(&args)?;
                Ok(Command::Delete { key: args })
            }
            "keys" => {
                if !args_str.trim().is_empty() {
                    return Err(ApiError::InvalidCommand(
                        "keys() takes no arguments".to_string(),
                    ));
                }
                Ok(Command::Keys)
            }
            "search" => {
                let (search_type_str, queries) = Self::parse_search_args(args_str)?;
                let search_type = SearchType::parse(&search_type_str)
                    .map_err(|e| ApiError::InvalidCommand(e))?;
                Ok(Command::Search { search_type, queries })
            }
            cmd => Err(ApiError::InvalidCommand(format!(
                "Unknown function: {}. Supported functions: set, get, delete/del, keys, search",
                cmd
            ))),
        }
    }

    fn parse_function_args_single(args_str: &str) -> ApiResult<String> {
        let args_str = args_str.trim();
        if args_str.is_empty() {
            return Err(ApiError::InvalidCommand("Function requires an argument".to_string()));
        }
        
        let unquoted = Self::unquote_string(args_str);
        Ok(unquoted)
    }

    fn parse_function_args(args_str: &str, expected_count: usize) -> ApiResult<(String, String)> {
        let args_str = args_str.trim();
        if args_str.is_empty() {
            return Err(ApiError::InvalidCommand(
                format!("Function requires {} arguments", expected_count)
            ));
        }

        // Split by comma, but respect quoted strings
        let args = Self::split_function_args(args_str)?;
        
        if args.len() != expected_count {
            return Err(ApiError::InvalidCommand(
                format!("Function requires {} arguments, got {}", expected_count, args.len())
            ));
        }

        let first = Self::unquote_string(&args[0]);
        let second = Self::unquote_string(&args[1]);
        
        Ok((first, second))
    }

    fn split_function_args(args_str: &str) -> ApiResult<Vec<String>> {
        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut in_quotes = false;
        let mut in_brackets = 0;
        let mut chars = args_str.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                    current_arg.push(ch);
                }
                '[' if !in_quotes => {
                    in_brackets += 1;
                    current_arg.push(ch);
                }
                ']' if !in_quotes => {
                    in_brackets -= 1;
                    current_arg.push(ch);
                }
                ',' if !in_quotes && in_brackets == 0 => {
                    args.push(current_arg.trim().to_string());
                    current_arg.clear();
                }
                _ => {
                    current_arg.push(ch);
                }
            }
        }
        
        if in_quotes {
            return Err(ApiError::InvalidCommand("Unclosed quote in arguments".to_string()));
        }
        
        if in_brackets != 0 {
            return Err(ApiError::InvalidCommand("Unclosed bracket in arguments".to_string()));
        }
        
        if !current_arg.trim().is_empty() {
            args.push(current_arg.trim().to_string());
        }
        
        Ok(args)
    }

    fn unquote_string(s: &str) -> String {
        let trimmed = s.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            trimmed[1..trimmed.len()-1].to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn parse_search_args(args_str: &str) -> ApiResult<(String, Vec<String>)> {
        let args_str = args_str.trim();
        if args_str.is_empty() {
            return Err(ApiError::InvalidCommand("Search requires 2 arguments".to_string()));
        }

        // Check for search patterns like "key" or "value", ["query1", "query2"] or "value" and "key", ["query1", "query2"]
        if let Some(or_pos) = Self::find_operator(args_str, " or ") {
            let (left_part, right_part) = Self::split_at_operator(args_str, or_pos, " or ");
            let search_type = Self::parse_search_type_parts(&left_part, &right_part, "or")?;
            let queries = Self::extract_queries_after_operator(&right_part)?;
            return Ok((search_type, queries));
        }
        
        if let Some(and_pos) = Self::find_operator(args_str, " and ") {
            let (left_part, right_part) = Self::split_at_operator(args_str, and_pos, " and ");
            let search_type = Self::parse_search_type_parts(&left_part, &right_part, "and")?;
            let queries = Self::extract_queries_after_operator(&right_part)?;
            return Ok((search_type, queries));
        }

        // Fall back to regular two-argument parsing only for simple cases like search("key", "query") or search("value", ["query1", "query2"])
        let args = Self::split_function_args(args_str)?;
        if args.len() != 2 {
            return Err(ApiError::InvalidCommand("Search requires 2 arguments".to_string()));
        }

        let search_type = Self::unquote_string(&args[0]);
        
        // Only allow simple search types "key" or "value", not compound ones like "key and value"
        if search_type != "key" && search_type != "value" {
            return Err(ApiError::InvalidCommand("Use search(\"key\" or \"value\", \"query\") or search(\"key\" and \"value\", \"query\") syntax".to_string()));
        }
        
        // Parse the second argument - could be a string or array
        let queries = Self::parse_query_argument(&args[1])?;
        
        Ok((search_type, queries))
    }

    fn find_operator(args_str: &str, operator: &str) -> Option<usize> {
        let mut in_quotes = false;
        let mut chars = args_str.char_indices().peekable();
        
        while let Some((i, ch)) = chars.next() {
            if ch == '"' {
                in_quotes = !in_quotes;
            } else if !in_quotes && args_str[i..].starts_with(operator) {
                return Some(i);
            }
        }
        
        None
    }

    fn split_at_operator(args_str: &str, pos: usize, operator: &str) -> (String, String) {
        let left = args_str[..pos].trim().to_string();
        let right = args_str[pos + operator.len()..].trim().to_string();
        (left, right)
    }

    fn parse_search_type_parts(left_part: &str, right_part: &str, operator: &str) -> ApiResult<String> {
        // Extract the first quoted term from left_part
        let left_term = Self::extract_first_quoted_term(left_part)?;
        
        // Extract the first quoted term from right_part (before the comma)
        let comma_pos = Self::find_comma_outside_quotes(right_part)
            .ok_or_else(|| ApiError::InvalidCommand("Missing comma after search type".to_string()))?;
        let right_term_part = &right_part[..comma_pos].trim();
        let right_term = Self::extract_first_quoted_term(right_term_part)?;
        
        // Validate terms are "key" or "value"
        if !Self::is_valid_search_term(&left_term) || !Self::is_valid_search_term(&right_term) {
            return Err(ApiError::InvalidCommand("Search type must be \"key\" or \"value\"".to_string()));
        }
        
        Ok(format!("{} {} {}", left_term, operator, right_term))
    }

    fn extract_queries_after_operator(right_part: &str) -> ApiResult<Vec<String>> {
        let comma_pos = Self::find_comma_outside_quotes(right_part)
            .ok_or_else(|| ApiError::InvalidCommand("Missing comma after search type".to_string()))?;
        let query_part = &right_part[comma_pos + 1..].trim();
        Self::parse_query_argument(query_part)
    }

    fn parse_query_argument(arg: &str) -> ApiResult<Vec<String>> {
        let arg = arg.trim();
        
        // Check if it's an array syntax like ["query1", "query2"]
        if arg.starts_with('[') && arg.ends_with(']') {
            let array_content = &arg[1..arg.len()-1].trim();
            if array_content.is_empty() {
                return Err(ApiError::InvalidCommand("Empty array not allowed".to_string()));
            }
            
            // Split by comma, respecting quotes
            let elements = Self::split_array_elements(array_content)?;
            let mut queries = Vec::new();
            
            for element in elements {
                let query = Self::unquote_string(&element);
                if query.is_empty() {
                    return Err(ApiError::InvalidCommand("Empty query not allowed".to_string()));
                }
                queries.push(query);
            }
            
            if queries.is_empty() {
                return Err(ApiError::InvalidCommand("At least one query required".to_string()));
            }
            
            Ok(queries)
        } else {
            // Single string query
            let query = Self::unquote_string(arg);
            if query.is_empty() {
                return Err(ApiError::InvalidCommand("Empty query not allowed".to_string()));
            }
            Ok(vec![query])
        }
    }

    fn split_array_elements(array_content: &str) -> ApiResult<Vec<String>> {
        let mut elements = Vec::new();
        let mut current_element = String::new();
        let mut in_quotes = false;
        let mut chars = array_content.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                '"' => {
                    in_quotes = !in_quotes;
                    current_element.push(ch);
                }
                ',' if !in_quotes => {
                    let element = current_element.trim().to_string();
                    if !element.is_empty() {
                        elements.push(element);
                    }
                    current_element.clear();
                }
                _ => {
                    current_element.push(ch);
                }
            }
        }
        
        if in_quotes {
            return Err(ApiError::InvalidCommand("Unclosed quote in array".to_string()));
        }
        
        // Add the last element
        let element = current_element.trim().to_string();
        if !element.is_empty() {
            elements.push(element);
        }
        
        Ok(elements)
    }

    fn extract_first_quoted_term(s: &str) -> ApiResult<String> {
        let s = s.trim();
        if s.starts_with('"') {
            if let Some(end_quote) = s[1..].find('"') {
                return Ok(s[1..end_quote + 1].to_string());
            }
        }
        Err(ApiError::InvalidCommand("Expected quoted term".to_string()))
    }

    fn find_comma_outside_quotes(s: &str) -> Option<usize> {
        let mut in_quotes = false;
        for (i, ch) in s.char_indices() {
            if ch == '"' {
                in_quotes = !in_quotes;
            } else if ch == ',' && !in_quotes {
                return Some(i);
            }
        }
        None
    }

    fn is_valid_search_term(term: &str) -> bool {
        term == "key" || term == "value"
    }



    fn validate_key(key: &str) -> ApiResult<()> {
        if key.is_empty() {
            return Err(ApiError::InvalidCommand("Key cannot be empty".to_string()));
        }

        if key.contains(' ') {
            return Err(ApiError::InvalidCommand("Key cannot contain spaces".to_string()));
        }

        for ch in key.chars() {
            if !ch.is_ascii_alphanumeric() && ch != '-' && ch != '_' {
                return Err(ApiError::InvalidCommand(format!(
                    "Key contains invalid character '{}'. Keys can only contain letters, numbers, hyphens, and underscores",
                    ch
                )));
            }
        }

        let chars: Vec<char> = key.chars().collect();
        for (i, &ch) in chars.iter().enumerate() {
            if ch == '-' || ch == '_' {
                if i == 0 || i == chars.len() - 1 {
                    return Err(ApiError::InvalidCommand(format!(
                        "Key cannot start or end with '{}'. Hyphens and underscores must be between letters or numbers",
                        ch
                    )));
                }
                
                let prev = chars[i - 1];
                let next = chars[i + 1];
                if !prev.is_ascii_alphanumeric() || !next.is_ascii_alphanumeric() {
                    return Err(ApiError::InvalidCommand(format!(
                        "Invalid key format. '{}' must be between letters or numbers",
                        ch
                    )));
                }
            }
        }
        for i in 0..chars.len() - 1 {
            if (chars[i] == '-' || chars[i] == '_') && (chars[i + 1] == '-' || chars[i + 1] == '_') {
                return Err(ApiError::InvalidCommand(
                    "Key cannot have consecutive hyphens or underscores".to_string()
                ));
            }
        }

        Ok(())
    }
}

pub struct TcpApiServer {
    listener: TcpListener,
}

impl TcpApiServer {
    pub async fn new(bind_addr: &str) -> ApiResult<Self> {
        let listener = TcpListener::bind(bind_addr).await?;
        Ok(Self { listener })
    }

    pub async fn run(&self) -> ApiResult<()> {
        loop {
            match self.listener.accept().await {
                Ok((stream, client_addr)) => {
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(stream, client_addr).await {
                            error!("Error handling client {}: {}", client_addr, e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting TCP connection: {}", e);
                }
            }
        }
    }

    async fn handle_client(stream: TcpStream, client_addr: SocketAddr) -> ApiResult<()> {
        use tokio::io::{AsyncBufReadExt, BufReader};
        
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                Ok(_) => {
                    let request_str = line.trim();
                    if request_str.is_empty() {
                        continue;
                    }
                    
                    let response = match Command::parse(request_str) {
                        Ok(command) => {
                            info!("{}", request_str);
                            Self::execute_command(command).await
                        }
                        Err(_) => {
                            warn!("Invalid endpoint accessed: {}", request_str);
                            format!("ERROR: Invalid endpoint format")
                        }
                    };
                    
                    let response_with_newline = format!("{}\n", response);
                    if let Err(e) = writer.write_all(response_with_newline.as_bytes()).await {
                        error!("Failed to send response to {}: {}", client_addr, e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Error reading from TCP stream {}: {}", client_addr, e);
                    break;
                }
            }
        }
        
        Ok(())
    }



    async fn execute_command(command: Command) -> String {
        match command {
            Command::Set { key, value } => {
                match threading::execute_cache_set(key, value).await {
                    Ok(()) => "OK".to_string(),
                    Err(e) => format!("ERROR: {}", e)
                }
            }
            Command::Get { key } => {
                match threading::execute_cache_get(key).await {
                    Ok(Some(value)) => value,
                    Ok(None) => "NULL".to_string(),
                    Err(e) => format!("ERROR: {}", e)
                }
            }
            Command::Delete { key } => {
                match threading::execute_cache_delete(key).await {
                    Ok(existed) => {
                        if existed {
                            "1".to_string()
                        } else {
                            "0".to_string()
                        }
                    }
                    Err(e) => format!("ERROR: {}", e)
                }
            }
            Command::Keys => {
                match threading::execute_cache_keys().await {
                    Ok(keys) => {
                        if keys.is_empty() {
                            "(empty)".to_string()
                        } else {
                            keys.join(" ")
                        }
                    }
                    Err(e) => format!("ERROR: {}", e)
                }
            }
            Command::Search { search_type, queries } => {
                match threading::execute_cache_search_multiple(search_type, queries).await {
                    Ok(keys) => {
                        if keys.is_empty() {
                            "(empty)".to_string()
                        } else {
                            keys.join(" ")
                        }
                    }
                    Err(e) => format!("ERROR: {}", e)
                }
            }
        }
    }

    pub fn local_addr(&self) -> ApiResult<SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}

