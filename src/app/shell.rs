use crownui::kit::{INTER_FAMILY, sidebar_with_header, toast_host, use_theme};
use crownui::prelude::{Cx, Listeners, Styled, View, keyed, vstack, zstack};

use super::actions::{ContextMenu, context_menu_layer};
use super::close_guard::guard_close;
use super::selection_guard::keep_selection_valid;
use super::service_unavailable::service_unavailable;
use super::toolbar::toolbar;
use crate::dialogs;
use crate::pages;
use crate::sidebar::sidebar_menus;
use crate::state::Store;
use crate::util::tint::tint;

pub fn shell(cx: &mut Cx, store: Store) -> impl View + use<> {
    let theme = use_theme(cx);
    let menu = ContextMenu::new(cx);
    keep_selection_valid(cx, store);
    guard_close(cx, store);
    let content = keyed(
        move |runtime| store.service_error.get(runtime),
        move |_, error| match error {
            Some(error) => service_unavailable(error).boxed(),
            None => pages::content(store, menu).boxed(),
        },
    )
    .flex_1()
    .flex_col()
    .min_h(0.0);
    let backdrop = vstack(())
        .absolute()
        .inset(0.0)
        .bg(tint(theme, |theme| theme.surface.bg));
    toast_host(
        zstack((
            backdrop,
            sidebar_with_header(sidebar_menus(cx, store, menu), toolbar(store), content),
            context_menu_layer(store, menu),
            dialogs::host(store),
        ))
        .w_full()
        .h_full()
        .on_pointer_down(move |cx, event| menu.track_pointer(cx, event.position)),
    )
    .font_family(INTER_FAMILY)
    .w_full()
    .h_full()
}
