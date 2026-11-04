use crownui::kit::{icons, title_bar_spacer, window_controls};
use crownui::prelude::{AnyView, Styled, View, ViewSeq, hstack, keyed};

use super::actions::{DiskAction, Offer, Placement, action_button, offer_menu, offers};
use crate::state::{Selection, Store};

const TOOLBAR_GAP: f32 = 4.0;
const IMAGE_MENU_LABEL: &str = "Image";
const MORE_MENU_LABEL: &str = "More Actions";

pub fn toolbar(store: Store) -> impl ViewSeq {
    let open_image = Offer {
        action: DiskAction::OpenImage,
        selection: Selection::Overview,
        refusal: None,
    };
    (
        contextual_actions(store),
        title_bar_spacer(),
        action_button(store, open_image),
        window_controls(),
    )
}

fn contextual_actions(store: Store) -> impl View {
    keyed(
        move |runtime| {
            let selection = store.selection.get(runtime);
            runtime.with2(
                store.inventory,
                store.operations,
                |inventory, operations| offers(inventory, operations, &selection),
            )
        },
        move |_, offers: Vec<Offer>| {
            hstack(toolbar_items(store, offers))
                .gap(TOOLBAR_GAP)
                .items_center()
        },
    )
}

fn toolbar_items(store: Store, offers: Vec<Offer>) -> Vec<AnyView> {
    let (mut images, mut more, mut buttons) = (Vec::new(), Vec::new(), Vec::new());
    for offer in offers {
        match offer.action.placement() {
            Placement::Toolbar => buttons.push(action_button(store, offer)),
            Placement::ImageMenu => images.push(offer),
            Placement::MoreMenu => more.push(offer),
        }
    }
    if !images.is_empty() {
        buttons.push(offer_menu(
            store,
            icons::HARD_DRIVE_DOWNLOAD,
            IMAGE_MENU_LABEL,
            images,
        ));
    }
    if !more.is_empty() {
        buttons.push(offer_menu(store, icons::ELLIPSIS, MORE_MENU_LABEL, more));
    }
    buttons
}
