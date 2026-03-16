use crate::error::Result;
use chrono::{DateTime, Local, Utc};
use log::warn;
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageAction {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<MessageAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversationMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub total_tokens: u64,
    #[serde(default)]
    pub commands_executed: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub session_id: String,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub messages: Vec<Message>,
    #[serde(default)]
    pub metadata: ConversationMetadata,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub workspace: String,
    pub category: String,
    pub session_id: String,
    pub title: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_active: DateTime<Utc>,
}

/// Manages per-workspace, per-category conversation memory.
///
/// # Examples
/// ```no_run
/// use mental_os::memory::{MemoryManager, Role};
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

        let file_path = self.active_or_new_session_file(&dir)?;
        let mut conversation = load_conversation(&file_path)?.unwrap_or_else(|| Conversation {
            session_id: session_id_from_path(&file_path),
            created_at: Utc::now(),
            last_active: Utc::now(),
            title: None,
            messages: Vec::new(),
            metadata: ConversationMetadata {
                workspace: workspace.to_string(),
                ..ConversationMetadata::default()
            },
        });

        conversation.messages.push(Message {
            role,
            content,
            timestamp: Utc::now(),
            actions: Vec::new(),
        });
        conversation.last_active = Utc::now();

        if conversation.title.is_none() && conversation.messages.len() >= 4 {
            conversation.title = auto_title(&conversation.messages);
        }

        if conversation.metadata.workspace.is_empty() {
            conversation.metadata.workspace = workspace.to_string();
        }

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

    pub fn list_sessions(&self, workspace: &str) -> Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();
        for category in self.list_categories(workspace)? {
            let dir = self.category_dir(workspace, &category);
            for file in list_memory_files(&dir)? {
                if let Some(conversation) = load_conversation(&file)? {
                    sessions.push(SessionSummary {
                        workspace: workspace.to_string(),
                        category: category.clone(),
                        session_id: conversation.session_id,
                        title: conversation.title,
                        created_at: conversation.created_at,
                        last_active: conversation.last_active,
                    });
                }
            }
        }

        sessions.sort_by_key(|s| s.last_active);
        sessions.reverse();
        Ok(sessions)
    }

    pub fn load_session(
        &self,
        workspace: &str,
        category: &str,
        session_id: &str,
    ) -> Result<Option<Conversation>> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        if !dir.exists() {
            return Ok(None);
        }

        for file in list_memory_files(&dir)? {
            if session_id_from_path(&file) == session_id {
                return load_conversation(&file);
            }
        }
        Ok(None)
    }

    fn category_dir(&self, workspace: &str, category: &str) -> PathBuf {
        self.workspace_dir
            .join(workspace)
            .join(".memory")
            .join(category)
    }

    fn active_or_new_session_file(&self, dir: &Path) -> Result<PathBuf> {
        let mut files = list_memory_files(dir)?;
        files.sort();
        if let Some(last) = files.last() {
            return Ok(last.clone());
        }

        let now = Local::now();
        let date = now.format("%Y-%m-%d").to_string();
        let time = now.format("%H%M").to_string();
        let session_id = generate_session_id();
        let file_name = format!("{date}-{time}-{session_id}.json");
        Ok(dir.join(file_name))
    }
}

fn auto_title(messages: &[Message]) -> Option<String> {
    let first_user = messages.iter().find(|m| m.role == Role::User)?;
    let title = first_user
        .content
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() { None } else { Some(title) }
}

fn load_conversation(path: &Path) -> Result<Option<Conversation>> {
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    if data.trim().is_empty() {
        return Ok(None);
    }

    if let Ok(mut conversation) = serde_json::from_str::<Conversation>(&data) {
        if conversation.session_id.is_empty() {
            conversation.session_id = session_id_from_path(path);
        }
        return Ok(Some(conversation));
    }

    // Backward-compat read path for pre-session schema files.
    if let Ok(legacy) = serde_json::from_str::<LegacyConversation>(&data) {
        let mut conversation = Conversation {
            session_id: legacy.id.unwrap_or_else(|| session_id_from_path(path)),
            created_at: legacy.created_at,
            last_active: legacy
                .messages
                .last()
                .map(|m| m.timestamp)
                .unwrap_or(legacy.created_at),
            title: None,
            messages: legacy
                .messages
                .into_iter()
                .map(|m| Message {
                    role: m.role,
                    content: m.content,
                    timestamp: m.timestamp,
                    actions: m.actions.unwrap_or_default(),
                })
                .collect(),
            metadata: ConversationMetadata {
                workspace: legacy.workspace.unwrap_or_default(),
                ..ConversationMetadata::default()
            },
        };
        if conversation.title.is_none() && conversation.messages.len() >= 4 {
            conversation.title = auto_title(&conversation.messages);
        }

        // Best-effort in-place migration so future loads are fast and consistent.
        match serde_json::to_string_pretty(&conversation) {
            Ok(serialized) => {
                if let Err(err) = fs::write(path, serialized) {
                    warn!(
                        "Failed to persist migrated memory file {}: {}",
                        path.display(),
                        err
                    );
                }
            }
            Err(err) => warn!(
                "Failed to serialize migrated memory file {}: {}",
                path.display(),
                err
            ),
        }

        return Ok(Some(conversation));
    }

    // Skip unknown/invalid files instead of crashing chat flow.
    warn!(
        "Skipping unreadable memory file (schema mismatch): {}",
        path.display()
    );
    Ok(None)
}

fn session_id_from_path(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown-session".to_string());

    if let Some((_, tail)) = stem.rsplit_once('-') {
        return tail.to_string();
    }
    stem
}

fn list_memory_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry.path().extension().and_then(|e| e.to_str()) == Some("json")
        {
            files.push(entry.path());
        }
    }
    Ok(files)
}

fn normalize_category(category: &str) -> String {
    let trimmed = category.trim();
    if trimmed.is_empty() {
        "general".to_string()
    } else {
        trimmed.to_lowercase().replace(' ', "-")
    }
}

fn generate_session_id() -> String {
    let raw = Utc::now().timestamp_nanos_opt().unwrap_or_default();
    format!("{:x}", raw.abs())
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyConversation {
    id: Option<String>,
    #[serde(default)]
    workspace: Option<String>,
    created_at: DateTime<Utc>,
    #[serde(default)]
    messages: Vec<LegacyMessage>,
}

#[derive(Debug, Clone, Deserialize)]
struct LegacyMessage {
    role: Role,
    content: String,
    timestamp: DateTime<Utc>,
    #[serde(default)]
    actions: Option<Vec<MessageAction>>,
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

    #[test]
    fn auto_title_is_generated() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());

        manager
            .append_message(
                "demo",
                "general",
                Role::User,
                "Create a rust web server with auth",
            )
            .unwrap();
        manager
            .append_message("demo", "general", Role::Assistant, "Sure")
            .unwrap();
        manager
            .append_message("demo", "general", Role::User, "Add postgres")
            .unwrap();
        manager
            .append_message("demo", "general", Role::Assistant, "Done")
            .unwrap();

        let sessions = manager.list_sessions("demo").unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].title.is_some());
    }
}
