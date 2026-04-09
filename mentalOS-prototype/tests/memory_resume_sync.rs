use mental_os::memory::{MemoryManager, Role};
use tempfile::TempDir;

fn create_two_sessions(manager: &MemoryManager) -> (String, String) {
    manager
        .append_message("demo", "general", Role::User, "first session message")
        .expect("first message should persist");

    let first = manager
        .list_sessions("demo")
        .expect("sessions should list")
        .into_iter()
        .next()
        .expect("first session should exist")
        .session_id;

    manager
        .set_active_session("demo", "general", "NEW")
        .expect("resetting active session should succeed");
    manager
        .append_message("demo", "general", Role::User, "second session message")
        .expect("second message should persist");

    let sessions = manager.list_sessions("demo").expect("sessions should list");
    assert_eq!(sessions.len(), 2, "expected two independent sessions");

    let second = sessions
        .iter()
        .find(|s| s.session_id != first)
        .expect("second session should exist")
        .session_id
        .clone();

    (first, second)
}

#[test]
fn active_session_marker_supports_resume_flow() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let manager = MemoryManager::new(temp_dir.path().to_path_buf());

    let (first, second) = create_two_sessions(&manager);
    manager
        .set_active_session("demo", "general", &first)
        .expect("setting active session should succeed");

    let active = manager
        .active_session_id("demo", "general")
        .expect("active session id should read")
        .expect("active session should exist");

    assert_eq!(active, first);
    assert_ne!(active, second);
}

#[test]
fn sync_payload_updates_when_local_only_is_toggled() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let manager = MemoryManager::new(temp_dir.path().to_path_buf());

    let (first, second) = create_two_sessions(&manager);

    manager
        .set_local_only("demo", "general", &first, true)
        .expect("local-only toggle should persist");

    let selected = vec![
        ("general".to_string(), first.clone()),
        ("general".to_string(), second.clone()),
    ];

    let payload = manager
        .build_sync_payload("demo", &selected)
        .expect("sync payload should build");

    assert_eq!(payload.conversations.len(), 1);
    assert_eq!(payload.skipped_local_only, 1);
    assert_eq!(payload.conversations[0].conversation.session_id, second);

    manager
        .set_local_only("demo", "general", &first, false)
        .expect("local-only toggle should clear");

    let payload = manager
        .build_sync_payload("demo", &selected)
        .expect("sync payload should rebuild");

    assert_eq!(payload.conversations.len(), 2);
    assert_eq!(payload.skipped_local_only, 0);
}
