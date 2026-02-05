use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use crate::monitor::events::{MonitorEvent, WatchEvent};
use crate::monitor::session::{Session, SessionState};

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No frontmatter found (missing --- delimiters)")]
    NoFrontmatter,

    #[error("Invalid YAML frontmatter: {0}")]
    InvalidFrontmatter(#[from] serde_yaml::Error),

    #[error("Session file not found: {path}")]
    FileNotFound { path: PathBuf },
}

/// Extract YAML frontmatter from a markdown file.
///
/// Efficiently reads only the frontmatter section, stopping
/// after the closing `---` delimiter.
pub fn extract_frontmatter(path: &Path) -> Result<String, ParseError> {
    let file = File::open(path).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            ParseError::FileNotFound {
                path: path.to_path_buf(),
            }
        } else {
            ParseError::Io(err)
        }
    })?;

    let reader = BufReader::new(file);
    let mut lines = reader.lines();

    let first_line = lines.next().ok_or(ParseError::NoFrontmatter)??;
    if first_line.trim() != "---" {
        return Err(ParseError::NoFrontmatter);
    }

    let mut frontmatter = String::new();
    for line in lines {
        let line = line?;
        if line.trim() == "---" {
            return Ok(frontmatter);
        }
        frontmatter.push_str(&line);
        frontmatter.push('\n');
    }

    Err(ParseError::NoFrontmatter)
}

/// Parse a session file and extract its state.
pub fn parse_session(path: &Path) -> Result<Session, ParseError> {
    let frontmatter = extract_frontmatter(path)?;
    let state: SessionState = serde_yaml::from_str(&frontmatter)?;

    Ok(Session {
        path: path.to_path_buf(),
        state,
    })
}

/// Maintains parsed session state for watch events.
#[derive(Debug, Default)]
pub struct SessionParser {
    sessions: HashMap<PathBuf, Session>,
}

impl SessionParser {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }

    pub fn handle_event(&mut self, event: WatchEvent) -> Option<MonitorEvent> {
        match event {
            WatchEvent::FileModified(path) | WatchEvent::FileCreated(path) => {
                match parse_session(&path) {
                    Ok(session) => {
                        let previous = self.sessions.insert(path, session.clone());
                        Some(MonitorEvent::SessionChanged { session, previous })
                    }
                    Err(err) => Some(MonitorEvent::Error {
                        source: "session_parser".to_string(),
                        message: err.to_string(),
                        recoverable: true,
                    }),
                }
            }
            WatchEvent::FileDeleted(path) => {
                self.sessions.remove(&path);
                None
            }
            WatchEvent::DirectoryCreated(_) => None,
            WatchEvent::Error(message) => Some(MonitorEvent::Error {
                source: "watcher".to_string(),
                message,
                recoverable: true,
            }),
        }
    }
}
