use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use anyhow::{Context, Result};
use capture::CaptureApp;
use crownos_diskmanager::state::{DialogRequest, Selection, Store};
use crownos_diskmanager::{app_config, shell};
use crownos_disks::model::Inventory;
use crownos_disks::{Command, Event};
use crowntest::{Query, Role, role};
use crownui::kit::{Toast, Toasts, provide_configured_theme, provide_toasts};
use crownui::prelude::{Point, component};
use input::{PointerButton, PointerEvent, TextInputEvent};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use windowing::WindowEvent;

const PARKED_POINTER: (f32, f32) = (1096.0, 736.0);

/// The real window shell over a fake backend: events are applied by hand and commands are
/// swallowed, so nothing ever reaches UDisks2.
pub struct Window {
    pub app: CaptureApp,
    store: Store,
    toasts: Toasts,
    _commands: UnboundedReceiver<Command>,
}

impl Window {
    pub fn blank() -> Result<Self> {
        let (sender, commands) = unbounded_channel();
        let slot = Rc::new(Cell::new(None));
        let published = Rc::clone(&slot);
        let app = capture::mount_app(
            component(move |cx| {
                provide_configured_theme(cx);
                let toasts = provide_toasts(cx);
                let store = Store::new(cx);
                store.connect(cx, sender);
                published.set(Some((store, toasts)));
                shell(cx, store)
            }),
            &app_config(),
            None,
        )?;
        let (store, toasts) = slot.get().context("the shell did not mount")?;
        Ok(Self {
            app,
            store,
            toasts,
            _commands: commands,
        })
    }

    pub fn with(inventory: Inventory) -> Result<Self> {
        let mut window = Self::blank()?;
        window.apply(Event::Inventory(Arc::new(inventory)));
        Ok(window)
    }

    pub fn apply(&mut self, event: Event) -> &mut Self {
        let store = self.store;
        store.apply(self.app.ui().cx(), event);
        self.app.settle();
        self
    }

    /// Shows `toast` as the app's toast host does; capturing settles the clock, so a toast
    /// meant to be captured must outlast the settle.
    pub fn show_toast(&mut self, toast: Toast) -> &mut Self {
        let toasts = self.toasts;
        toasts.show_with(self.app.ui().cx(), toast);
        self.app.settle();
        self
    }

    pub fn select(&mut self, selection: Selection) -> &mut Self {
        let store = self.store;
        store.selection.set(self.app.ui().cx(), selection);
        self.app.settle();
        self
    }

    pub fn open(&mut self, request: DialogRequest) -> &mut Self {
        let store = self.store;
        store.open_dialog(self.app.ui().cx(), request);
        self.app.settle();
        self
    }

    pub fn set_inventory(&mut self, inventory: Inventory) -> &mut Self {
        self.apply(Event::Inventory(Arc::new(inventory)))
    }

    /// Clicks the button named `name`; when the toolbar repeats a sheet's button, the sheet's
    /// one, which is drawn last, is pressed.
    pub fn click_button(&mut self, name: &str) -> &mut Self {
        let tree = self.app.ui().semantic_tree();
        let topmost = role(Role::Button)
            .name(name)
            .find_all(tree)
            .last()
            .map(|node| node.bounds.center());
        match topmost {
            Some(center) => {
                self.app.pointer_move(center.x, center.y);
                self.app.pointer_down(center.x, center.y);
                self.app.pointer_up(center.x, center.y);
            }
            None => _ = self.app.click(role(Role::Button).name(name)),
        }
        self.park_pointer()
    }

    /// Moves the pointer to an empty corner so no hover highlight is captured.
    pub fn park_pointer(&mut self) -> &mut Self {
        self.app.pointer_move(PARKED_POINTER.0, PARKED_POINTER.1);
        self.app.settle();
        self
    }

    pub fn click_menu_item(&mut self, name: &str) -> &mut Self {
        self.app.click(role(Role::MenuItem).name(name));
        self.park_pointer()
    }

    pub fn click_sidebar(&mut self, name: &str) -> &mut Self {
        self.app.click(role(Role::MenuItem).name(name));
        self.park_pointer()
    }

    /// Types into `field` as committed keyboard input, which secret fields accept.
    pub fn type_into(&mut self, field: &str, text: &str) -> &mut Self {
        self.app.click(role(Role::TextField).name(field));
        let event = WindowEvent::TextInput(TextInputEvent::Commit(text.to_owned()));
        self.app.deliver(event).settle();
        self
    }

    pub fn right_click(&mut self, query: Query) -> &mut Self {
        let center = self.app.find(query).bounds.center();
        self.right_click_at(center)
    }

    /// Right-clicks the first node matching `query`, for labels that repeat.
    pub fn right_click_first(&mut self, query: Query) -> Result<&mut Self> {
        let tree = self.app.ui().semantic_tree();
        let center = query
            .find_all(tree)
            .first()
            .map(|node| node.bounds.center())
            .context("nothing to right-click")?;
        Ok(self.right_click_at(center))
    }

    fn right_click_at(&mut self, center: Point) -> &mut Self {
        let press = PointerEvent {
            button: Some(PointerButton::Secondary),
            timestamp: self.app.inline().now(),
            ..PointerEvent::mouse(center)
        };
        self.app.pointer_move(center.x, center.y);
        self.app.deliver(WindowEvent::PointerDown(press));
        self.app.deliver(WindowEvent::PointerUp(press));
        self.app.settle();
        self
    }

    pub fn scroll_page(&mut self, page: &str, dy: f32) -> &mut Self {
        self.app.scroll(role(Role::ScrollView).name(page), dy);
        self.app.settle();
        self
    }
}
