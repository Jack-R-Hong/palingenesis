use std::path::{Path, PathBuf};

use tracing::debug;

#[derive(Debug, Clone)]
pub struct AssistantDefinition {
    pub name: String,
    pub session_dir: PathBuf,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DetectedAssistant {
    pub name: String,
    pub session_dir: PathBuf,
    pub detected_by: DetectionMethod,
    pub active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionMethod {
    Directory,
    Process,
    SessionFile,
}

impl DetectionMethod {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Directory => "directory",
            Self::Process => "process",
            Self::SessionFile => "session-file",
        }
    }
}

#[derive(Debug, Default)]
pub struct DetectionResult {
    pub assistants: Vec<DetectedAssistant>,
}

pub fn known_assistants() -> Vec<AssistantDefinition> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    vec![AssistantDefinition {
        name: "opencode".to_string(),
        session_dir: home.join(".opencode"),
        process_name: Some("opencode".to_string()),
    }]
}

pub fn detect_assistants() -> DetectionResult {
    let mut assistants = Vec::new();

    for assistant in known_assistants() {
        if let Some(detected) = detect_assistant(&assistant) {
            assistants.push(detected);
        }
    }

    DetectionResult { assistants }
}

fn detect_assistant(definition: &AssistantDefinition) -> Option<DetectedAssistant> {
    let has_sessions = has_session_files(&definition.session_dir);
    let dir_exists = definition.session_dir.exists();
    let process_running = definition
        .process_name
        .as_deref()
        .map(is_process_running)
        .unwrap_or(false);

    if !(has_sessions || dir_exists || process_running) {
        return None;
    }

    let detected_by = if has_sessions {
        DetectionMethod::SessionFile
    } else if process_running {
        DetectionMethod::Process
    } else {
        DetectionMethod::Directory
    };

    if has_sessions {
        debug!(
            assistant = %definition.name,
            path = %definition.session_dir.display(),
            "Detected assistant via session files"
        );
    } else if dir_exists {
        debug!(
            assistant = %definition.name,
            path = %definition.session_dir.display(),
            "Detected assistant via directory"
        );
    } else if process_running {
        debug!(assistant = %definition.name, "Detected assistant via process");
    }

    Some(DetectedAssistant {
        name: definition.name.clone(),
        session_dir: definition.session_dir.clone(),
        detected_by,
        active: has_sessions || process_running,
    })
}

fn has_session_files(dir: &Path) -> bool {
    let mut stack = vec![dir.to_path_buf()];

    while let Some(path) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(entries) => entries,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }

            if is_session_artifact(&path) {
                return true;
            }
        }
    }

    false
}

fn is_session_artifact(path: &Path) -> bool {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("md") => true,
        Some("lock") => true,
        Some("sock") => true,
        _ => false,
    }
}

#[cfg(unix)]
fn is_process_running(name: &str) -> bool {
    std::process::Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_process_running(_name: &str) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_detect_assistant_from_directory() {
        let temp = tempdir().unwrap();
        let definition = AssistantDefinition {
            name: "opencode".to_string(),
            session_dir: temp.path().to_path_buf(),
            process_name: None,
        };

        let detected = detect_assistant(&definition).expect("detect assistant");
        assert_eq!(detected.name, "opencode");
        assert_eq!(detected.detected_by, DetectionMethod::Directory);
    }

    #[test]
    fn test_detect_assistant_from_session_file() {
        let temp = tempdir().unwrap();
        let session_file = temp.path().join("session.md");
        std::fs::write(&session_file, "content").unwrap();
        let definition = AssistantDefinition {
            name: "opencode".to_string(),
            session_dir: temp.path().to_path_buf(),
            process_name: None,
        };

        let detected = detect_assistant(&definition).expect("detect assistant");
        assert_eq!(detected.detected_by, DetectionMethod::SessionFile);
        assert!(detected.active);
    }
}
