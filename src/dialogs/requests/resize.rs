use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::{self, ConfirmationTier, Refusal};
use crownos_disks::limits::{self, ResizeBounds};
use crownos_disks::model::{BlockId, ByteSize, Content, Encryption, LuksVersion};
use crownui::kit::{
    ButtonOptions, ButtonVariant, CalloutOptions, TextInputOptions, Tone, button, callout,
    text_input,
};
use crownui::prelude::{AnyView, Cx, Listeners, Runtime, Semantic, Signal, View, bind, component};

use crate::dialogs::context::SheetContext;
use crate::dialogs::fields::{ByteSizeField, take_secret};
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form, form_dialog, labeled, muted_text, sheet_title,
};
use crate::dialogs::sheets::{Confirmation, Step, confirm_sheet, gone_sheet, proceed, steps};
use crate::dialogs::target::Resolved;
use crate::state::DialogRequest;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encrypted {
    No,
    Locked,
    NeedsPassphrase,
}

impl Encrypted {
    fn of(content: Option<&Content>) -> Self {
        match content {
            Some(Content::Encrypted(Encryption { unlocked: None, .. })) => Self::Locked,
            Some(Content::Encrypted(Encryption {
                version: LuksVersion::Luks2,
                ..
            })) => Self::NeedsPassphrase,
            _ => Self::No,
        }
    }
}

pub fn summary(name: &str, current: ByteSize, requested: ByteSize) -> String {
    match requested.cmp(&current) {
        std::cmp::Ordering::Greater => {
            format!(
                "{name} will grow by {} to {requested}.",
                ByteSize(requested.bytes() - current.bytes())
            )
        }
        std::cmp::Ordering::Less => format!(
            "{name} will shrink by {} to {requested}. Its files are checked first.",
            current.saturating_sub(requested)
        ),
        std::cmp::Ordering::Equal => "Drag the slider or type a new size.".to_owned(),
    }
}

pub fn resize_sheet(context: SheetContext, block: BlockId) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let target = Target::Block(block.clone());
        let (Some(resolved), Some(expected)) =
            (Resolved::find(&inventory, &target), guard::fingerprint(&inventory, &target))
        else {
            return gone_sheet(context, "The volume".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let open = context.sheet_open(cx);
        let encrypted = Encrypted::of(resolved.content());
        if encrypted == Encrypted::Locked {
            return locked_sheet(context, open, name, block).boxed();
        }
        let bounds = match limits::resize_bounds(&inventory, &block) {
            Ok(bounds) => bounds,
            Err(refusal) => return refused_sheet(open, name, refusal).boxed(),
        };
        let step = cx.signal(Step::Form);
        let size = ByteSizeField::new(cx, bounds.current, bounds.min, bounds.max);
        let passphrase = cx.signal(String::new());
        let ready = move |runtime: &mut Runtime| {
            size.size.get(runtime) != bounds.current
                && (encrypted != Encrypted::NeedsPassphrase || passphrase.with(runtime, |text| !text.is_empty()))
        };

        let submit = move |cx: &mut Cx| {
            let requested = size.size.get(cx);
            let passphrase = (encrypted == Encrypted::NeedsPassphrase).then(|| take_secret(cx, passphrase));
            let command = Command::Resize { block: block.clone(), size: requested, passphrase, expected: expected.clone() };
            context.launch(cx, target.clone(), command);
        };
        let form_sheet = {
            let (name, submit) = (name.clone(), submit.clone());
            move || {
                let (name, submit) = (name.clone(), submit.clone());
                form_dialog(open, move || {
                    let (name, submit) = (name.clone(), submit.clone());
                    (
                        sheet_title(format!("Resize “{name}”")),
                        resize_form(name, bounds, size, (encrypted == Encrypted::NeedsPassphrase).then_some(passphrase)),
                        actions((
                            cancel_button(open),
                            button("Resize")
                                .variant(ButtonVariant::Primary)
                                .disabled(bind(move |runtime| !ready(runtime)))
                                .on_click(move |cx, _| {
                                    let tier = guard::resize_confirmation_tier(bounds.current, size.size.get(cx));
                                    proceed(cx, tier, step, submit.clone());
                                }),
                        )),
                    )
                })
            }
        };
        let confirm = move |cx: &mut Cx| {
            let requested = size.size.get(cx);
            let confirmation = Confirmation {
                tier: ConfirmationTier::Simple,
                title: format!("Shrink “{name}”?"),
                message: format!(
                    "{name} will shrink from {} to {requested}. Back up important files before shrinking.",
                    bounds.current
                ),
                action: "Shrink".to_owned(),
                target_name: name.clone(),
                losses: Vec::new(),
                notes: Vec::new(),
            };
            confirm_sheet(open, confirmation, submit.clone())
        };
        steps(step, form_sheet, confirm).boxed()
    })
    .boxed()
}

fn resize_form(
    name: String,
    bounds: ResizeBounds,
    size: ByteSizeField,
    passphrase: Option<Signal<String>>,
) -> impl View {
    form((
        labeled("New size", size.view()),
        muted_text(format!("Currently {}", bounds.current)),
        muted_text(bind(move |runtime| {
            summary(&name, bounds.current, size.size.get(runtime))
        })),
        passphrase.map(|passphrase| {
            labeled(
                "Encryption password",
                text_input(passphrase)
                    .secret()
                    .placeholder("Password")
                    .label("Encryption password")
                    .stretch(),
            )
        }),
    ))
}

fn locked_sheet(
    context: SheetContext,
    open: Signal<bool>,
    name: String,
    block: BlockId,
) -> impl View {
    form_dialog(open, move || {
        let block = block.clone();
        (
            sheet_title(format!("Resize “{name}”")),
            callout(Tone::Warning, "Unlock first").body(format!(
                "{name} is encrypted. Unlock it to change its size."
            )),
            actions((
                cancel_button(open),
                button("Unlock…")
                    .variant(ButtonVariant::Primary)
                    .on_click(move |cx, _| {
                        context
                            .store
                            .open_dialog(cx, DialogRequest::Unlock(block.clone()));
                    }),
            )),
        )
    })
}

fn refused_sheet(open: Signal<bool>, name: String, refusal: Refusal) -> impl View {
    form_dialog(open, move || {
        (
            sheet_title(format!("Resize “{name}”")),
            callout(Tone::Warning, "Can't resize").body(refusal.to_string()),
            actions((button("OK")
                .variant(ButtonVariant::Primary)
                .on_click(move |cx, _| open.set(cx, false)),)),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const GB: u64 = 1_000_000_000;

    #[test]
    fn the_summary_names_the_direction_and_amount() {
        let current = ByteSize(100 * GB);
        assert_eq!(
            summary("Root", current, ByteSize(150 * GB)),
            "Root will grow by 50.0 GB to 150 GB."
        );
        assert_eq!(
            summary("Root", current, ByteSize(80 * GB)),
            "Root will shrink by 20.0 GB to 80.0 GB. Its files are checked first."
        );
        assert_eq!(
            summary("Root", current, current),
            "Drag the slider or type a new size."
        );
    }
}
