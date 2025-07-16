// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use std::io::{self, Write, BufRead, BufReader};
use std::net::TcpStream;

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
                
                // Parse address and command
                let parts: Vec<&str> = input.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    println!("Error: Usage: <address> <command>");
                    continue;
                }
                
                let address = parts[0];
                let command = parts[1];
                
                execute_command(address, command);
            }
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                break;
            }
        }
    }
}

fn execute_command(address: &str, command: &str) {
    match TcpStream::connect(address) {
        Ok(mut stream) => {
            if let Err(e) = stream.write_all(command.as_bytes()) {
                println!("Failed to send command: {}", e);
                return;
            }

            if let Err(e) = stream.write_all(b"\n") {
                println!("Failed to send newline: {}", e);
                return;
            }

            let mut reader = BufReader::new(&mut stream);
            let mut response = String::new();
            match reader.read_line(&mut response) {
                Ok(_) => {
                    let trimmed = response.trim();
                    if !trimmed.is_empty() {
                        println!("{}", trimmed);
                    }
                }
                Err(e) => {
                    println!("Failed to read response: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Failed to connect to {}: {}", address, e);
        }
    }
} 