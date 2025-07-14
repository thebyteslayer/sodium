// Copyright (c) 2025, TheByteSlayer, Sodium
// A scalable and optimized Key Value Caching System, written in Rust.

use std::io::{Write, BufRead, BufReader};
use std::net::TcpStream;

pub fn execute_command(address: &str, command: &str) {
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