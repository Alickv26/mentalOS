use crate::error::{MentalOSError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectCommands {
    #[serde(default)]
    pub setup: Option<String>,
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub test: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub framework: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub commands: ProjectCommands,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            name: String::new(),
            created_by: String::new(),
            created_at: Utc::now(),
            language: None,
            framework: None,
            description: None,
            commands: ProjectCommands::default(),
        }
    }
}

/// Handles workspace discovery and project metadata.
///
/// # Examples
/// ```no_run
/// use mental_os::workspace::WorkspaceManager;
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

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_dir
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
            created_by: agent.to_string(),
            created_at: Utc::now(),
            language: None,
            framework: None,
            description: None,
            commands: ProjectCommands::default(),
        };
        let serialized = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, serialized)?;
        Ok(metadata_path)
    }

    pub fn update_metadata(
        &self,
        workspace_path: &Path,
        language: Option<String>,
        framework: Option<String>,
        description: Option<String>,
        commands: ProjectCommands,
    ) -> Result<PathBuf> {
        let metadata_path = workspace_path.join(".mentalOS-project.json");

        let mut metadata = if metadata_path.exists() {
            let data = fs::read_to_string(&metadata_path)?;
            serde_json::from_str::<ProjectMetadata>(&data).unwrap_or_default()
        } else {
            ProjectMetadata {
                name: workspace_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default(),
                created_by: String::new(),
                created_at: Utc::now(),
                ..Default::default()
            }
        };

        if language.is_some() {
            metadata.language = language;
        }
        if framework.is_some() {
            metadata.framework = framework;
        }
        if description.is_some() {
            metadata.description = description;
        }
        metadata.commands = commands;

        let serialized = serde_json::to_string_pretty(&metadata)?;
        fs::write(&metadata_path, serialized)?;
        Ok(metadata_path)
    }

    pub fn load_metadata(&self, workspace_path: &Path) -> Result<Option<ProjectMetadata>> {
        let metadata_path = workspace_path.join(".mentalOS-project.json");
        if !metadata_path.exists() {
            return Ok(None);
        }
        let data = fs::read_to_string(&metadata_path)?;
        let metadata: ProjectMetadata = serde_json::from_str(&data)?;
        Ok(Some(metadata))
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

    #[test]
    fn list_workspaces_returns_empty_for_nonexistent_dir() {
        let manager = WorkspaceManager::new(PathBuf::from("/tmp/nonexistent-mentalos-test"));
        let workspaces = manager.list_workspaces().unwrap();
        assert!(workspaces.is_empty());
    }

    #[test]
    fn create_workspace_rejects_empty_name() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path().to_path_buf());
        assert!(manager.create_workspace("", "agent").is_err());
    }

    #[test]
    fn update_metadata_creates_and_reads_back() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_dir = temp_dir.path().join("test-proj");
        fs::create_dir_all(&workspace_dir).unwrap();

        let manager = WorkspaceManager::new(temp_dir.path().to_path_buf());
        let commands = ProjectCommands {
            setup: Some("npm install".to_string()),
            run: Some("npm start".to_string()),
            test: Some("npm test".to_string()),
        };
        manager
            .update_metadata(
                &workspace_dir,
                Some("javascript".to_string()),
                Some("react".to_string()),
                Some("A test project".to_string()),
                commands.clone(),
            )
            .unwrap();

        let metadata = manager.load_metadata(&workspace_dir).unwrap().unwrap();
        assert_eq!(metadata.language, Some("javascript".to_string()));
        assert_eq!(metadata.framework, Some("react".to_string()));
        assert_eq!(metadata.commands.setup, Some("npm install".to_string()));
        assert_eq!(metadata.commands.run, Some("npm start".to_string()));
    }

    #[test]
    fn load_metadata_returns_none_for_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path().to_path_buf());
        let result = manager
            .load_metadata(&temp_dir.path().join("nonexistent"))
            .unwrap();
        assert!(result.is_none());
    }
}
