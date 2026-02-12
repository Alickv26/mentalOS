use crate::error::{MentalOSError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub agent: String,
}

/// Handles workspace discovery and project metadata.
///
/// # Examples
/// ```no_run
/// use mentalOS::workspace::WorkspaceManager;
/// let manager = WorkspaceManager::new(std::path::PathBuf::from("/tmp/workspaces"));
/// manager.create_workspace("demo", "openclaw").unwrap();
/// ```
pub struct WorkspaceManager {
    workspace_dir: PathBuf,
}

impl WorkspaceManager {
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self { workspace_dir }
    }

    pub fn list_workspaces(&self) -> Result<Vec<PathBuf>> {
        if !self.workspace_dir.exists() {
            return Ok(Vec::new());
        }
        let mut workspaces = Vec::new();
        for entry in fs::read_dir(&self.workspace_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                workspaces.push(entry.path());
            }
        }
        workspaces.sort();
        Ok(workspaces)
    }

    pub fn create_workspace(&self, name: &str, agent: &str) -> Result<PathBuf> {
        if name.trim().is_empty() {
            return Err(MentalOSError::Other("Workspace name is empty".into()));
        }
        let workspace_path = self.workspace_dir.join(name);
        fs::create_dir_all(&workspace_path)?;
        self.ensure_metadata(&workspace_path, name, agent)?;
        Ok(workspace_path)
    }

    pub fn ensure_metadata(
        &self,
        workspace_path: &Path,
        name: &str,
        agent: &str,
    ) -> Result<PathBuf> {
        let metadata_path = workspace_path.join(".mentalOS-project.json");
        if metadata_path.exists() {
            return Ok(metadata_path);
        }
        let metadata = ProjectMetadata {
            name: name.to_string(),
            created_at: Utc::now(),
            agent: agent.to_string(),
        };
        let serialized = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, serialized)?;
        Ok(metadata_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn creates_workspace_and_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path().to_path_buf());

        let path = manager.create_workspace("demo", "openclaw").unwrap();
        assert!(path.exists());
        assert!(path.join(".mentalOS-project.json").exists());
    }
}
