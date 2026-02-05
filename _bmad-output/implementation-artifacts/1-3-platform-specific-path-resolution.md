# Story 1.3: Platform-Specific Path Resolution

Status: ready-for-dev

## Story

As a user,
I want palingenesis to use platform-appropriate paths for config and state,
So that it integrates cleanly with my system conventions.

## Acceptance Criteria

**AC1: Linux Config Path**
**Given** a Linux system
**When** palingenesis resolves config path
**Then** it uses `~/.config/palingenesis/config.toml`

**AC2: macOS Config Path**
**Given** a macOS system
**When** palingenesis resolves config path
**Then** it uses `~/Library/Application Support/palingenesis/config.toml`

**AC3: Linux State Path**
**Given** a Linux system
**When** palingenesis resolves state path
**Then** it uses `~/.local/state/palingenesis/`

**AC4: macOS State Path**
**Given** a macOS system
**When** palingenesis resolves state path
**Then** it uses `~/Library/Application Support/palingenesis/`

**AC5: Environment Variable Override**
**Given** the environment variable `PALINGENESIS_CONFIG` is set
**When** palingenesis resolves config path
**Then** it uses the path from the environment variable

## Tasks / Subtasks

- [ ] Create `src/config/paths.rs` with platform-specific path resolution (AC: 1, 2, 3, 4, 5)
  - [ ] Define `Paths` struct with `config_dir()`, `config_file()`, `state_dir()`, `runtime_dir()` methods
  - [ ] Implement Linux path resolution using XDG Base Directory Specification
  - [ ] Implement macOS path resolution using Apple conventions
  - [ ] Add `PALINGENESIS_CONFIG` environment variable override support
  - [ ] Add `PALINGENESIS_STATE` environment variable override support (optional)
- [ ] Add `dirs` crate dependency for reliable home directory detection
  - [ ] Update Cargo.toml with `dirs = "6.0"` dependency
- [ ] Implement runtime directory resolution (AC: N/A, prep for Story 1.5, 1.6)
  - [ ] Linux: `/run/user/{uid}/palingenesis/`
  - [ ] macOS: `/tmp/palingenesis-{uid}/`
- [ ] Add directory creation helper functions
  - [ ] `ensure_config_dir()` - creates config directory if not exists
  - [ ] `ensure_state_dir()` - creates state directory if not exists
  - [ ] `ensure_runtime_dir()` - creates runtime directory if not exists
- [ ] Update `src/config/mod.rs` to re-export paths module
- [ ] Add unit tests for path resolution (AC: 1, 2, 3, 4, 5)
  - [ ] Test Linux paths (mock or cfg-gated)
  - [ ] Test macOS paths (mock or cfg-gated)
  - [ ] Test environment variable override
  - [ ] Test directory creation functions
- [ ] Add integration test for environment variable override

## Dev Notes

### Architecture Requirements

**From architecture.md - Platform-Specific Paths:**

| Resource | Linux | macOS |
|----------|-------|-------|
| Config | `~/.config/palingenesis/` | `~/Library/Application Support/palingenesis/` |
| State | `~/.local/state/palingenesis/` | `~/Library/Application Support/palingenesis/` |
| Runtime | `/run/user/{uid}/` | `/tmp/palingenesis-{uid}/` |

**From architecture.md - Project Structure:**
```
src/config/
├── mod.rs           # Config module root
├── schema.rs        # Config struct definitions
├── loader.rs        # File/env/CLI config loading
├── paths.rs         # Platform-specific paths  <-- THIS STORY
└── validation.rs    # Config validation
```

**Implements:** ARCH16 (Linux config), ARCH17 (macOS config), ARCH18 (PID file path prep)

### Technical Implementation

**Paths Struct Pattern:**
```rust
// src/config/paths.rs
use std::path::PathBuf;

/// Platform-specific path resolution for palingenesis
pub struct Paths;

impl Paths {
    /// Returns the configuration directory path
    /// - Linux: ~/.config/palingenesis/
    /// - macOS: ~/Library/Application Support/palingenesis/
    /// - Override: PALINGENESIS_CONFIG env var (directory)
    pub fn config_dir() -> PathBuf {
        if let Ok(path) = std::env::var("PALINGENESIS_CONFIG") {
            return PathBuf::from(path).parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(path));
        }
        
        #[cfg(target_os = "linux")]
        {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("~/.config"))
                .join("palingenesis")
        }
        
        #[cfg(target_os = "macos")]
        {
            dirs::config_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join("Library/Application Support"))
                        .unwrap_or_else(|| PathBuf::from("~/Library/Application Support"))
                })
                .join("palingenesis")
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            // Fallback for unsupported platforms (Windows deferred to post-MVP)
            PathBuf::from(".palingenesis")
        }
    }

    /// Returns the full config file path
    pub fn config_file() -> PathBuf {
        if let Ok(path) = std::env::var("PALINGENESIS_CONFIG") {
            return PathBuf::from(path);
        }
        Self::config_dir().join("config.toml")
    }

    /// Returns the state directory path
    /// - Linux: ~/.local/state/palingenesis/
    /// - macOS: ~/Library/Application Support/palingenesis/
    pub fn state_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            dirs::state_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir()
                        .map(|h| h.join(".local/state"))
                        .unwrap_or_else(|| PathBuf::from("~/.local/state"))
                })
                .join("palingenesis")
        }
        
        #[cfg(target_os = "macos")]
        {
            // macOS doesn't have separate state dir, use config location
            Self::config_dir()
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from(".palingenesis")
        }
    }

    /// Returns the runtime directory path (for PID file, Unix socket)
    /// - Linux: /run/user/{uid}/palingenesis/
    /// - macOS: /tmp/palingenesis-{uid}/
    pub fn runtime_dir() -> PathBuf {
        #[cfg(target_os = "linux")]
        {
            dirs::runtime_dir()
                .map(|d| d.join("palingenesis"))
                .unwrap_or_else(|| PathBuf::from("/tmp/palingenesis"))
        }
        
        #[cfg(target_os = "macos")]
        {
            let uid = unsafe { libc::getuid() };
            PathBuf::from(format!("/tmp/palingenesis-{}", uid))
        }
        
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            PathBuf::from(".palingenesis/run")
        }
    }
}
```

**Directory Creation Helpers:**
```rust
use std::fs;
use std::io;

impl Paths {
    /// Ensures the config directory exists, creating it if necessary
    pub fn ensure_config_dir() -> io::Result<PathBuf> {
        let dir = Self::config_dir();
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Ensures the state directory exists, creating it if necessary
    pub fn ensure_state_dir() -> io::Result<PathBuf> {
        let dir = Self::state_dir();
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Ensures the runtime directory exists with secure permissions
    pub fn ensure_runtime_dir() -> io::Result<PathBuf> {
        let dir = Self::runtime_dir();
        fs::create_dir_all(&dir)?;
        // Set permissions to 700 (owner only) for security
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
        }
        Ok(dir)
    }
}
```

### Dependencies to Add

```toml
# Cargo.toml additions
[dependencies]
dirs = "6.0"  # Cross-platform directory resolution

[target.'cfg(target_os = "macos")'.dependencies]
libc = "0.2"  # For getuid() on macOS (already available via nix on unix)
```

Note: The `libc` crate is likely already available transitively through `nix`, but verify.

### Error Handling Pattern

Use `thiserror` for domain errors per architecture guidelines:
```rust
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    #[error("Home directory not found")]
    HomeNotFound,
    
    #[error("Failed to create directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    
    #[error("Invalid config path from PALINGENESIS_CONFIG: {0}")]
    InvalidEnvPath(String),
}
```

### Testing Strategy

**Unit Tests (cfg-gated):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_env_override() {
        env::set_var("PALINGENESIS_CONFIG", "/custom/path/config.toml");
        assert_eq!(Paths::config_file(), PathBuf::from("/custom/path/config.toml"));
        env::remove_var("PALINGENESIS_CONFIG");
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_linux_config_dir() {
        env::remove_var("PALINGENESIS_CONFIG");
        let path = Paths::config_dir();
        assert!(path.to_string_lossy().contains(".config/palingenesis"));
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn test_macos_config_dir() {
        env::remove_var("PALINGENESIS_CONFIG");
        let path = Paths::config_dir();
        assert!(path.to_string_lossy().contains("Application Support/palingenesis"));
    }
}
```

**Integration Test:**
```rust
// tests/paths_test.rs
use std::env;
use tempfile::tempdir;

#[test]
fn test_env_override_integration() {
    let temp = tempdir().unwrap();
    let config_path = temp.path().join("config.toml");
    
    env::set_var("PALINGENESIS_CONFIG", config_path.to_str().unwrap());
    
    // Import the actual function from the crate
    let resolved = palingenesis::config::Paths::config_file();
    assert_eq!(resolved, config_path);
    
    env::remove_var("PALINGENESIS_CONFIG");
}
```

### Previous Story Learnings

From Story 1-2:
1. **Module pattern**: Re-export public API through mod.rs
2. **Rust 2024 edition**: Use edition 2024 features
3. **Test isolation**: Unit tests inline, integration tests in `tests/`
4. **Existing structure**: `src/config/mod.rs` exists but is empty (just module doc comment)

### Project Structure Notes

- This story creates `src/config/paths.rs` which aligns with architecture spec
- The `config/mod.rs` needs to be updated to re-export `Paths` and `PathError`
- Runtime directory will be used by Story 1.5 (PID file) and Story 1.6 (Unix socket)

### References

- [Source: _bmad-output/planning-artifacts/architecture.md#Platform-Specific Paths]
- [Source: _bmad-output/planning-artifacts/architecture.md#Infrastructure & Deployment]
- [Source: _bmad-output/planning-artifacts/architecture.md#Project Structure & Boundaries]
- [Source: _bmad-output/planning-artifacts/epics.md#Story 1.3: Platform-Specific Path Resolution]

## Dev Agent Record

### Agent Model Used

{{agent_model_name_version}}

### Debug Log References

### Completion Notes List

### File List

**Files to create:**
- `src/config/paths.rs`
- `tests/paths_test.rs`

**Files to modify:**
- `Cargo.toml` - Add dirs dependency
- `src/config/mod.rs` - Re-export paths module
