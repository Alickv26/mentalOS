use crate::error::Result;
use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub workspace: String,
    pub category: String,
    pub created_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

/// Manages per-workspace, per-category conversation memory.
///
/// # Examples
/// ```no_run
/// use mentalOS::memory::{MemoryManager, Role};
/// let manager = MemoryManager::new(std::path::PathBuf::from("/tmp/workspaces"));
/// manager.append_message("demo", "general", Role::User, "hello").unwrap();
/// ```
pub struct MemoryManager {
    workspace_dir: PathBuf,
}

impl MemoryManager {
    pub fn new(workspace_dir: PathBuf) -> Self {
        Self { workspace_dir }
    }

    pub fn append_message(
        &self,
        workspace: &str,
        category: &str,
        role: Role,
        content: impl Into<String>,
    ) -> Result<PathBuf> {
        let content = content.into();
        let category = normalize_category(category);
        let dir = self.category_dir(workspace, &category);
        fs::create_dir_all(&dir)?;

        let file_path = self.find_or_create_today_file(&dir, workspace, &category)?;
        let mut conversation = load_conversation(&file_path)?.unwrap_or_else(|| Conversation {
            id: file_id_from_path(&file_path),
            workspace: workspace.to_string(),
            category: category.clone(),
            created_at: Utc::now(),
            messages: Vec::new(),
        });

        conversation.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
        });

        let serialized = serde_json::to_string_pretty(&conversation)?;
        fs::write(&file_path, serialized)?;
        Ok(file_path)
    }

    pub fn load_recent(
        &self,
        workspace: &str,
        category: &str,
        limit: usize,
    ) -> Result<Vec<Message>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let category = normalize_category(category);
        let dir = self.category_dir(workspace, &category);
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut files = list_memory_files(&dir)?;
        files.sort();

        let mut collected: Vec<Message> = Vec::new();
        for file in files.into_iter().rev() {
            if let Some(conversation) = load_conversation(&file)? {
                for message in conversation.messages.into_iter().rev() {
                    collected.push(message);
                    if collected.len() >= limit {
                        break;
                    }
                }
            }
            if collected.len() >= limit {
                break;
            }
        }

        collected.reverse();
        Ok(collected)
    }

    pub fn list_categories(&self, workspace: &str) -> Result<Vec<String>> {
        let workspace_dir = self.workspace_dir.join(workspace).join(".memory");
        if !workspace_dir.exists() {
            return Ok(Vec::new());
        }
        let mut categories = Vec::new();
        for entry in fs::read_dir(workspace_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                categories.push(entry.file_name().to_string_lossy().to_string());
            }
        }
        categories.sort();
        Ok(categories)
    }

    fn category_dir(&self, workspace: &str, category: &str) -> PathBuf {
        self.workspace_dir
            .join(workspace)
            .join(".memory")
            .join(category)
    }

    fn find_or_create_today_file(
        &self,
        dir: &Path,
        workspace: &str,
        category: &str,
    ) -> Result<PathBuf> {
        let date = Local::now().format("%Y-%m-%d").to_string();
        let mut max_nnn = 0u32;

        for file in list_memory_files(dir)? {
            if let Some(nnn) = parse_nnn(&file, &date) {
                if nnn > max_nnn {
                    max_nnn = nnn;
                }
            }
        }

        let next_nnn = if max_nnn == 0 { 1 } else { max_nnn };
        let file_name = format!("{date}-{next_nnn:03}.json");
        let file_path = dir.join(file_name);

        if !file_path.exists() {
            let conversation = Conversation {
                id: format!("{date}-{next_nnn:03}"),
                workspace: workspace.to_string(),
                category: category.to_string(),
                created_at: Utc::now(),
                messages: Vec::new(),
            };
            let serialized = serde_json::to_string_pretty(&conversation)?;
            fs::write(&file_path, serialized)?;
        }
        Ok(file_path)
    }
}

fn load_conversation(path: &Path) -> Result<Option<Conversation>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(None);
    }
    let conversation: Conversation = serde_json::from_str(&data)?;
    Ok(Some(conversation))
}

fn file_id_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn list_memory_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            files.push(entry.path());
        }
    }
    Ok(files)
}

fn parse_nnn(path: &Path, date: &str) -> Option<u32> {
    let stem = path.file_stem()?.to_string_lossy();
    let prefix = format!("{date}-");
    if !stem.starts_with(&prefix) {
        return None;
    }
    stem.strip_prefix(&prefix)?.parse().ok()
}

fn normalize_category(category: &str) -> String {
    let trimmed = category.trim();
    if trimmed.is_empty() {
        "general".to_string()
    } else {
        trimmed.to_lowercase().replace(' ', "-")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn append_and_load_recent() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());

        manager
            .append_message("demo", "notes", Role::User, "hello")
            .unwrap();
        manager
            .append_message("demo", "notes", Role::Assistant, "hi")
            .unwrap();

        let messages = manager.load_recent("demo", "notes", 10).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].content, "hello");
        assert_eq!(messages[1].content, "hi");
    }

    #[test]
    fn categories_are_listed() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());

        manager
            .append_message("demo", "tasks", Role::User, "one")
            .unwrap();
        manager
            .append_message("demo", "ideas", Role::User, "two")
            .unwrap();

        let categories = manager.list_categories("demo").unwrap();
        assert_eq!(categories, vec!["ideas".to_string(), "tasks".to_string()]);
    }
}
