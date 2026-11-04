use crownui::kit::{
    ButtonOptions, ButtonSize, ButtonVariant, button, popup_menu, popup_menu_item, tooltip,
};
use crownui::prelude::{AnyView, Listeners, Semantic, View};

use super::offer::Offer;
use super::perform::perform;
use crate::state::Store;

pub fn action_button(store: Store, offer: Offer) -> AnyView {
    let Offer {
        action,
        selection,
        refusal,
    } = offer;
    let control = button(action.label())
        .leading_icon(action.icon())
        .variant(ButtonVariant::Ghost)
        .control_size(ButtonSize::Small)
        .disabled(refusal.is_some())
        .on_click(move |cx, _| perform(cx, store, &selection, action));
    match refusal {
        Some(refusal) => {
            let reason = refusal.to_string();
            tooltip(reason.clone(), control.description(reason)).boxed()
        }
        None => control.boxed(),
    }
}

pub fn offer_menu(store: Store, icon: &'static str, label: &str, offers: Vec<Offer>) -> AnyView {
    let entries = offers.into_iter().map(
        |Offer {
             action,
             selection,
             refusal,
         }| {
            popup_menu_item(action.label(), move |cx| {
                perform(cx, store, &selection, action)
            })
            .disabled(refusal.is_some())
        },
    );
    popup_menu(icon, label, entries).boxed()
}
