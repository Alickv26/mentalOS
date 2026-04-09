use mental_os::memory::{MemoryManager, Role};
use serde_json::Value;
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

#[test]
fn project_workspace_link_enables_related_session_lookup() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let manager = MemoryManager::new(temp_dir.path().to_path_buf());

    manager
        .append_message("demo", "general", Role::User, "create project alpha")
        .expect("message should persist");
    let session = manager
        .list_sessions("demo")
        .expect("sessions should list")
        .into_iter()
        .next()
        .expect("session should exist");

    let project_dir = temp_dir.path().join("workspaces").join("alpha");
    std::fs::create_dir_all(&project_dir).expect("project directory should be created");
    manager
        .set_active_session("demo", "general", &session.session_id)
        .expect("active session should set");
    let linked = manager
        .set_workspace_for_active_session("demo", "general", &project_dir.to_string_lossy())
        .expect("workspace link should succeed");
    assert!(linked, "expected workspace to be linked to active session");

    let related = manager
        .find_related_session_for_project("demo", &project_dir)
        .expect("related session lookup should succeed")
        .expect("related session should exist");
    assert_eq!(related.session_id, session.session_id);

    let loaded = manager
        .load_related_conversation_for_project("demo", &project_dir)
        .expect("related conversation should load")
        .expect("conversation should exist");
    assert_eq!(loaded.session_id, session.session_id);
}

#[test]
fn new_chat_marker_routes_future_messages_to_new_session() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let manager = MemoryManager::new(temp_dir.path().to_path_buf());

    manager
        .append_message("demo", "general", Role::User, "hello from first")
        .expect("first message should persist");
    let first_session = manager
        .list_sessions("demo")
        .expect("sessions should list")
        .into_iter()
        .next()
        .expect("first session should exist")
        .session_id;

    manager
        .set_active_session("demo", "general", "NEW")
        .expect("NEW marker should set");
    manager
        .append_message("demo", "general", Role::User, "hello from second")
        .expect("second message should persist");

    let sessions = manager.list_sessions("demo").expect("sessions should list");
    assert_eq!(sessions.len(), 2, "NEW marker should force a new session");
    let active = manager
        .active_session_id("demo", "general")
        .expect("active marker should read")
        .expect("active marker should exist");
    assert_ne!(active, first_session);

    let latest = manager
        .load_session("demo", "general", &active)
        .expect("latest session should load")
        .expect("latest conversation should exist");
    assert_eq!(latest.messages.len(), 1);
    assert!(latest.messages[0].content.contains("second"));
}

#[test]
fn exported_sync_payload_has_expected_shape_and_redacts_tokens() {
    let temp_dir = TempDir::new().expect("temp dir should be created");
    let manager = MemoryManager::new(temp_dir.path().to_path_buf());

    manager
        .append_message(
            "demo",
            "general",
            Role::User,
            "token=super-secret-value and api_key=abc123",
        )
        .expect("message should persist");
    let session = manager
        .list_sessions("demo")
        .expect("sessions should list")
        .into_iter()
        .next()
        .expect("session should exist");

    let export = manager
        .export_sync_payload(
            "demo",
            &[(session.category.clone(), session.session_id.clone())],
        )
        .expect("sync export should succeed");
    assert!(export.path.exists());
    assert!(export.redactions >= 1, "expected at least one redaction");

    let raw = std::fs::read_to_string(&export.path).expect("export file should be readable");
    assert!(!raw.contains("super-secret-value"));
    assert!(!raw.contains("abc123"));

    let parsed: Value = serde_json::from_str(&raw).expect("export json should parse");
    assert_eq!(parsed["workspace"], Value::String("demo".to_string()));
    assert!(parsed["exported_at"].is_string());
    assert!(parsed["conversations"].is_array());
    assert!(parsed["redactions"].as_u64().unwrap_or(0) >= 1);
}
