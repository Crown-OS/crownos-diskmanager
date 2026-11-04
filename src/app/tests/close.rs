use crownos_disks::command::{Completion, OperationId, QuitImpact, Target};
use crownos_disks::fixtures;
use crownos_disks::{Command, Event};
use crowntest::{Role, role, text};

use super::{Backend, USB_STICK, Window, disk_id};
use crate::state::DialogRequest;

const ID: OperationId = OperationId(5);

fn running(label: &str, cancelable: bool, quit_impact: QuitImpact) -> Window {
    let inventory = fixtures::removable();
    let started = Event::OperationStarted {
        id: ID,
        label: label.to_owned(),
        target: Target::Disk(disk_id(&inventory, USB_STICK)),
        cancelable,
        quit_impact,
    };
    Window::open(Backend::Busy(inventory, Box::new(started)))
}

fn click(window: &mut Window, name: &str) {
    window.app.click(role(Role::Button).name(name));
    window.app.settle();
}

fn asking(window: &mut Window) -> bool {
    let store = window.store;
    store
        .dialog
        .with_untracked(window.app.ui().cx(), |request| {
            *request == Some(DialogRequest::QuitWhileBusy)
        })
}

fn focused_name(window: &mut Window) -> Option<String> {
    let id = window.app.ui().focused_semantic_id()?;
    window.app.ui().semantic_tree().get(id)?.name.clone()
}

#[test]
fn an_idle_window_closes_at_once() {
    let mut window = Window::with(fixtures::removable());
    assert!(window.app.request_close());
}

#[test]
fn quick_operations_do_not_hold_the_window() {
    let mut window = running("Mounting SanDisk Ultra", false, QuitImpact::Harmless);
    assert!(window.app.request_close());
}

#[test]
fn closing_during_an_image_restore_asks_and_keep_working_stays_open() {
    let mut window = running(
        "Restoring an image to SanDisk Ultra",
        true,
        QuitImpact::LeavesTargetUnusable,
    );
    assert!(!window.app.request_close());
    assert!(asking(&mut window));
    assert!(window.app.exists(text(
        "Quit while restoring an image to SanDisk Ultra is in progress?"
    )));
    assert!(
        window
            .app
            .exists(text("Stopping now can leave SanDisk Ultra unusable."))
    );
    assert_eq!(focused_name(&mut window).as_deref(), Some("Keep Working"));

    click(&mut window, "Keep Working");
    assert!(!asking(&mut window));
    assert!(!window.app.take_close_request());
    assert!(window.sent().is_empty());
}

#[test]
fn cancel_and_quit_closes_once_the_restore_has_stopped() {
    let mut window = running(
        "Restoring an image to SanDisk Ultra",
        true,
        QuitImpact::LeavesTargetUnusable,
    );
    assert!(!window.app.request_close());
    click(&mut window, "Cancel and Quit");
    assert!(matches!(window.sent().as_slice(), [Command::Cancel(ID)]));
    assert!(
        window
            .app
            .find(role(Role::Button).name("Cancelling…"))
            .states
            .disabled
    );
    assert!(!window.app.take_close_request());

    let completion = Completion {
        message: "Cancelled".to_owned(),
    };
    window.apply(Event::OperationFinished {
        id: ID,
        result: Ok(completion),
    });
    assert!(window.app.take_close_request());
}

#[test]
fn creating_an_image_warns_that_it_will_not_be_saved() {
    let mut window = running(
        "Creating an image of SanDisk Ultra",
        true,
        QuitImpact::LosesImage,
    );
    assert!(!window.app.request_close());
    assert!(window.app.exists(text("The image won't be saved.")));
}

#[test]
fn a_format_keeps_running_in_the_background_and_quit_closes() {
    let mut window = running(
        "Formatting SanDisk Ultra",
        false,
        QuitImpact::FinishesInBackground,
    );
    assert!(!window.app.request_close());
    assert!(window.app.exists(text(
        "Formatting SanDisk Ultra will keep running in the background and finish on its own."
    )));
    assert!(
        !window
            .app
            .exists(role(Role::Button).name("Cancel and Quit"))
    );
    click(&mut window, "Quit");
    assert!(window.sent().is_empty());
    assert!(window.app.take_close_request());
}

#[test]
fn a_resize_warns_that_quitting_stops_it_partway() {
    let mut window = running("Resizing SanDisk Ultra", false, QuitImpact::StopsPartway);
    assert!(!window.app.request_close());
    assert!(window.app.exists(text(
        "Quitting now stops resizing SanDisk Ultra before it finishes. Disk Manager will show how \
         to finish or recover the next time it opens."
    )));
    click(&mut window, "Quit");
    assert!(window.app.take_close_request());
}
