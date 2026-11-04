mod close;

use std::cell::Cell;
use std::rc::Rc;

use crownos_disks::command::{OperationId, Target};
use crownos_disks::fixtures;
use crownos_disks::guard::Fingerprint;
use crownos_disks::journal::{JournalEntry, JournalOperation};
use crownos_disks::model::{ByteSize, DiskId, Inventory, Region};
use crownos_disks::{Command, Event};
use crowntest::{Role, TestApp, role, text};
use crownui::kit::{Theme, provide_theme, provide_toasts};
use crownui::prelude::{
    Listeners, Semantic, Styled, View, bind, button, component, vstack, zstack,
};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use super::fixtures::{lose_service, populate};
use super::service_unavailable::SERVICE_UNAVAILABLE_TITLE;
use super::shell::shell;
use crate::state::Store;

const WIDTH: f32 = 1100.0;
const HEIGHT: f32 = 740.0;
const TOOLTIP_DELAY_MS: u64 = 700;
const SYSTEM_DISK: &str = "SAMSUNG MZVL2512HDJD-00BL2";
const DIALOG_PROBE: &str = "Dialog probe";
const SELECTION_PROBE: &str = "Selection probe";
const USB_STICK: &str = "SanDisk Ultra";
const DELIVER_PROBE: &str = "Deliver inventory";

enum Backend {
    Running(Inventory),
    Busy(Inventory, Box<Event>),
    Late(Inventory),
    Lost,
}

struct Window {
    app: TestApp,
    store: Store,
    commands: UnboundedReceiver<Command>,
}

impl Window {
    fn open(backend: Backend) -> Self {
        let (sender, commands) = unbounded_channel();
        let slot = Rc::new(Cell::new(None));
        let published = Rc::clone(&slot);
        let mut app = TestApp::with_size(
            component(move |cx| {
                provide_theme(cx, Theme::light());
                provide_toasts(cx);
                let store = Store::new(cx);
                published.set(Some(store));
                store.connect(cx, sender.clone());
                match &backend {
                    Backend::Running(inventory) => populate(cx, store, inventory.clone()),
                    Backend::Busy(inventory, event) => {
                        populate(cx, store, inventory.clone());
                        store.apply(cx, (**event).clone());
                    }
                    Backend::Lost => lose_service(cx, store),
                    Backend::Late(_) => {}
                }
                let deliver = match &backend {
                    Backend::Late(inventory) => {
                        let inventory = inventory.clone();
                        button(DELIVER_PROBE)
                            .on_click(move |cx, _| populate(cx, store, inventory.clone()))
                            .absolute()
                            .w(1.0)
                            .h(1.0)
                            .boxed()
                    }
                    _ => crate::util::nothing::nothing().boxed(),
                };
                zstack((shell(cx, store), probes(store), deliver))
                    .w_full()
                    .h_full()
            }),
            WIDTH,
            HEIGHT,
        );
        app.settle();
        let store = slot.get().expect("the root builds synchronously");
        Self {
            app,
            store,
            commands,
        }
    }

    fn apply(&mut self, event: Event) {
        let store = self.store;
        store.apply(self.app.ui().cx(), event);
        self.app.settle();
    }

    fn with(inventory: Inventory) -> Self {
        Self::open(Backend::Running(inventory))
    }

    fn sent(&mut self) -> Vec<Command> {
        std::iter::from_fn(|| self.commands.try_recv().ok()).collect()
    }

    fn probe(&mut self, name: &str) -> String {
        self.app
            .find(role(Role::Group).name(name))
            .value
            .unwrap_or_default()
    }

    fn first_name(&mut self, query: crowntest::Query) -> String {
        let tree = self.app.ui().semantic_tree();
        query
            .find_all(tree)
            .first()
            .and_then(|node| node.name.clone())
            .unwrap_or_default()
    }
}

fn probes(store: Store) -> impl crownui::prelude::View {
    vstack((
        vstack(())
            .role(Role::Group)
            .label(DIALOG_PROBE)
            .value(bind(move |runtime| {
                format!("{:?}", store.dialog.get(runtime))
            })),
        vstack(())
            .role(Role::Group)
            .label(SELECTION_PROBE)
            .value(bind(move |runtime| {
                format!("{:?}", store.selection.get(runtime))
            })),
    ))
    .absolute()
    .pointer_events_none()
}

fn every_fixture() -> [(&'static str, Inventory); 5] {
    [
        ("desktop", fixtures::desktop()),
        ("removable", fixtures::removable()),
        ("encrypted", fixtures::encrypted()),
        ("ata_failing", fixtures::ata_failing()),
        ("blank_disk", fixtures::blank_disk()),
    ]
}

#[test]
fn every_disk_of_every_fixture_opens_its_page() {
    for (fixture, inventory) in every_fixture() {
        let names: Vec<String> = inventory
            .disks
            .iter()
            .map(|disk| disk.name.clone())
            .collect();
        let mut window = Window::with(inventory);
        assert!(
            window.app.exists(role(Role::ScrollView).name("Overview")),
            "{fixture}"
        );
        for name in names {
            window.app.click(role(Role::MenuItem).name(name.as_str()));
            window.app.settle();
            assert!(
                window
                    .app
                    .exists(role(Role::ScrollView).name(name.as_str())),
                "{fixture}: {name} did not open:\n{}",
                window.app.semantics_snapshot()
            );
            assert!(
                window.app.exists(role(Role::Group).name("Partition map")),
                "{fixture}: {name}"
            );
        }
    }
}

#[test]
fn overview_cards_open_their_disk() {
    let mut window = Window::with(fixtures::removable());
    window.app.click(role(Role::Button).name("SanDisk Ultra"));
    window.app.settle();
    assert!(
        window
            .app
            .exists(role(Role::ScrollView).name("SanDisk Ultra"))
    );
}

#[test]
fn the_sidebar_groups_disks_by_where_they_live() {
    let mut window = Window::with(fixtures::removable());
    for section in ["Internal", "External", "Disk Images"] {
        assert!(
            window.app.exists(text(section).role(Role::Text)),
            "{section} is listed"
        );
    }
    let mut window = Window::with(fixtures::desktop());
    assert!(window.app.exists(text("Internal").role(Role::Text)));
    assert!(
        !window.app.exists(text("External").role(Role::Text)),
        "an empty section is hidden"
    );
    assert!(!window.app.exists(text("Disk Images").role(Role::Text)));
}

#[test]
fn only_removable_drives_and_images_get_an_eject_button() {
    let mut window = Window::with(fixtures::desktop());
    assert_eq!(window.app.count(role(Role::Button).name("Eject")), 0);
    assert_eq!(window.app.count(role(Role::Button).name("Close Image")), 0);

    let mut window = Window::with(fixtures::removable());
    assert_eq!(window.app.count(role(Role::Button).name("Eject")), 1);
    assert_eq!(window.app.count(role(Role::Button).name("Close Image")), 1);
}

#[test]
fn eject_in_the_sidebar_sends_without_confirmation() {
    let mut window = Window::with(fixtures::removable());
    window.app.click(role(Role::Button).name("Eject"));
    assert!(matches!(window.sent().as_slice(), [Command::Eject(_)]));
    assert!(
        window.probe(DIALOG_PROBE).contains("None"),
        "no dialog for tier 0"
    );

    window.app.click(role(Role::Button).name("Close Image"));
    assert!(matches!(window.sent().as_slice(), [Command::CloseImage(_)]));
}

#[test]
fn opening_a_disk_page_refreshes_its_health() {
    let mut window = Window::with(fixtures::desktop());
    window.app.click(role(Role::MenuItem).name(SYSTEM_DISK));
    window.app.settle();
    assert!(
        window
            .sent()
            .iter()
            .any(|command| matches!(command, Command::RefreshHealth(_)))
    );
}

#[test]
fn the_system_disk_refuses_erase_and_says_why() {
    let mut window = Window::with(fixtures::desktop());
    window.app.click(role(Role::MenuItem).name(SYSTEM_DISK));
    window.app.settle();
    let erase = window.app.find(role(Role::Button).name("Erase"));
    assert!(erase.states.disabled, "erase stays visible but disabled");
    assert_eq!(
        erase.description.as_deref(),
        Some("This is the disk CrownOS is running from.")
    );

    let center = erase.bounds.center();
    window.app.pointer_move(center.x, center.y);
    window.app.advance_ms(TOOLTIP_DELAY_MS);
    window.app.settle();
    assert!(
        window
            .app
            .exists(role(Role::Tooltip).name("This is the disk CrownOS is running from.")),
        "the tooltip explains"
    );
}

#[test]
fn a_free_segment_offers_new_partition() {
    let inventory = fixtures::ata_failing();
    let disk = inventory
        .disks
        .iter()
        .find(|disk| {
            disk.layout
                .iter()
                .any(|region| matches!(region, Region::Free { .. }))
        })
        .map(|disk| disk.name.clone())
        .unwrap_or_default();
    let mut window = Window::with(inventory);
    window.app.click(role(Role::MenuItem).name(disk.as_str()));
    window.app.settle();
    let free = window.first_name(role(Role::Button).text("Free space"));
    window.app.click(role(Role::Button).name(free.as_str()));
    window.app.settle();
    assert!(window.probe(SELECTION_PROBE).starts_with("Free"));

    let new_partition = window.app.find(role(Role::Button).name("New Partition"));
    assert!(
        !new_partition.states.disabled,
        "{:?}",
        new_partition.description
    );
    window.app.click(role(Role::Button).name("New Partition"));
    assert!(window.probe(DIALOG_PROBE).contains("NewPartition"));
}

#[test]
fn arrow_keys_walk_the_partition_map() {
    let mut window = Window::with(fixtures::desktop());
    window.app.click(role(Role::MenuItem).name(SYSTEM_DISK));
    window.app.settle();
    let first = window.first_name(role(Role::Button).text("FAT32"));
    window.app.click(role(Role::Button).name(first.as_str()));
    let before = window.probe(SELECTION_PROBE);
    window.app.press_key("ArrowRight");
    window.app.settle();
    let after = window.probe(SELECTION_PROBE);
    assert_ne!(before, after, "→ moves to the next region");
    window.app.press_key("ArrowLeft");
    window.app.settle();
    assert_eq!(window.probe(SELECTION_PROBE), before, "← comes back");
}

#[test]
fn a_mounted_volume_offers_unmount_directly() {
    let mut window = Window::with(fixtures::desktop());
    window
        .app
        .click(role(Role::MenuItem).name("Workspace ext4"));
    window.app.settle();
    assert!(window.app.exists(role(Role::Group).name("Selected region")));
    assert!(window.app.exists(role(Role::Button).name("Open in Files")));
    let unmount = window.app.find(role(Role::Button).name("Unmount"));
    if !unmount.states.disabled {
        window.app.click(role(Role::Button).name("Unmount"));
        assert!(matches!(
            window.sent().as_slice(),
            [.., Command::Unmount(_)]
        ));
    }
}

#[test]
fn a_lost_storage_service_replaces_the_pages() {
    let mut window = Window::open(Backend::Lost);
    assert!(
        window
            .app
            .exists(role(Role::Group).name(SERVICE_UNAVAILABLE_TITLE))
    );
    assert!(!window.app.exists(role(Role::ScrollView).name("Overview")));
}

#[test]
fn system_disk_page_semantics() {
    let mut window = Window::with(fixtures::desktop());
    window.app.click(role(Role::MenuItem).name(SYSTEM_DISK));
    window.app.settle();
    insta::assert_snapshot!(window.app.semantics_snapshot());
}

fn disk_id(inventory: &Inventory, name: &str) -> DiskId {
    inventory
        .disks
        .iter()
        .find(|disk| disk.name == name)
        .map(|disk| disk.id.clone())
        .unwrap_or_else(|| panic!("{name} is in the fixture"))
}

#[test]
fn a_busy_disk_disables_its_actions() {
    let inventory = fixtures::removable();
    let started = Event::OperationStarted {
        id: OperationId(1),
        label: "Restoring image".to_owned(),
        target: Target::Disk(disk_id(&inventory, USB_STICK)),
        cancelable: true,
        quit_impact: crownos_disks::command::QuitImpact::Harmless,
    };
    let mut window = Window::with(inventory.clone());
    window.app.click(role(Role::MenuItem).name(USB_STICK));
    window.app.settle();
    assert!(
        !window
            .app
            .find(role(Role::Button).name("Erase"))
            .states
            .disabled
    );

    let mut window = Window::open(Backend::Busy(inventory, Box::new(started)));
    window.app.click(role(Role::MenuItem).name(USB_STICK));
    window.app.settle();
    let erase = window.app.find(role(Role::Button).name("Erase"));
    assert!(erase.states.disabled);
    assert_eq!(
        erase.description.as_deref(),
        Some("Another operation is running on this disk.")
    );
}

#[test]
fn an_interrupted_resize_offers_to_finish() {
    let inventory = fixtures::desktop();
    let entry = JournalEntry {
        operation: JournalOperation::Grow {
            target_size: ByteSize::GIB,
        },
        disk: disk_id(&inventory, SYSTEM_DISK),
        block: None,
        target_name: "Root".to_owned(),
        fingerprint: Fingerprint {
            drive_serial: String::new(),
            size: ByteSize::GIB,
            table_uuid: String::new(),
            partitions: Vec::new(),
        },
        completed_steps: 1,
        started_at_unix: 0,
    };
    let mut window = Window::open(Backend::Busy(
        inventory,
        Box::new(Event::Interrupted(vec![entry])),
    ));
    window.app.click(role(Role::MenuItem).name(SYSTEM_DISK));
    window.app.settle();
    assert!(window.app.exists(text("Resizing was interrupted")));
    window.sent();
    window.app.click(role(Role::Button).name("Finish Resizing"));
    assert!(matches!(
        window.sent().as_slice(),
        [Command::ResumeInterrupted(_)]
    ));
    window.app.click(role(Role::Button).name("Dismiss"));
    assert!(matches!(
        window.sent().as_slice(),
        [Command::DismissInterrupted(_)]
    ));
}

#[test]
fn a_blank_disk_is_unallocated_and_erase_comes_first() {
    let inventory = fixtures::blank_disk();
    let blank = inventory
        .disks
        .iter()
        .find(|disk| disk.table.is_none() && disk.partitions().next().is_none())
        .map(|disk| disk.name.clone())
        .unwrap_or_default();
    let mut window = Window::with(inventory);
    window.app.click(role(Role::MenuItem).name(blank.as_str()));
    window.app.settle();
    assert!(
        !window
            .app
            .find(role(Role::Button).name("Erase"))
            .states
            .disabled
    );
    let unallocated = window.first_name(role(Role::Button).text("Unallocated"));
    window
        .app
        .click(role(Role::Button).name(unallocated.as_str()));
    window.app.settle();
    assert!(
        window
            .app
            .find(role(Role::Button).name("New Partition"))
            .states
            .disabled,
        "no table yet"
    );
}

#[test]
fn a_whole_disk_filesystem_is_never_partition_zero() {
    let mut window = Window::with(fixtures::removable());
    assert!(!window.app.exists(text("Partition 0")));
}

#[test]
fn an_inventory_arriving_after_launch_fills_the_window() {
    let mut window = Window::open(Backend::Late(fixtures::desktop()));
    assert!(window.app.exists(text("No disks found")));
    window.app.click(role(Role::Button).name(DELIVER_PROBE));
    window.app.settle();
    assert!(window.app.exists(text("Internal").role(Role::Text)));
    assert_eq!(
        window.first_name(role(Role::Button).name(SYSTEM_DISK)),
        SYSTEM_DISK
    );
    assert!(!window.app.exists(text("No disks found")));
}
