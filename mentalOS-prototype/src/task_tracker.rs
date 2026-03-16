use crate::error::Result;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskPriority {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub due_date: Option<DateTime<Utc>>,
    pub related_conversation: Option<String>,
    pub related_project: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskStore {
    pub tasks: Vec<Task>,
}

pub struct TaskTracker {
    tasks_path: PathBuf,
    store: TaskStore,
}

impl TaskTracker {
    pub fn new(workspace_dir: PathBuf) -> Self {
        let tasks_path = workspace_dir.join(".memory").join("tasks.json");
        let store = Self::load_tasks(&tasks_path).unwrap_or_default();
        Self { tasks_path, store }
    }

    fn load_tasks(path: &PathBuf) -> Result<TaskStore> {
        if !path.exists() {
            return Ok(TaskStore::default());
        }
        let data = fs::read_to_string(path)?;
        let store: TaskStore = serde_json::from_str(&data)?;
        Ok(store)
    }

    fn save(&self) -> Result<()> {
        if let Some(parent) = self.tasks_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let serialized = serde_json::to_string_pretty(&self.store)?;
        fs::write(&self.tasks_path, serialized)?;
        Ok(())
    }

    pub fn extract_and_add_tasks(
        &mut self,
        content: &str,
        conversation_id: Option<&str>,
        project_name: Option<&str>,
    ) -> Vec<Task> {
        let mut added_tasks = Vec::new();

        let patterns = Self::task_patterns();

        for pattern in &patterns {
            for cap in pattern.regex.captures_iter(content) {
                if let Some(desc_match) = cap.get(1) {
                    let description = desc_match.as_str().trim().to_string();

                    let mut priority = TaskPriority::Medium;
                    let mut due_date = None;

                    // Group 2 is due date, Group 3 is priority (in the regex: (.+?)(?:by (.+?))?(?:priority (.+?))?)
                    if let Some(due_match) = cap.get(2) {
                        due_date = Self::parse_due_date(due_match.as_str());
                    }

                    if let Some(pri_match) = cap.get(3) {
                        priority = Self::parse_priority(pri_match.as_str());
                    }

                    let task = Task {
                        id: Self::generate_task_id(),
                        description,
                        created_at: Utc::now(),
                        status: TaskStatus::Pending,
                        priority,
                        due_date,
                        related_conversation: conversation_id.map(String::from),
                        related_project: project_name.map(String::from),
                    };

                    self.store.tasks.push(task.clone());
                    added_tasks.push(task);
                }
            }
        }

        if !added_tasks.is_empty() {
            let _ = self.save();
        }

        added_tasks
    }

    fn task_patterns() -> Vec<TaskPattern> {
        vec![
            TaskPattern {
                regex: Regex::new(r"(?i)(?:remind me to|remind me about|remember to)\s+(.+?)(?:\s+by\s+(.+?))?(?:\s+priority\s+(.+?))?(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)(?:TODO|TODO:)\s*(.+?)(?:\s+by\s+(.+?))?(?:\s+priority\s+(.+?))?(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)i need to\s+(.+?)(?:\s+by\s+(.+?))?(?:\s+priority\s+(.+?))?(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)i should\s+(.+?)(?:\s+by\s+(.+?))?(?:\s+priority\s+(.+?))?(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)(?:action item|action:)\s+(.+?)(?:\s+by\s+(.+?))?(?:\s+priority\s+(.+?))?(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)don'?t forget to\s+(.+?)(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)make sure to\s+(.+?)(?:\.|$)").unwrap(),
            },
            TaskPattern {
                regex: Regex::new(r"(?i)(?:follow up|followup)\s+(?:on\s+)?(.+?)(?:\.|$)").unwrap(),
            },
        ]
    }

    fn parse_priority(text: &str) -> TaskPriority {
        let lower = text.to_lowercase();
        if lower.contains("critical") || lower.contains("urgent") || lower.contains("asap") {
            TaskPriority::Critical
        } else if lower.contains("high") {
            TaskPriority::High
        } else if lower.contains("low") {
            TaskPriority::Low
        } else {
            TaskPriority::Medium
        }
    }

    fn parse_due_date(text: &str) -> Option<DateTime<Utc>> {
        let lower = text.to_lowercase();

        if let Some(duration) = Self::parse_relative_date(&lower) {
            return Some(Utc::now() + duration);
        }

        if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
            return Some(dt.with_timezone(&Utc));
        }

        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%d %H:%M") {
            return Some(dt.and_utc());
        }

        None
    }

    fn parse_relative_date(text: &str) -> Option<chrono::Duration> {
        let pattern = Regex::new(r"(\d+)\s*(day|week|hour|minute|month)s?").ok()?;

        if let Some(cap) = pattern.captures(text) {
            let num: i64 = cap.get(1)?.as_str().parse().ok()?;
            let unit = cap.get(2)?.as_str().to_lowercase();

            return Some(match unit.as_str() {
                "minute" | "minutes" => chrono::Duration::minutes(num),
                "hour" | "hours" => chrono::Duration::hours(num),
                "day" | "days" => chrono::Duration::days(num),
                "week" | "weeks" => chrono::Duration::weeks(num),
                "month" | "months" => chrono::Duration::days(num * 30),
                _ => return None,
            });
        }

        if text.contains("tomorrow") {
            return Some(chrono::Duration::days(1));
        }
        if text.contains("today") {
            return Some(chrono::Duration::zero());
        }
        if text.contains("next week") {
            return Some(chrono::Duration::weeks(1));
        }

        None
    }

    fn generate_task_id() -> String {
        format!("task-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"))
    }

    pub fn list_tasks(&self, status: Option<TaskStatus>) -> Vec<Task> {
        match status {
            Some(s) => self
                .store
                .tasks
                .iter()
                .filter(|t| t.status == s)
                .cloned()
                .collect(),
            None => self.store.tasks.clone(),
        }
    }

    pub fn get_task(&self, id: &str) -> Option<Task> {
        self.store.tasks.iter().find(|t| t.id == id).cloned()
    }

    pub fn update_status(&mut self, id: &str, status: TaskStatus) -> Result<Option<Task>> {
        let task_index = self.store.tasks.iter().position(|t| t.id == id);

        if let Some(idx) = task_index {
            self.store.tasks[idx].status = status;
            let task = self.store.tasks[idx].clone();
            let _ = self.save();
            return Ok(Some(task));
        }
        Ok(None)
    }

    pub fn delete_task(&mut self, id: &str) -> Result<bool> {
        let initial_len = self.store.tasks.len();
        self.store.tasks.retain(|t| t.id != id);
        if self.store.tasks.len() != initial_len {
            let _ = self.save();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn get_pending_count(&self) -> usize {
        self.store
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count()
    }

    pub fn get_tasks_by_project(&self, project: &str) -> Vec<Task> {
        self.store
            .tasks
            .iter()
            .filter(|t| t.related_project.as_deref() == Some(project))
            .cloned()
            .collect()
    }
}

struct TaskPattern {
    regex: Regex,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn extracts_remind_me_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

        let tasks = tracker.extract_and_add_tasks(
            "Remind me to deploy the server by next week priority high",
            None,
            None,
        );

        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].description.contains("deploy"));
        assert_eq!(tasks[0].priority, TaskPriority::High);
    }

    #[test]
    fn extracts_todo_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

        let tasks = tracker.extract_and_add_tasks("TODO: Fix the login bug", None, None);

        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].description.contains("login"));
    }

    #[test]
    fn extracts_i_need_to_tasks() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

        let tasks = tracker.extract_and_add_tasks(
            "I need to update the documentation",
            Some("conv-123"),
            Some("my-project"),
        );

        assert_eq!(tasks.len(), 1);
        assert!(tasks[0].description.contains("documentation"));
        assert_eq!(tasks[0].related_conversation, Some("conv-123".to_string()));
        assert_eq!(tasks[0].related_project, Some("my-project".to_string()));
    }

    #[test]
    fn updates_task_status() {
        let temp_dir = TempDir::new().unwrap();
        let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

        let tasks = tracker.extract_and_add_tasks("Remind me to test", None, None);
        let task_id = &tasks[0].id;

        let updated = tracker
            .update_status(task_id, TaskStatus::Completed)
            .unwrap();
        assert!(updated.is_some());
        assert_eq!(updated.unwrap().status, TaskStatus::Completed);
    }
}
