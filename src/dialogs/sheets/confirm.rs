use crownos_disks::guard::{ConfirmationTier, Loss};
use crownui::ext::{Key, NamedKey};
use crownui::kit::{
    ButtonOptions, ButtonVariant, CalloutOptions, DialogOptions, TextInputOptions, Tone, button,
    callout, dialog, text_input,
};
use crownui::prelude::{
    AnyView, Cx, Listeners, Semantic, Signal, Styled, View, ViewSeq, bind, component, text, vstack,
};

use super::layout::{actions, muted_text, sheet_title};

const LIST_GAP: f32 = 4.0;
const IRREVERSIBLE: &str = "This can't be undone.";

/// What a confirmation asks, at the strength `tier` demands.
#[derive(Debug, Clone)]
pub struct Confirmation {
    pub tier: ConfirmationTier,
    pub title: String,
    pub message: String,
    pub action: String,
    pub target_name: String,
    pub losses: Vec<Loss>,
    pub notes: Vec<&'static str>,
}

impl Confirmation {
    pub const fn is_destructive(&self) -> bool {
        matches!(
            self.tier,
            ConfirmationTier::Destructive | ConfirmationTier::TypeName
        )
    }
}

pub fn loss_line(loss: &Loss) -> String {
    let filesystem = loss
        .filesystem
        .as_ref()
        .map(|filesystem| filesystem.short_name().to_owned());
    let used = loss.used.map(|used| format!("{used} used"));
    let parts: Vec<String> = [Some(loss.name.clone()), filesystem, used]
        .into_iter()
        .flatten()
        .collect();
    format!("• {}", parts.join(" · "))
}

/// A confirmation sheet: a plain question for [`ConfirmationTier::Simple`], a danger notice
/// listing what is lost for the destructive tiers, plus typing the name for
/// [`ConfirmationTier::TypeName`]. Destructive sheets start on Cancel, ignore Enter on the
/// confirm button and stay open on a scrim press.
pub fn confirm_sheet(
    open: Signal<bool>,
    confirmation: Confirmation,
    on_confirm: impl FnMut(&mut Cx) + Clone + 'static,
) -> impl View {
    let dismiss_on_scrim = !confirmation.is_destructive();
    component(move |cx| {
        let typed = cx.signal(String::new());
        dialog(open, move || {
            content(open, confirmation.clone(), typed, on_confirm.clone())
        })
        .dismiss_on_scrim(dismiss_on_scrim)
    })
}

fn content(
    open: Signal<bool>,
    confirmation: Confirmation,
    typed: Signal<String>,
    mut on_confirm: impl FnMut(&mut Cx) + 'static,
) -> impl ViewSeq {
    let destructive = confirmation.is_destructive();
    let needs_name = confirmation.tier == ConfirmationTier::TypeName;
    let name = confirmation.target_name.clone();
    let confirm = button(confirmation.action.clone())
        .variant(if destructive {
            ButtonVariant::Destructive
        } else {
            ButtonVariant::Primary
        })
        .disabled(bind(move |runtime| {
            needs_name && typed.with(runtime, |typed| *typed != name)
        }))
        .on_key_down(move |_, event| {
            if destructive && event.key == Key::Named(NamedKey::Enter) {
                event.prevent_default();
                event.stop_propagation();
            }
        })
        .on_click(move |cx, _| on_confirm(cx));
    (
        sheet_title(confirmation.title.clone()),
        body(&confirmation),
        needs_name.then(|| type_name_field(&confirmation.target_name, typed)),
        actions((
            button("Cancel")
                .variant(ButtonVariant::Secondary)
                .on_click(move |cx, _| open.set(cx, false)),
            confirm,
        )),
    )
}

fn body(confirmation: &Confirmation) -> AnyView {
    let notes: Vec<AnyView> = confirmation
        .notes
        .iter()
        .map(|note| callout(Tone::Warning, *note).boxed())
        .collect();
    if !confirmation.is_destructive() {
        return vstack((muted_text(confirmation.message.clone()), notes))
            .gap(LIST_GAP * 2.0)
            .boxed();
    }
    let losses: Vec<AnyView> = confirmation
        .losses
        .iter()
        .map(|loss| text(loss_line(loss)).boxed())
        .collect();
    vstack((
        callout(Tone::Danger, confirmation.message.clone()).body(IRREVERSIBLE),
        vstack(losses).gap(LIST_GAP).label("Will be lost"),
        notes,
    ))
    .gap(LIST_GAP * 2.0)
    .boxed()
}

fn type_name_field(name: &str, typed: Signal<String>) -> impl View + use<> {
    vstack((
        text(format!("Type “{name}” to confirm.")),
        text_input(typed)
            .placeholder(name.to_owned())
            .label("Name to confirm")
            .stretch(),
    ))
    .gap(LIST_GAP * 2.0)
}
