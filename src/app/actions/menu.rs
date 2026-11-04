use crownui::kit::{
    MenuRowOptions, context_menu_at, menu_item, menu_item_disabled, menu_separator,
};
use crownui::prelude::{AnyView, Cx, Listeners, Point, Signal, Styled, View, keyed, vstack};

use super::offer::{Offer, offers};
use super::perform::perform;
use crate::state::{Selection, Store};

#[derive(Clone, Copy)]
pub struct ContextMenu {
    position: Signal<Option<Point>>,
    pointer: Signal<Point>,
    target: Signal<Selection>,
}

impl ContextMenu {
    pub fn new(cx: &mut Cx) -> Self {
        Self {
            position: cx.signal(None),
            pointer: cx.signal(Point::ZERO),
            target: cx.signal(Selection::Overview),
        }
    }

    pub fn track_pointer(self, cx: &mut Cx, position: Point) {
        self.pointer.set(cx, position);
    }

    pub fn open(self, cx: &mut Cx, target: Selection) {
        let at = self.pointer.get(cx);
        self.target.set(cx, target);
        self.position.set(cx, Some(at));
    }
}

pub fn context_menu_layer(store: Store, menu: ContextMenu) -> impl View {
    context_menu_at(menu.position, move || {
        keyed(
            move |runtime| {
                let target = menu.target.get(runtime);
                runtime.with2(
                    store.inventory,
                    store.operations,
                    |inventory, operations| offers(inventory, operations, &target),
                )
            },
            move |_, offers: Vec<Offer>| vstack(menu_rows(store, offers)).items_stretch(),
        )
        .flex_col()
        .items_stretch()
    })
}

fn menu_rows(store: Store, offers: Vec<Offer>) -> Vec<AnyView> {
    let mut rows = Vec::with_capacity(offers.len() * 2);
    let mut placement = None;
    for Offer {
        action,
        selection,
        refusal,
    } in offers
    {
        if placement.is_some_and(|previous| previous != action.placement()) {
            rows.push(menu_separator().boxed());
        }
        placement = Some(action.placement());
        rows.push(match refusal {
            Some(_) => menu_item_disabled(action.label())
                .leading_icon(action.icon())
                .boxed(),
            None => menu_item(action.label())
                .leading_icon(action.icon())
                .on_click(move |cx, _| perform(cx, store, &selection, action))
                .boxed(),
        });
    }
    rows
}
