# Story 4.7: Auto-Detect AI Assistants

Status: done

## Story

As a user,
I want palingenesis to auto-detect running AI assistants,
So that I don't need to configure them manually.

## Acceptance Criteria

**AC1: Empty Config Triggers Detection**
**Given** config has empty `monitoring.assistants` list
**When** the daemon starts
**Then** it auto-detects supported assistants

**AC2: Detect opencode**
**Given** opencode is running
**When** auto-detection runs
**Then** it finds opencode and adds it to monitored list

**AC3: Log Detection Results**
**Given** auto-detection finds assistants
**When** logging the result
**Then** it logs "Auto-detected assistants: [list]"

**AC4: Explicit Config Overrides**
**Given** explicit assistants are configured
**When** auto-detection is skipped
**Then** only configured assistants are monitored

**AC5: Detection Methods**
**Given** the daemon performs auto-detection
**When** scanning for assistants
**Then** it checks: process list, known directories, file patterns

**AC6: Periodic Re-detection**
**Given** the daemon is running
**When** monitoring.auto_detect is true
**Then** it periodically re-scans for new assistants

## Tasks / Subtasks

- [x] Create detection module (AC: 1, 5)
  - [x] Create `src/monitor/detection.rs`
  - [x] Define `Assistant` struct with name, session_dir, process_pattern
  - [x] Define `DetectionResult` struct

- [x] Implement opencode detection (AC: 2, 5)
  - [x] Check for `~/.opencode` directory
  - [x] Check for opencode process in process list
  - [x] Check for opencode session files pattern

- [x] Implement process-based detection (AC: 5)
  - [x] Scan `/proc` on Linux for process names
  - [x] Use `sysctl` or `ps` on macOS
  - [x] Match against known assistant process names

- [x] Implement directory-based detection (AC: 5)
  - [x] Check for known assistant config directories
  - [x] opencode: `~/.opencode`
  - [x] claude-code: `~/.claude` (future)
  - [x] cursor: `~/.cursor` (future)

- [x] Implement file pattern detection (AC: 5)
  - [x] Look for session file patterns
  - [x] Look for lock files indicating active sessions
  - [x] Look for IPC sockets

- [x] Integrate detection into daemon startup (AC: 1, 4)
  - [x] Check if monitoring.assistants is empty
  - [x] If empty and auto_detect is true, run detection
  - [x] If explicit list, skip detection

- [x] Add logging (AC: 3)
  - [x] Log detected assistants
  - [x] Log detection method used
  - [x] Log if no assistants found

- [x] Implement periodic re-detection (AC: 6)
  - [x] Add detection interval to config
  - [x] Spawn background task for periodic detection
  - [x] Add newly detected assistants to monitoring
  - [x] Handle assistant removal gracefully

- [x] Add unit/integration tests (AC: 1, 2, 3, 4, 5, 6)
  - [x] Test opencode detection with directory
  - [x] Test opencode detection with process
  - [x] Test explicit config skips detection
  - [x] Test logging output
  - [x] Test periodic re-detection

## Dev Notes

### Architecture Requirements

**From architecture.md - Monitor Module:**

```
src/monitor/
    mod.rs                    # Monitor module root
    watcher.rs                # File system watcher
    session.rs                # Session file parsing
    classifier.rs             # Stop reason classification
```

**Implements:** FR25 (Daemon can auto-detect AI assistants if not configured)

### Technical Implementation

**Assistant Registry:**

```rust
// src/monitor/detection.rs
use std::path::PathBuf;

/// Known AI assistant definitions
#[derive(Debug, Clone)]
pub struct AssistantDefinition {
    /// Human-readable name
    pub name: String,
    /// Directory to watch for sessions
    pub session_dir: PathBuf,
    /// Process name pattern for detection
    pub process_pattern: Option<String>,
    /// Session file glob pattern
    pub session_pattern: String,
}

/// Get all known assistant definitions
pub fn known_assistants() -> Vec<AssistantDefinition> {
    vec![
        AssistantDefinition {
            name: "opencode".to_string(),
            session_dir: dirs::home_dir()
                .unwrap_or_default()
                .join(".opencode"),
            process_pattern: Some("opencode".to_string()),
            session_pattern: "**/*.md".to_string(),
        },
        // Future: Add more assistants here
        // AssistantDefinition {
        //     name: "claude-code".to_string(),
        //     session_dir: dirs::home_dir().unwrap().join(".claude"),
        //     process_pattern: Some("claude".to_string()),
        //     session_pattern: "**/*.md".to_string(),
        // },
    ]
}

/// Result of assistant detection
#[derive(Debug)]
pub struct DetectionResult {
    pub assistants: Vec<DetectedAssistant>,
    pub method: DetectionMethod,
}

#[derive(Debug)]
pub struct DetectedAssistant {
    pub definition: AssistantDefinition,
    pub detected_by: DetectionMethod,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub enum DetectionMethod {
    Directory,
    Process,
    SessionFile,
}
```

**Detection Logic:**

```rust
// src/monitor/detection.rs
use std::fs;
use std::process::Command;

/// Detect running AI assistants
pub fn detect_assistants() -> DetectionResult {
    let mut detected = Vec::new();
    
    for assistant in known_assistants() {
        // Try directory detection first (most reliable)
        if assistant.session_dir.exists() {
            tracing::debug!(
                "Detected {} via directory: {}",
                assistant.name,
                assistant.session_dir.display()
            );
            detected.push(DetectedAssistant {
                definition: assistant.clone(),
                detected_by: DetectionMethod::Directory,
                active: has_active_sessions(&assistant.session_dir),
            });
            continue;
        }
        
        // Try process detection
        if let Some(ref pattern) = assistant.process_pattern {
            if is_process_running(pattern) {
                tracing::debug!(
                    "Detected {} via running process",
                    assistant.name
                );
                detected.push(DetectedAssistant {
                    definition: assistant.clone(),
                    detected_by: DetectionMethod::Process,
                    active: true,
                });
                continue;
            }
        }
    }
    
    DetectionResult {
        assistants: detected,
        method: DetectionMethod::Directory, // Primary method
    }
}

fn has_active_sessions(dir: &PathBuf) -> bool {
    // Check for recent session files
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    let age = std::time::SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default();
                    // Consider active if modified in last hour
                    if age.as_secs() < 3600 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(unix)]
fn is_process_running(name: &str) -> bool {
    Command::new("pgrep")
        .arg("-x")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_process_running(_name: &str) -> bool {
    // Windows implementation would go here
    false
}
```

**Integration in Daemon:**

```rust
// src/daemon/core.rs
use crate::monitor::detection::{detect_assistants, DetectionResult};

impl Daemon {
    pub async fn start(&mut self) -> anyhow::Result<()> {
        let config = self.config.read().await;
        
        // Determine which assistants to monitor
        let assistants = if config.monitoring.assistants.is_empty() 
            && config.monitoring.auto_detect 
        {
            tracing::info!("No assistants configured, running auto-detection");
            let result = detect_assistants();
            
            if result.assistants.is_empty() {
                tracing::warn!("No AI assistants detected");
            } else {
                let names: Vec<_> = result.assistants.iter()
                    .map(|a| a.definition.name.as_str())
                    .collect();
                tracing::info!("Auto-detected assistants: {:?}", names);
            }
            
            result.assistants
        } else if !config.monitoring.assistants.is_empty() {
            tracing::info!(
                "Using configured assistants: {:?}",
                config.monitoring.assistants
            );
            // Convert string names to DetectedAssistant
            resolve_configured_assistants(&config.monitoring.assistants)
        } else {
            tracing::warn!("Auto-detect disabled and no assistants configured");
            Vec::new()
        };
        
        // Start watching detected assistants
        for assistant in assistants {
            self.add_watcher(&assistant.definition).await?;
        }
        
        Ok(())
    }
}

fn resolve_configured_assistants(names: &[String]) -> Vec<DetectedAssistant> {
    let known = known_assistants();
    names.iter()
        .filter_map(|name| {
            known.iter()
                .find(|a| a.name == *name)
                .map(|def| DetectedAssistant {
                    definition: def.clone(),
                    detected_by: DetectionMethod::Directory,
                    active: false,
                })
        })
        .collect()
}
```

**Periodic Re-detection (Optional):**

```rust
// src/daemon/core.rs
impl Daemon {
    async fn periodic_detection_task(&self, interval: Duration) {
        let mut ticker = tokio::time::interval(interval);
        
        loop {
            ticker.tick().await;
            
            let config = self.config.read().await;
            if !config.monitoring.auto_detect {
                continue;
            }
            drop(config);
            
            let result = detect_assistants();
            for assistant in result.assistants {
                if !self.is_watching(&assistant.definition.name) {
                    tracing::info!(
                        "Newly detected assistant: {}",
                        assistant.definition.name
                    );
                    if let Err(e) = self.add_watcher(&assistant.definition).await {
                        tracing::error!("Failed to add watcher: {}", e);
                    }
                }
            }
        }
    }
}
```

### Supported Assistants (MVP)

| Assistant | Detection Method | Session Dir |
|-----------|-----------------|-------------|
| opencode | Directory + Process | `~/.opencode` |

### Future Assistants (Post-MVP)

| Assistant | Detection Method | Session Dir |
|-----------|-----------------|-------------|
| claude-code | Directory | `~/.claude` |
| cursor | Directory + Process | `~/.cursor` |
| windsurf | Directory | `~/.windsurf` |

### Dependencies

Uses existing dependencies:
- `dirs` for home directory resolution
- `std::process::Command` for process detection

### Testing Strategy

**Unit Tests:**
- Test known_assistants() returns correct definitions
- Test has_active_sessions() with mock directory
- Test is_process_running() with mock

**Integration Tests:**
- Test detection with real opencode directory (if available)
- Test explicit config skips detection
- Test logging output

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Monitor Module]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 4.7: Auto-Detect AI Assistants]

## File List

**Files to create:**
- `src/monitor/detection.rs`


**Files to modify:**
- `src/monitor/mod.rs`
- `src/daemon/core.rs`
- `src/daemon/state.rs`
- `src/config/schema.rs`
- `src/config/validation.rs`
- `src/cli/commands/config.rs`
- `tests/config_schema_test.rs`
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
- 2026-02-05: Implemented assistant auto-detection, periodic refresh, and tests
