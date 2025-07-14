// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

mod call;
mod parser;

use std::io::{self, Write, BufRead};
use parser::{parse_command_line, ParsedCommand};
use call::execute_command;

fn main() {
    let stdin = io::stdin();
    loop {
        print!("sodium-cli> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match stdin.lock().read_line(&mut input) {
            Ok(_) => {
                let input = input.trim();
                if input.is_empty() {
                    continue;
                }
                
                match parse_command_line(input) {
                    Some(ParsedCommand::Set { address, key, value }) => {
                        execute_command(&address, &format!("SET {} \"{}\"", key, value));
                    }
                    Some(ParsedCommand::Get { address, key }) => {
                        execute_command(&address, &format!("GET {}", key));
                    }
                    Some(ParsedCommand::Del { address, key }) => {
                        execute_command(&address, &format!("DEL {}", key));
                    }
                    Some(ParsedCommand::Keys { address }) => {
                        execute_command(&address, "KEYS");
                    }
                    Some(ParsedCommand::Error(msg)) => {
                        println!("{}", msg);
                    }
                    None => {}
                }
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
} 