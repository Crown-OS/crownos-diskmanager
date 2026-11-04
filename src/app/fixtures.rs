use std::sync::Arc;

use crownos_disks::model::Inventory;
use crownos_disks::{Event, UserError};
use crownui::prelude::Cx;

use crate::state::Store;

pub fn populate(cx: &mut Cx, store: Store, inventory: Inventory) {
    store.apply(cx, Event::Inventory(Arc::new(inventory)));
}

pub fn lose_service(cx: &mut Cx, store: Store) {
    let error = UserError::new(
        "Storage service unavailable",
        "UDisks2 stopped answering. Start udisks2.service and try again.",
    );
    store.apply(cx, Event::ServiceUnavailable(error));
}
