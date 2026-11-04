use std::sync::Arc;

use crownos_disks::Command;
use crownos_disks::command::Target;
use crownos_disks::guard::{self, Action};
use crownos_disks::model::ByteSize;
use crownui::kit::{
    ButtonOptions, ButtonVariant, CalloutOptions, ToggleOptions, Tone, button, callout, toggle,
};
use crownui::prelude::{
    AnyView, Cx, Listeners, Semantic, Signal, Styled, View, bind, component, hstack, keyed, vstack,
};

use super::image_file::{ChosenImage, pick_image};
use crate::dialogs::context::SheetContext;
use crate::dialogs::sheets::layout::{
    actions, cancel_button, form, form_dialog, labeled, muted_text, sheet_title,
};
use crate::dialogs::sheets::{Confirmation, Step, confirm_sheet, gone_sheet, proceed, steps};
use crate::dialogs::target::Resolved;
use crate::util::portal::FileChoice;

const GAP: f32 = 8.0;

pub fn image_summary(image: &ChosenImage, name: &str, capacity: ByteSize) -> (Tone, String) {
    let file = image.file_name();
    match (image.compressed, image.size) {
        (true, _) => (
            Tone::Neutral,
            format!("{file} is compressed, so its size is checked while writing."),
        ),
        (false, None) => (
            Tone::Neutral,
            format!("{file} will be checked while writing."),
        ),
        (false, Some(size)) if size <= capacity => (
            Tone::Neutral,
            format!("{file} · {size}, fits on {name} ({capacity})."),
        ),
        (false, Some(size)) => (
            Tone::Danger,
            format!("{file} is {size}, but {name} holds only {capacity}."),
        ),
    }
}

fn image_status(image: Signal<Option<ChosenImage>>, name: String, capacity: ByteSize) -> impl View {
    keyed(
        move |runtime| image.get(runtime),
        move |_, chosen| match chosen {
            None => muted_text("No image chosen").boxed(),
            Some(chosen) => match image_summary(&chosen, &name, capacity) {
                (Tone::Danger, message) => callout(Tone::Danger, "This image is too large")
                    .body(message)
                    .boxed(),
                (_, message) => muted_text(message).boxed(),
            },
        },
    )
}

pub fn restore_image_sheet(context: SheetContext, target: Target) -> AnyView {
    component(move |cx| {
        let inventory = context.store.inventory.with_untracked(cx, Arc::clone);
        let (Some(resolved), Some(expected)) = (
            Resolved::find(&inventory, &target),
            guard::fingerprint(&inventory, &target),
        ) else {
            return gone_sheet(context, "The disk".to_owned()).boxed();
        };
        let name = resolved.name().into_owned();
        let capacity = resolved.size();
        let open = context.sheet_open(cx);
        let step = cx.signal(Step::Form);
        let image: Signal<Option<ChosenImage>> = cx.signal(None);
        let choosing = cx.signal(false);
        let verify = cx.signal(true);
        let tier = guard::confirmation_tier(&inventory, &target, Action::RestoreImage);
        let losses = guard::losses(&inventory, &target);

        let choose = move |cx: &mut Cx| {
            choosing.set(cx, true);
            pick_image(
                cx,
                context,
                "Choose a Disk Image",
                FileChoice::Open,
                move |cx, path| {
                    choosing.set(cx, false);
                    if let Some(path) = path {
                        image.set(cx, Some(ChosenImage::inspect(path)));
                    }
                },
            );
        };
        let submit = move |cx: &mut Cx| {
            let Some(chosen) = image.get(cx) else {
                return;
            };
            let verify = verify.get(cx);
            let command = Command::RestoreImage {
                image: chosen.path,
                target: target.clone(),
                verify,
                expected: expected.clone(),
            };
            context.launch(cx, target.clone(), command);
        };
        let form_sheet = {
            let (name, submit) = (name.clone(), submit.clone());
            move || {
                let (name, submit) = (name.clone(), submit.clone());
                form_dialog(open, move || {
                    let (name, submit) = (name.clone(), submit.clone());
                    (
                        sheet_title(format!("Restore Image to “{name}”")),
                        muted_text(format!("The image replaces everything on {name}.")),
                        form((
                            labeled(
                                "Image",
                                hstack((
                                    vstack(image_status(image, name, capacity)).grow(1.0),
                                    button("Choose…")
                                        .disabled(choosing)
                                        .on_click(move |cx, _| choose(cx)),
                                ))
                                .gap(GAP)
                                .items_center(),
                            ),
                            toggle(verify).caption("Verify after writing"),
                        )),
                        actions((
                            cancel_button(open),
                            button("Restore…")
                                .variant(ButtonVariant::Primary)
                                .disabled(bind(move |runtime| {
                                    !image.with(runtime, |image| {
                                        image.as_ref().is_some_and(|image| image.fits(capacity))
                                    })
                                }))
                                .on_click(move |cx, _| proceed(cx, tier, step, submit.clone())),
                        )),
                    )
                })
            }
        };
        let confirm = move |cx: &mut Cx| {
            let file = image.with(cx, |image| {
                image
                    .as_ref()
                    .map(ChosenImage::file_name)
                    .unwrap_or_default()
            });
            let confirmation = Confirmation {
                tier,
                title: format!("Restore “{file}” to “{name}”?"),
                message: format!(
                    "Everything on {name} will be replaced by the contents of {file}."
                ),
                action: format!("Restore to {name}"),
                target_name: name.clone(),
                losses: losses.clone(),
                notes: Vec::new(),
            };
            confirm_sheet(open, confirmation, submit.clone())
        };
        steps(step, form_sheet, confirm).boxed()
    })
    .boxed()
}
