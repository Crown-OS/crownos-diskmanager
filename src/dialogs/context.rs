use crownos_disks::UserError;
use crownos_disks::command::{Command, Target};
use crownui::prelude::{Cx, Runtime, Signal};

use crate::state::Store;

/// What every sheet shares: the store, the error sheet, the progress hand-off and the result a
/// sheet waits for.
#[derive(Clone, Copy)]
pub struct SheetContext {
    pub store: Store,
    pub error: Signal<Option<UserError>>,
    pub awaiting: Signal<Option<Target>>,
    pub inline_error: Signal<Option<UserError>>,
    pub launched: Signal<Vec<Target>>,
}

impl SheetContext {
    pub fn new(cx: &mut Cx, store: Store) -> Self {
        Self {
            store,
            error: cx.signal(None),
            awaiting: cx.signal(None),
            inline_error: cx.signal(None),
            launched: cx.signal(Vec::new()),
        }
    }

    pub fn close(self, runtime: &mut Runtime) {
        self.store.dialog.set(runtime, None);
    }

    /// Hands a long operation over to the progress sheet and closes the current sheet.
    pub fn launch(self, runtime: &mut Runtime, target: Target, command: Command) {
        self.launched
            .update(runtime, |launched| launched.push(target));
        self.store.send(runtime, command);
        self.close(runtime);
    }

    /// Sends `command` and keeps the sheet open until its result arrives.
    pub fn send_and_wait(self, runtime: &mut Runtime, target: Target, command: Command) {
        self.inline_error.set(runtime, None);
        self.awaiting.set(runtime, Some(target));
        self.store.send(runtime, command);
    }

    pub fn report(self, runtime: &mut Runtime, error: UserError) {
        self.error.set(runtime, Some(error));
    }

    /// The `open` signal of a request's sheets: closing any of them ends the request.
    pub fn sheet_open(self, cx: &mut Cx) -> Signal<bool> {
        let open = cx.signal(true);
        cx.effect(move |runtime| {
            if !open.get(runtime) {
                self.close(runtime);
            }
        });
        open
    }
}
