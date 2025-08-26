// src/error.rs
use std::fmt;

#[derive(Debug)]
pub enum GmuxError {
    Subprocess {
        command: String,
        stderr: String,
    },
}

impl fmt::Display for GmuxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GmuxError::Subprocess { command, stderr } => {
                write!(f, "Failed to run '{}': {}", command.trim(), stderr.trim())
            }
        }
    }
}
