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
    #[serde(default, skip_serializing_if = "is_false")]
    pub local_only: bool,
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
    pub local_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncConversation {
    pub category: String,
    pub conversation: Conversation,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncPayload {
    pub workspace: String,
    pub exported_at: DateTime<Utc>,
    pub conversations: Vec<SyncConversation>,
    pub skipped_local_only: usize,
    pub redactions: usize,
}

#[derive(Debug, Clone)]
pub struct SyncExportResult {
    pub path: PathBuf,
    pub exported_count: usize,
    pub skipped_local_only: usize,
    pub redactions: usize,
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
        self.append_message_with_actions(workspace, category, role, content, Vec::new())
    }

    pub fn append_message_with_actions(
        &self,
        workspace: &str,
        category: &str,
        role: Role,
        content: impl Into<String>,
        actions: Vec<MessageAction>,
    ) -> Result<PathBuf> {
        let content = content.into();
        let category = normalize_category(category);
        let dir = self.category_dir(workspace, &category);
        fs::create_dir_all(&dir)?;

        let file_path = self.active_or_new_session_file(&dir)?;
        let _ = self.set_active_session(workspace, &category, &session_id_from_path(&file_path));
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
            actions,
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
                        local_only: conversation.metadata.local_only,
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
        if let Some(file) = find_session_file(&dir, session_id)? {
            return load_conversation(&file);
        }
        Ok(None)
    }

    pub fn set_active_session(
        &self,
        workspace: &str,
        category: &str,
        session_id: &str,
    ) -> Result<PathBuf> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        fs::create_dir_all(&dir)?;
        let marker = active_session_marker(&dir);
        fs::write(&marker, session_id)?;
        Ok(marker)
    }

    pub fn start_new_session(&self, workspace: &str, category: &str) -> Result<PathBuf> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        fs::create_dir_all(&dir)?;
        let marker = active_session_marker(&dir);
        fs::write(&marker, "NEW")?;
        Ok(marker)
    }

    pub fn active_session_id(&self, workspace: &str, category: &str) -> Result<Option<String>> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        if !dir.exists() {
            return Ok(None);
        }
        Ok(read_active_session_id(&dir))
    }

    pub fn delete_session(
        &self,
        workspace: &str,
        category: &str,
        session_id: &str,
    ) -> Result<bool> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        if !dir.exists() {
            return Ok(false);
        }

        let Some(file) = find_session_file(&dir, session_id)? else {
            return Ok(false);
        };

        fs::remove_file(file)?;
        if let Some(active) = read_active_session_id(&dir)
            && active == session_id
        {
            let marker = active_session_marker(&dir);
            let _ = fs::remove_file(marker);
        }
        Ok(true)
    }

    pub fn set_local_only(
        &self,
        workspace: &str,
        category: &str,
        session_id: &str,
        local_only: bool,
    ) -> Result<bool> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        if !dir.exists() {
            return Ok(false);
        }
        let Some(file) = find_session_file(&dir, session_id)? else {
            return Ok(false);
        };
        let Some(mut conversation) = load_conversation(&file)? else {
            return Ok(false);
        };
        conversation.metadata.local_only = local_only;
        let serialized = serde_json::to_string_pretty(&conversation)?;
        fs::write(file, serialized)?;
        Ok(true)
    }

    pub fn build_sync_payload(
        &self,
        workspace: &str,
        selected: &[(String, String)],
    ) -> Result<SyncPayload> {
        let mut conversations = Vec::new();
        let mut skipped_local_only = 0usize;
        let mut redactions = 0usize;

        for (category, session_id) in selected {
            let loaded = self.load_session(workspace, category, session_id)?;
            let Some(mut conversation) = loaded else {
                continue;
            };
            if conversation.metadata.local_only {
                skipped_local_only += 1;
                continue;
            }
            for message in &mut conversation.messages {
                let (updated, count) = redact_sensitive_text(&message.content);
                if count > 0 {
                    message.content = updated;
                    redactions += count;
                }
            }
            conversations.push(SyncConversation {
                category: category.clone(),
                conversation,
            });
        }

        Ok(SyncPayload {
            workspace: workspace.to_string(),
            exported_at: Utc::now(),
            conversations,
            skipped_local_only,
            redactions,
        })
    }

    pub fn export_sync_payload(
        &self,
        workspace: &str,
        selected: &[(String, String)],
    ) -> Result<SyncExportResult> {
        let payload = self.build_sync_payload(workspace, selected)?;
        let export_dir = self
            .workspace_dir
            .join(workspace)
            .join(".memory")
            .join("sync-exports");
        fs::create_dir_all(&export_dir)?;
        let filename = format!("sync-export-{}.json", Utc::now().format("%Y%m%d-%H%M%S"));
        let path = export_dir.join(filename);
        let serialized = serde_json::to_string_pretty(&payload)?;
        fs::write(&path, serialized)?;
        Ok(SyncExportResult {
            path,
            exported_count: payload.conversations.len(),
            skipped_local_only: payload.skipped_local_only,
            redactions: payload.redactions,
        })
    }

    pub fn set_workspace_for_active_session(
        &self,
        workspace: &str,
        category: &str,
        project_workspace: &str,
    ) -> Result<bool> {
        let category = normalize_category(category);
        let Some(active) = self.active_session_id(workspace, &category)? else {
            return Ok(false);
        };
        self.set_workspace_for_session(workspace, &category, &active, project_workspace)
    }

    pub fn set_workspace_for_session(
        &self,
        workspace: &str,
        category: &str,
        session_id: &str,
        project_workspace: &str,
    ) -> Result<bool> {
        let dir = self.category_dir(workspace, &normalize_category(category));
        if !dir.exists() {
            return Ok(false);
        }
        let Some(file) = find_session_file(&dir, session_id)? else {
            return Ok(false);
        };
        let Some(mut conversation) = load_conversation(&file)? else {
            return Ok(false);
        };
        conversation.metadata.workspace = project_workspace.to_string();
        let serialized = serde_json::to_string_pretty(&conversation)?;
        fs::write(file, serialized)?;
        Ok(true)
    }

    pub fn find_related_session_for_project(
        &self,
        workspace: &str,
        project_workspace: &Path,
    ) -> Result<Option<SessionSummary>> {
        let project_key = normalize_project_path(project_workspace);
        for session in self.list_sessions(workspace)? {
            let loaded =
                self.load_session(&session.workspace, &session.category, &session.session_id)?;
            let Some(conversation) = loaded else {
                continue;
            };
            if paths_match(
                project_key.as_deref(),
                Some(&conversation.metadata.workspace),
            ) {
                return Ok(Some(session));
            }
            let has_action_match = conversation.messages.iter().any(|m| {
                m.actions.iter().any(|a| {
                    if let Some(path) = &a.path {
                        paths_match(project_key.as_deref(), Some(path))
                    } else {
                        false
                    }
                })
            });
            if has_action_match {
                return Ok(Some(session));
            }
        }
        Ok(None)
    }

    pub fn load_related_conversation_for_project(
        &self,
        workspace: &str,
        project_workspace: &Path,
    ) -> Result<Option<Conversation>> {
        let Some(session) = self.find_related_session_for_project(workspace, project_workspace)?
        else {
            return Ok(None);
        };
        self.load_session(&session.workspace, &session.category, &session.session_id)
    }

    fn category_dir(&self, workspace: &str, category: &str) -> PathBuf {
        self.workspace_dir
            .join(workspace)
            .join(".memory")
            .join(category)
    }

    fn active_or_new_session_file(&self, dir: &Path) -> Result<PathBuf> {
        if let Some(active) = read_active_session_id(dir) {
            if active == "NEW" {
                return new_session_file(dir);
            }
            if let Some(active_file) = find_session_file(dir, &active)? {
                return Ok(active_file);
            }
            return new_session_file(dir);
        }
        let mut files = list_memory_files(dir)?;
        files.sort();
        if let Some(last) = files.last() {
            return Ok(last.clone());
        }

        new_session_file(dir)
    }
}

pub fn default_workspace_root() -> PathBuf {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join("workspaces")
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

    if is_legacy_session_id(&stem) {
        return stem;
    }
    if let Some((_, tail)) = stem.rsplit_once('-') {
        return tail.to_string();
    }
    stem
}

fn active_session_marker(dir: &Path) -> PathBuf {
    dir.join(".active")
}

fn read_active_session_id(dir: &Path) -> Option<String> {
    let marker = active_session_marker(dir);
    fs::read_to_string(marker)
        .ok()
        .map(|s| s.trim().to_string())
}

fn find_session_file(dir: &Path, session_id: &str) -> Result<Option<PathBuf>> {
    for file in list_memory_files(dir)? {
        if session_id_from_path(&file) == session_id {
            return Ok(Some(file));
        }
        if let Some(conversation) = load_conversation(&file)?
            && conversation.session_id == session_id
        {
            return Ok(Some(file));
        }
    }
    Ok(None)
}

fn new_session_file(dir: &Path) -> Result<PathBuf> {
    let now = Local::now();
    let date = now.format("%Y-%m-%d").to_string();
    let time = now.format("%H%M").to_string();
    let session_id = generate_session_id();
    let file_name = format!("{date}-{time}-{session_id}.json");
    Ok(dir.join(file_name))
}

fn is_legacy_session_id(stem: &str) -> bool {
    let mut parts = stem.split('-');
    let year = parts.next().unwrap_or("");
    let month = parts.next().unwrap_or("");
    let day = parts.next().unwrap_or("");
    let tail = parts.next().unwrap_or("");
    if parts.next().is_some() {
        return false;
    }
    year.len() == 4
        && month.len() == 2
        && day.len() == 2
        && !tail.is_empty()
        && stem.chars().all(|c| c.is_ascii_digit() || c == '-')
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

fn is_false(v: &bool) -> bool {
    !*v
}

fn normalize_project_path(path: &Path) -> Option<String> {
    let base = if path.exists() {
        fs::canonicalize(path).ok()
    } else {
        None
    }
    .unwrap_or_else(|| path.to_path_buf());
    let text = base.to_string_lossy().to_string();
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

fn paths_match(left: Option<&str>, right: Option<&str>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    if left == right {
        return true;
    }

    let left_norm = normalize_project_path(Path::new(left));
    let right_norm = normalize_project_path(Path::new(right));
    match (left_norm, right_norm) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn redact_sensitive_text(content: &str) -> (String, usize) {
    let mut redacted = content.to_string();
    let mut count = 0usize;

    let sensitive_prefixes = ["sk-", "ghp_", "xoxb-", "xoxp-", "xoxs-", "AIza", "Bearer "];
    for prefix in sensitive_prefixes {
        while let Some(start) = redacted.find(prefix) {
            let end = redacted[start..]
                .find(char::is_whitespace)
                .map(|idx| start + idx)
                .unwrap_or(redacted.len());
            redacted.replace_range(start..end, "[REDACTED]");
            count += 1;
        }
    }

    let mut lines = Vec::new();
    for line in redacted.lines() {
        let lowered = line.to_lowercase();
        let mut replaced = line.to_string();
        for marker in ["api_key", "token", "password", "secret", "authorization"] {
            if lowered.contains(marker) {
                if let Some(idx) = replaced.find('=') {
                    replaced = format!("{}=[REDACTED]", &replaced[..idx].trim());
                    count += 1;
                    break;
                }
                if let Some(idx) = replaced.find(':') {
                    replaced = format!("{}: [REDACTED]", &replaced[..idx].trim());
                    count += 1;
                    break;
                }
            }
        }
        lines.push(replaced);
    }

    (lines.join("\n"), count)
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

    #[test]
    fn active_session_marker_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let marker = active_session_marker(temp_dir.path());
        fs::write(&marker, "abc123").unwrap();
        assert_eq!(
            read_active_session_id(temp_dir.path()),
            Some("abc123".into())
        );
    }

    #[test]
    fn load_session_matches_full_session_id() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        let dir = temp_dir.path().join("demo").join(".memory").join("general");
        fs::create_dir_all(&dir).unwrap();

        let session_id = "2026-02-13-001";
        let conversation = Conversation {
            session_id: session_id.to_string(),
            created_at: Utc::now(),
            last_active: Utc::now(),
            title: Some("Legacy chat".to_string()),
            messages: vec![Message {
                role: Role::User,
                content: "hello".to_string(),
                timestamp: Utc::now(),
                actions: Vec::new(),
            }],
            metadata: ConversationMetadata {
                workspace: "demo".to_string(),
                ..ConversationMetadata::default()
            },
        };
        let file_path = dir.join(format!("{session_id}.json"));
        fs::write(
            &file_path,
            serde_json::to_string_pretty(&conversation).unwrap(),
        )
        .unwrap();

        let loaded = manager.load_session("demo", "general", session_id).unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().session_id, session_id);
    }

    #[test]
    fn active_session_id_reads_saved_marker() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .set_active_session("demo", "general", "session-42")
            .unwrap();
        let active = manager.active_session_id("demo", "general").unwrap();
        assert_eq!(active, Some("session-42".to_string()));
    }

    #[test]
    fn delete_session_removes_file_and_clears_active_marker() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message("demo", "general", Role::User, "hello")
            .unwrap();
        let session = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        manager
            .set_active_session("demo", "general", &session.session_id)
            .unwrap();

        let deleted = manager
            .delete_session("demo", "general", &session.session_id)
            .unwrap();
        assert!(deleted);
        assert_eq!(manager.list_sessions("demo").unwrap().len(), 0);
        assert_eq!(manager.active_session_id("demo", "general").unwrap(), None);
    }

    #[test]
    fn set_local_only_persists_in_summary() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message("demo", "general", Role::User, "hello")
            .unwrap();
        let session = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let updated = manager
            .set_local_only("demo", "general", &session.session_id, true)
            .unwrap();
        assert!(updated);
        let after = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        assert!(after.local_only);
    }

    #[test]
    fn build_sync_payload_skips_local_only_and_redacts_tokens() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message(
                "demo",
                "general",
                Role::User,
                "api_key=abc123\nAuthorization: Bearer secret-token-value",
            )
            .unwrap();
        let session = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let payload = manager
            .build_sync_payload(
                "demo",
                &[(session.category.clone(), session.session_id.clone())],
            )
            .unwrap();
        assert_eq!(payload.conversations.len(), 1);
        assert!(payload.redactions >= 1);
        let content = &payload.conversations[0].conversation.messages[0].content;
        assert!(!content.contains("abc123"));
        assert!(!content.contains("secret-token-value"));

        manager
            .set_local_only("demo", "general", &session.session_id, true)
            .unwrap();
        let payload = manager
            .build_sync_payload(
                "demo",
                &[(session.category.clone(), session.session_id.clone())],
            )
            .unwrap();
        assert_eq!(payload.conversations.len(), 0);
        assert_eq!(payload.skipped_local_only, 1);
    }

    #[test]
    fn append_message_with_actions_persists_action_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message_with_actions(
                "demo",
                "general",
                Role::Assistant,
                "Created project",
                vec![MessageAction {
                    action_type: "create_project".to_string(),
                    path: Some("/tmp/workspaces/demo".to_string()),
                    command: None,
                }],
            )
            .unwrap();
        let session = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let loaded = manager
            .load_session("demo", "general", &session.session_id)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.messages.len(), 1);
        assert_eq!(loaded.messages[0].actions.len(), 1);
        assert_eq!(
            loaded.messages[0].actions[0].path.as_deref(),
            Some("/tmp/workspaces/demo")
        );
    }

    #[test]
    fn finds_related_session_by_project_path() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        let project = temp_dir.path().join("workspaces").join("demo-app");
        fs::create_dir_all(&project).unwrap();

        manager
            .append_message_with_actions(
                "demo",
                "general",
                Role::Assistant,
                "Created project",
                vec![MessageAction {
                    action_type: "create_project".to_string(),
                    path: Some(project.to_string_lossy().to_string()),
                    command: None,
                }],
            )
            .unwrap();

        let related = manager
            .find_related_session_for_project("demo", &project)
            .unwrap();
        assert!(related.is_some());

        let loaded = manager
            .load_related_conversation_for_project("demo", &project)
            .unwrap();
        assert!(loaded.is_some());
    }

    #[test]
    fn export_sync_payload_writes_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = MemoryManager::new(temp_dir.path().to_path_buf());
        manager
            .append_message("demo", "general", Role::User, "token=super-secret")
            .unwrap();
        let session = manager
            .list_sessions("demo")
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let export = manager
            .export_sync_payload(
                "demo",
                &[(session.category.clone(), session.session_id.clone())],
            )
            .unwrap();
        assert!(export.path.exists());
        assert_eq!(export.exported_count, 1);
        assert!(export.redactions >= 1);
    }
}
