use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::model::BlockId;
use crownui::kit::{
    ButtonOptions, ButtonVariant, CalloutOptions, TextInputOptions, Tone, button, callout, spinner,
    text_input,
};
use crownui::prelude::{
    AnyView, Cx, Listeners, Runtime, Semantic, Styled, View, bind, component, hstack, keyed, show,
};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::take_secret;
use crate::dialogs::sheets::gone_sheet;
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form_dialog, muted_text, sheet_title,
};
use crate::dialogs::target::Resolved;
use crate::util::nothing::nothing;

const GAP: f32 = 8.0;

pub fn unlock_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Block(block.clone());
        let Some(resolved) = Resolved::find(&inventory, &target) else {
            return gone_sheet(context, "The volume".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let open = context.sheet_open(cx);
        let password = cx.signal(String::new());
        context.inline_error.set(cx, None);
        cx.on_cleanup(move |runtime| {
            context.awaiting.set(runtime, None);
            context.inline_error.set(runtime, None);
        });
        let waiting = {
            let target = target.clone();
            move |runtime: &mut Runtime| {
                context
                    .awaiting
                    .with(runtime, |awaiting| awaiting.as_ref() == Some(&target))
            }
        };
        let blocked = {
            let waiting = waiting.clone();
            move |runtime: &mut Runtime| {
                waiting(runtime) || password.with(runtime, String::is_empty)
            }
        };
        let unlock = {
            let blocked = blocked.clone();
            move |cx: &mut Cx| {
                if blocked(cx) {
                    return;
                }
                let passphrase = take_secret(cx, password);
                context.send_and_wait(
                    cx,
                    target.clone(),
                    Command::Unlock {
                        block: block.clone(),
                        passphrase,
                    },
                );
            }
        };
        form_dialog(open, move || {
            let (submit, click, blocked, waiting) = (
                unlock.clone(),
                unlock.clone(),
                blocked.clone(),
                waiting.clone(),
            );
            (
                sheet_title(format!("Unlock “{name}”")),
                muted_text(format!("Enter the password for {name}.")),
                text_input(password)
                    .secret()
                    .placeholder("Password")
                    .label("Password")
                    .stretch()
                    .on_submit(move |cx, _| submit(cx)),
                keyed(
                    move |runtime| context.inline_error.get(runtime),
                    |_, error| match error {
                        Some(error) => callout(Tone::Danger, error.title)
                            .body(error.message)
                            .boxed(),
                        None => nothing().boxed(),
                    },
                ),
                show(
                    bind(waiting),
                    || {
                        hstack((spinner(), muted_text("Unlocking…")))
                            .gap(GAP)
                            .items_center()
                    },
                    nothing,
                ),
                actions((
                    cancel_button(open),
                    button("Unlock")
                        .variant(ButtonVariant::Primary)
                        .disabled(bind(blocked))
                        .on_click(move |cx, _| click(cx)),
                )),
            )
        })
        .boxed()
    })
    .boxed()
}
