use crownui::kit::{provide_configured_theme, provide_toasts};
use crownui::prelude::{View, component};

use super::shell::shell;
use crate::state::Store;
use crate::util::worker::spawn_worker;

pub fn root() -> impl View {
    component(|cx| {
        provide_configured_theme(cx);
        provide_toasts(cx);
        let store = Store::new(cx);
        let commands = spawn_worker(
            cx,
            |events, commands| crownos_disks::run(move |event| _ = events.message(event), commands),
            move |cx, event| store.apply(cx, event),
        );
        store.connect(cx, commands);
        shell(cx, store)
    })
}
