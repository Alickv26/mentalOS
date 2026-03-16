use mentalOS::ui::chat_view::ChatView;
use mentalOS::ui::omni_pill::OmniPill;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{OnceLock, mpsc};

type GtkJob = Box<dyn FnOnce() + Send + 'static>;

fn run_on_gtk_thread<F>(test_name: &str, test_fn: F)
where
    F: FnOnce() + Send + 'static,
{
    static GTK_TEST_QUEUE: OnceLock<Option<mpsc::Sender<GtkJob>>> = OnceLock::new();
    let sender_opt = GTK_TEST_QUEUE.get_or_init(|| {
        let (job_tx, job_rx) = mpsc::channel::<GtkJob>();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        std::thread::spawn(move || {
            let initialized = gtk4::init().is_ok();
            let _ = ready_tx.send(initialized);
            if !initialized {
                return;
            }
            for job in job_rx {
                job();
            }
        });

        if ready_rx.recv().unwrap_or(false) {
            Some(job_tx)
        } else {
            None
        }
    });

    let Some(sender) = sender_opt else {
        eprintln!(
            "Skipping GTK accessibility test '{}': GTK init unavailable",
            test_name
        );
        return;
    };

    let (done_tx, done_rx) = mpsc::sync_channel(1);
    sender
        .send(Box::new(move || {
            let result = panic::catch_unwind(AssertUnwindSafe(test_fn));
            let _ = done_tx.send(result);
        }))
        .expect("GTK test job should be queued");

    match done_rx.recv().expect("GTK test result should be returned") {
        Ok(()) => {}
        Err(payload) => panic::resume_unwind(payload),
    }
}

#[test]
fn omni_pill_exposes_accessible_labels_and_roles() {
    run_on_gtk_thread("omni_pill_exposes_accessible_labels_and_roles", || {
        let pill = OmniPill::new();

        assert!(gtk4::test_accessible_has_role(
            &pill.agent_selector,
            gtk4::AccessibleRole::ComboBox
        ));
        assert!(gtk4::test_accessible_has_role(
            &pill.input,
            gtk4::AccessibleRole::TextBox
        ));
        assert!(gtk4::test_accessible_has_property(
            &pill.input,
            gtk4::AccessibleProperty::Label
        ));
        assert!(gtk4::test_accessible_has_property(
            &pill.input,
            gtk4::AccessibleProperty::KeyShortcuts
        ));
        assert!(gtk4::test_accessible_has_property(
            &pill.apps_btn,
            gtk4::AccessibleProperty::Label
        ));
    });
}

#[test]
fn chat_view_exposes_accessible_region_metadata() {
    run_on_gtk_thread("chat_view_exposes_accessible_region_metadata", || {
        let chat = ChatView::new();
        assert!(gtk4::test_accessible_has_role(
            &chat.container,
            gtk4::AccessibleRole::Region
        ));
        assert!(gtk4::test_accessible_has_property(
            &chat.container,
            gtk4::AccessibleProperty::Label
        ));
    });
}
