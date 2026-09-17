use mental_os::task_tracker::{TaskStatus, TaskTracker};
use tempfile::TempDir;

#[test]
fn extracted_tasks_persist_and_can_be_completed() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

    let created = tracker.extract_and_add_tasks(
        "TODO: ship release notes. I need to verify deployment by next week priority high.",
        Some("conv-42"),
        Some("alpha-app"),
    );

    assert!(created.len() >= 2, "expected at least two extracted tasks");
    assert_eq!(tracker.get_pending_count(), created.len());

    let first_id = created[0].id.clone();
    let updated = tracker
        .update_status(&first_id, TaskStatus::Completed)
        .expect("status update should succeed")
        .expect("task should exist");
    assert_eq!(updated.status, TaskStatus::Completed);

    let pending = tracker.list_tasks(Some(TaskStatus::Pending));
    assert_eq!(pending.len(), created.len() - 1);

    let by_project = tracker.get_tasks_by_project("alpha-app");
    assert_eq!(by_project.len(), created.len());

    let reloaded = TaskTracker::new(temp_dir.path().to_path_buf());
    let completed = reloaded
        .get_task(&first_id)
        .expect("completed task should persist");
    assert_eq!(completed.status, TaskStatus::Completed);
    assert_eq!(completed.related_conversation.as_deref(), Some("conv-42"));
}

#[test]
fn duplicate_task_descriptions_in_single_message_are_deduped() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let mut tracker = TaskTracker::new(temp_dir.path().to_path_buf());

    let created = tracker.extract_and_add_tasks(
        "TODO: fix auth bug. TODO: fix auth bug.",
        Some("conv-dup"),
        Some("alpha-app"),
    );

    assert_eq!(
        created.len(),
        1,
        "duplicate descriptions should be collapsed"
    );
    assert!(
        created[0]
            .description
            .to_lowercase()
            .contains("fix auth bug")
    );
    assert_eq!(tracker.get_pending_count(), 1);
}
