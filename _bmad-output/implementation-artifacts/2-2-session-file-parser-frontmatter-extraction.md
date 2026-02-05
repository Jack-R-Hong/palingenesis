# Story 2.2: Session File Parser (Frontmatter Extraction)

Status: ready-for-dev

## Story

As a monitor,
I want to parse session file frontmatter,
So that I can extract workflow state and metadata.

## Acceptance Criteria

**AC1: Valid Frontmatter Extraction**
**Given** a session file with YAML frontmatter
**When** the parser reads the file
**Then** it extracts frontmatter between `---` delimiters
**And** parses it as YAML
**And** returns a Session struct

**AC2: Missing Frontmatter Handling**
**Given** a session file without frontmatter
**When** the parser reads the file
**Then** it returns an error `NoFrontmatter`

**AC3: Invalid YAML Handling**
**Given** a session file with invalid YAML frontmatter
**When** the parser reads the file
**Then** it returns an error `InvalidFrontmatter` with details

**AC4: stepsCompleted Array Extraction**
**Given** frontmatter contains `stepsCompleted` array
**When** parsed successfully
**Then** Session struct contains the array values

**AC5: lastStep Field Extraction**
**Given** frontmatter contains `lastStep` field
**When** parsed successfully
**Then** Session struct contains the step number

**AC6: Efficient Parsing (Body Ignored)**
**Given** a large session file with extensive body content
**When** the parser reads the file
**Then** it only parses the frontmatter section
**And** ignores the body content for efficiency

**AC7: Watcher Integration**
**Given** the file watcher emits a `FileModified` event
**When** the monitor receives the event
**Then** it uses the parser to extract session state
**And** emits a `SessionChanged` MonitorEvent with parsed data

## Tasks / Subtasks

- [ ] Create frontmatter extraction module (AC: 1, 2, 3, 6)
  - [ ] Create `src/monitor/frontmatter.rs` with extraction logic
  - [ ] Implement `extract_frontmatter()` function
  - [ ] Handle missing frontmatter delimiter case
  - [ ] Implement efficient line-by-line parsing (stop after second `---`)

- [ ] Define Session struct and error types (AC: 1, 2, 3, 4, 5)
  - [ ] Create `src/monitor/session.rs` with Session struct
  - [ ] Define `SessionState` struct with workflow metadata
  - [ ] Define `ParseError` enum with thiserror
  - [ ] Implement serde deserialization for Session

- [ ] Implement YAML parsing with serde_yaml (AC: 1, 4, 5)
  - [ ] Add `serde_yaml` dependency to Cargo.toml
  - [ ] Parse frontmatter YAML into Session struct
  - [ ] Handle `stepsCompleted` as `Vec<StepValue>` (supports int/string)
  - [ ] Handle `lastStep` as optional field
  - [ ] Handle `status`, `workflowType`, `project_name` fields

- [ ] Implement MonitorEvent integration (AC: 7)
  - [ ] Extend `MonitorEvent` enum with `SessionChanged` variant
  - [ ] Add `Session` payload to `SessionChanged` event
  - [ ] Create `SessionParser` struct for stateful parsing
  - [ ] Integrate with watcher event stream

- [ ] Update monitor module exports (AC: 1-7)
  - [ ] Export `Session`, `SessionState` from `src/monitor/mod.rs`
  - [ ] Export `extract_frontmatter` function
  - [ ] Export `ParseError` type

- [ ] Add unit tests (AC: 1, 2, 3, 4, 5, 6)
  - [ ] Test valid frontmatter extraction
  - [ ] Test missing frontmatter returns `NoFrontmatter`
  - [ ] Test invalid YAML returns `InvalidFrontmatter`
  - [ ] Test `stepsCompleted` array parsing
  - [ ] Test `lastStep` field parsing
  - [ ] Test large file efficiency (body ignored)

- [ ] Add integration tests (AC: 7)
  - [ ] Test watcher + parser pipeline
  - [ ] Test end-to-end file modification -> MonitorEvent emission
  - [ ] Test fixture files from real opencode sessions

## Dev Notes

### Architecture Requirements

**From architecture.md - Project Structure:**

```
src/monitor/
    mod.rs                    # Monitor module root
    watcher.rs                # File system watcher (notify) - Story 2.1
    session.rs                # Session file parsing (THIS STORY)
    frontmatter.rs            # YAML frontmatter extraction (THIS STORY)
    classifier.rs             # Stop reason classification (Story 2.4-2.6)
    error.rs                  # MonitorError type
```

**From architecture.md - Data Architecture:**

> | Decision | Choice | Rationale |
> |----------|--------|-----------|
> | **Session Parsing** | YAML frontmatter only | Only need `stepsCompleted`, `lastStep`, workflow metadata. Body parsing is unnecessary overhead. |

**From architecture.md - Internal Communication:**

> | From | To | Mechanism |
> |------|-----|-----------|
> | Monitor -> Daemon | `tokio::sync::mpsc` | `MonitorEvent` channel |

**Implements:** FR6 (read session state from frontmatter), FR7 (parse stepsCompleted array), ARCH12 (YAML frontmatter only)

### Technical Implementation

**Session Struct:**

```rust
// src/monitor/session.rs
use std::path::PathBuf;
use serde::Deserialize;

/// Represents a step identifier (can be integer or string like "step-01-validate").
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum StepValue {
    Integer(i64),
    String(String),
}

/// Session metadata extracted from frontmatter.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    /// Steps that have been completed
    #[serde(default)]
    pub steps_completed: Vec<StepValue>,
    
    /// The last step executed (if available)
    #[serde(default)]
    pub last_step: Option<i64>,
    
    /// Workflow status (e.g., "complete", "in-progress")
    #[serde(default)]
    pub status: Option<String>,
    
    /// Type of workflow (e.g., "architecture", "epics-and-stories")
    #[serde(default)]
    pub workflow_type: Option<String>,
    
    /// Project name
    #[serde(default)]
    pub project_name: Option<String>,
    
    /// Input documents used
    #[serde(default)]
    pub input_documents: Vec<String>,
}

/// A parsed session file with path and state.
#[derive(Debug, Clone)]
pub struct Session {
    /// Path to the session file
    pub path: PathBuf,
    
    /// Parsed frontmatter state
    pub state: SessionState,
}

impl Session {
    /// Check if the session is complete.
    pub fn is_complete(&self) -> bool {
        self.state.status.as_deref() == Some("complete")
    }
    
    /// Get the number of completed steps.
    pub fn steps_completed_count(&self) -> usize {
        self.state.steps_completed.len()
    }
}
```

**Frontmatter Extraction:**

```rust
// src/monitor/frontmatter.rs
use std::io::{BufRead, BufReader};
use std::fs::File;
use std::path::Path;

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
    FileNotFound { path: std::path::PathBuf },
}

/// Extract YAML frontmatter from a markdown file.
/// 
/// Efficiently reads only the frontmatter section, stopping
/// after the closing `---` delimiter.
pub fn extract_frontmatter(path: &Path) -> Result<String, ParseError> {
    let file = File::open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ParseError::FileNotFound { path: path.to_path_buf() }
        } else {
            ParseError::Io(e)
        }
    })?;
    
    let reader = BufReader::new(file);
    let mut lines = reader.lines();
    
    // Check for opening delimiter
    let first_line = lines.next()
        .ok_or(ParseError::NoFrontmatter)?
        .map_err(ParseError::Io)?;
    
    if first_line.trim() != "---" {
        return Err(ParseError::NoFrontmatter);
    }
    
    // Collect frontmatter lines until closing delimiter
    let mut frontmatter = String::new();
    for line in lines {
        let line = line.map_err(ParseError::Io)?;
        if line.trim() == "---" {
            return Ok(frontmatter);
        }
        frontmatter.push_str(&line);
        frontmatter.push('\n');
    }
    
    // Reached EOF without closing delimiter
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
```

**MonitorEvent Extension:**

```rust
// src/monitor/events.rs (extend from Story 2.1)
use std::path::PathBuf;
use crate::monitor::session::Session;

/// Events emitted by the file system watcher and parser.
#[derive(Debug, Clone)]
pub enum MonitorEvent {
    /// File was created in the session directory
    FileCreated(PathBuf),
    /// File was modified in the session directory
    FileModified(PathBuf),
    /// File was deleted from the session directory
    FileDeleted(PathBuf),
    /// Directory was created (for session directory appearance)
    DirectoryCreated(PathBuf),
    /// Session state changed (parsed from frontmatter)
    SessionChanged {
        session: Session,
        previous: Option<Session>,
    },
    /// Watcher encountered an error
    Error(String),
}
```

### Dependencies

**New dependency to add:**

```toml
# Cargo.toml
[dependencies]
serde_yaml = "0.9"
```

**Uses existing dependencies:**
- `serde = { version = "1.0.228", features = ["derive"] }` - deserialization
- `thiserror = "2.0.17"` - error types
- `tracing = "0.1.44"` - structured logging
- `tokio = { version = "1.49", features = ["full"] }` - async runtime

### Error Handling Pattern

Uses `thiserror` following project conventions from architecture.md:
- `ParseError::Io` - File system operations failed
- `ParseError::NoFrontmatter` - File lacks `---` delimiters
- `ParseError::InvalidFrontmatter` - YAML parsing failed
- `ParseError::FileNotFound` - Session file doesn't exist

### Previous Story Learnings

From Story 2.1 (File System Watcher Setup):
1. **WatchEvent types**: Extend with `SessionChanged` variant
2. **Channel integration**: Parser results flow through same mpsc channel
3. **Error resilience**: Continue monitoring on parse failures
4. **Debouncing**: Parser receives debounced events (no rapid re-parsing)

From Story 1.4 (State Persistence Layer):
1. **File handling**: Use BufReader for efficient reading
2. **Error recovery**: Log parse errors but don't crash

From architecture.md patterns:
1. **thiserror for domain errors**: ParseError enum
2. **Structured logging**: Use tracing with fields
3. **Module re-exports**: Export public types via mod.rs

### Session File Format

Example session file (from opencode/BMAD workflows):

```markdown
---
stepsCompleted: [1, 2, 3]
inputDocuments:
  - '_bmad-output/planning-artifacts/prd.md'
workflowType: 'architecture'
project_name: 'palingenesis'
user_name: 'Jack'
date: '2026-02-05'
lastStep: 3
status: 'in-progress'
---

# Architecture Document

Content here is ignored by the parser...
```

Alternative format with string steps:

```markdown
---
stepsCompleted: ['step-01-validate-prerequisites', 'step-02-design-epics']
workflowType: 'epics-and-stories'
status: 'complete'
---
```

### Performance Considerations

- **Efficiency**: Only read frontmatter section (stop at second `---`)
- **Memory**: Stream file line-by-line, don't load entire file
- **CPU**: Single parse per file modification (debounced events from watcher)
- **NFR1: <5s detection**: Parse is ~1-10ms, well within budget

### Testing Strategy

**Unit Tests:**
- Test frontmatter extraction from fixture files
- Test missing frontmatter detection
- Test invalid YAML error handling
- Test various step formats (int, string, mixed)

**Integration Tests:**
- Create temp session file, modify, verify event emission
- Test with real opencode session file fixtures
- Test watcher -> parser -> MonitorEvent pipeline

**Fixtures:**
- `tests/fixtures/session_valid.md` - complete valid session
- `tests/fixtures/session_no_frontmatter.md` - missing delimiters
- `tests/fixtures/session_invalid_yaml.md` - malformed YAML
- `tests/fixtures/session_string_steps.md` - string step identifiers

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Data Architecture]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 2.2: Session File Parser]
- [Source: _bmad-output/implementation-artifacts/2-1-file-system-watcher-setup.md]

## File List

**Files to create:**
- `src/monitor/frontmatter.rs`
- `src/monitor/session.rs`
- `tests/fixtures/session_valid.md`
- `tests/fixtures/session_no_frontmatter.md`
- `tests/fixtures/session_invalid_yaml.md`
- `tests/fixtures/session_string_steps.md`
- `tests/session_parser_test.rs`

**Files to modify:**
- `Cargo.toml` (add serde_yaml)
- `Cargo.lock`
- `src/monitor/mod.rs` (export new modules)
- `src/monitor/events.rs` (add SessionChanged variant)
- `_bmad-output/implementation-artifacts/sprint-status.yaml`

## Change Log

- 2026-02-05: Story created and marked ready-for-dev
