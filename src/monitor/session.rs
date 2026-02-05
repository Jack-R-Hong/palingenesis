use std::path::PathBuf;

use serde::Deserialize;

/// Represents a step identifier (integer or string).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(untagged)]
pub enum StepValue {
    Integer(i64),
    String(String),
}

/// Session metadata extracted from frontmatter.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SessionState {
    /// Steps that have been completed.
    #[serde(default, rename = "stepsCompleted", alias = "steps_completed")]
    pub steps_completed: Vec<StepValue>,

    /// The last step executed (if available).
    #[serde(default, rename = "lastStep", alias = "last_step")]
    pub last_step: Option<i64>,

    /// Workflow status (e.g., "complete", "in-progress").
    #[serde(default)]
    pub status: Option<String>,

    /// Type of workflow (e.g., "architecture", "epics-and-stories").
    #[serde(default, rename = "workflowType", alias = "workflow_type")]
    pub workflow_type: Option<String>,

    /// Project name.
    #[serde(default, rename = "project_name", alias = "projectName")]
    pub project_name: Option<String>,

    /// Input documents used.
    #[serde(default, rename = "inputDocuments", alias = "input_documents")]
    pub input_documents: Vec<String>,
}

/// A parsed session file with path and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Path to the session file.
    pub path: PathBuf,

    /// Parsed frontmatter state.
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
