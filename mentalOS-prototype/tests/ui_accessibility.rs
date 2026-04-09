use gtk4::prelude::{ButtonExt, Cast, WidgetExt};
use mental_os::ui::app_bar::AppBar;
use mental_os::ui::chat_view::ChatView;
use mental_os::ui::omni_pill::OmniPill;
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

#[test]
fn app_bar_exposes_accessible_controls_for_critical_actions() {
    run_on_gtk_thread(
        "app_bar_exposes_accessible_controls_for_critical_actions",
        || {
            let bar = AppBar::new();
            let container = bar.container.clone();

            let mut buttons: Vec<gtk4::Button> = Vec::new();
            let mut child = container.first_child();
            while let Some(widget) = child {
                if let Ok(row) = widget.clone().downcast::<gtk4::Box>() {
                    let mut row_child = row.first_child();
                    while let Some(node) = row_child {
                        if let Ok(button) = node.clone().downcast::<gtk4::Button>() {
                            buttons.push(button);
                        }
                        row_child = node.next_sibling();
                    }
                }
                child = widget.next_sibling();
            }

            let button_labels: Vec<String> = buttons
                .iter()
                .filter_map(|btn| btn.label())
                .map(|s| s.to_string())
                .collect();
            assert!(button_labels.iter().any(|label| label == "New chat"));
            assert!(button_labels.iter().any(|label| label == "Memory"));
            assert!(button_labels.iter().any(|label| label.contains("STOP")));

            let stop_btn = buttons
                .iter()
                .find(|btn| btn.label().map(|l| l.contains("STOP")).unwrap_or(false))
                .expect("stop button should be present")
                .clone();
            assert!(gtk4::test_accessible_has_property(
                &stop_btn,
                gtk4::AccessibleProperty::Label
            ));
            assert!(gtk4::test_accessible_has_property(
                &stop_btn,
                gtk4::AccessibleProperty::KeyShortcuts
            ));
        },
    );
}

#[test]
fn app_bar_exposes_status_and_progress_accessibility_metadata() {
    run_on_gtk_thread(
        "app_bar_exposes_status_and_progress_accessibility_metadata",
        || {
            let bar = AppBar::new();
            let container = bar.container.clone();

            let mut found_status = false;
            let mut found_progress = false;

            let mut child = container.first_child();
            while let Some(widget) = child {
                if let Ok(row) = widget.clone().downcast::<gtk4::Box>() {
                    let mut row_child = row.first_child();
                    while let Some(node) = row_child {
                        if let Ok(label) = node.clone().downcast::<gtk4::Label>()
                            && gtk4::test_accessible_has_role(&label, gtk4::AccessibleRole::Status)
                        {
                            found_status = true;
                            assert!(gtk4::test_accessible_has_property(
                                &label,
                                gtk4::AccessibleProperty::Label
                            ));
                        }
                        row_child = node.next_sibling();
                    }
                }

                if let Ok(progress) = widget.clone().downcast::<gtk4::ProgressBar>() {
                    found_progress = true;
                    assert!(gtk4::test_accessible_has_role(
                        &progress,
                        gtk4::AccessibleRole::ProgressBar
                    ));
                    assert!(gtk4::test_accessible_has_property(
                        &progress,
                        gtk4::AccessibleProperty::Label
                    ));
                }
                child = widget.next_sibling();
            }

            assert!(found_status, "expected an AI status label in app bar");
            assert!(found_progress, "expected app bar progress bar");
        },
    );
}
