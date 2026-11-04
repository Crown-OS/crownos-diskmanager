use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use crownos_disks::model::Inventory;
use crownos_disks::{Command, Event};
use crowntest::{Role, TestApp, role};
use crownui::kit::{Theme, provide_theme, provide_toasts, toast_host};
use crownui::prelude::{Styled, component, vstack};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

use crate::dialogs::host;
use crate::state::{DialogRequest, Store};

const FRAME_MILLIS: u64 = 16;

pub struct Harness {
    pub app: TestApp,
    pub store: Store,
    commands: UnboundedReceiver<Command>,
}

impl Harness {
    pub fn new(inventory: Inventory) -> Self {
        let (sender, commands) = unbounded_channel();
        let slot = Rc::new(Cell::new(None));
        let published = Rc::clone(&slot);
        let app = TestApp::new(component(move |cx| {
            provide_theme(cx, Theme::dark());
            provide_toasts(cx);
            let store = Store::new(cx);
            store.connect(cx, sender);
            store.inventory.set(cx, Arc::new(inventory));
            published.set(Some(store));
            toast_host(vstack(host(store)).w_full().h_full().relative())
        }));
        let store = slot.get().expect("the root builds synchronously");
        let mut harness = Self {
            app,
            store,
            commands,
        };
        harness.app.settle();
        harness
    }

    pub fn opened(inventory: Inventory, request: DialogRequest) -> Self {
        let mut harness = Self::new(inventory);
        harness.open(request);
        harness
    }

    pub fn open(&mut self, request: DialogRequest) {
        let store = self.store;
        store.open_dialog(self.app.ui().cx(), request);
        self.app.settle();
    }

    pub fn apply(&mut self, event: Event) {
        let store = self.store;
        store.apply(self.app.ui().cx(), event);
        self.app.settle();
    }

    /// Applies `event` and renders one frame only, so short-lived toasts are still up.
    pub fn apply_briefly(&mut self, event: Event) {
        let store = self.store;
        store.apply(self.app.ui().cx(), event);
        self.app.advance_ms(FRAME_MILLIS);
    }

    pub fn set_inventory(&mut self, inventory: Inventory) {
        self.apply(Event::Inventory(Arc::new(inventory)));
    }

    pub fn sent(&mut self) -> Vec<Command> {
        std::iter::from_fn(|| self.commands.try_recv().ok()).collect()
    }

    pub fn dialogs(&mut self) -> usize {
        self.app.count(role(Role::Dialog))
    }

    pub fn has_text(&mut self, text: &str) -> bool {
        self.app.count(crowntest::text(text)) > 0
    }

    pub fn button_disabled(&mut self, name: &str) -> bool {
        self.app.find(role(Role::Button).name(name)).states.disabled
    }

    pub fn click(&mut self, name: &str) {
        self.app.click(role(Role::Button).name(name));
        self.app.settle();
    }

    /// Types like a keyboard would: secret fields refuse agent typing, not key presses.
    pub fn type_secret(&mut self, field: &str, text: &str) {
        self.app.click(role(Role::TextField).name(field));
        let event =
            windowing::WindowEvent::TextInput(input::TextInputEvent::Commit(text.to_owned()));
        self.app.deliver(event).settle();
    }

    pub fn focused_name(&mut self) -> Option<String> {
        let id = self.app.ui().focused_semantic_id()?;
        self.app.ui().semantic_tree().get(id)?.name.clone()
    }

    pub fn request(&mut self) -> Option<DialogRequest> {
        let store = self.store;
        store
            .dialog
            .with_untracked(self.app.ui().cx(), Clone::clone)
    }
}
