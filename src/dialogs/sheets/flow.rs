use crownos_disks::guard::ConfirmationTier;
use crownui::prelude::{Cx, Signal, View, keyed};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Form,
    Confirm,
}

/// Shows `form` until `step` moves on, then the confirmation `confirm` builds.
pub fn steps<V, W>(
    step: Signal<Step>,
    mut form: impl FnMut() -> V + 'static,
    mut confirm: impl FnMut(&mut Cx) -> W + 'static,
) -> impl View
where
    V: View,
    W: View,
{
    keyed(
        move |runtime| step.get(runtime),
        move |cx, step| match step {
            Step::Form => form().boxed(),
            Step::Confirm => confirm(cx).boxed(),
        },
    )
}

/// Leaves the form: straight to `submit` when no confirmation is needed, otherwise to the
/// confirmation step.
pub fn proceed(
    cx: &mut Cx,
    tier: ConfirmationTier,
    step: Signal<Step>,
    mut submit: impl FnMut(&mut Cx),
) {
    match tier {
        ConfirmationTier::None => submit(cx),
        _ => step.set(cx, Step::Confirm),
    }
}
