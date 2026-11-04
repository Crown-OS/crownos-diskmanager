use crownos_disks::command::Secret;
use crownui::kit::{
    StatusOptions, TextInputOptions, ToggleOptions, Tone, status, text_input, toggle,
};
use crownui::prelude::{Cx, Memo, Runtime, Signal, Styled, View, bind, keyed, show, vstack};

use super::take_secret;
use crate::util::nothing::nothing;
use crate::util::password_strength::{PasswordStrength, password_strength};

const GAP: f32 = 8.0;
pub const FORGOTTEN_PASSWORD_WARNING: &str =
    "If you forget this password, the data can't be recovered.";

const fn strength_tone(strength: PasswordStrength) -> Tone {
    match strength {
        PasswordStrength::Weak => Tone::Danger,
        PasswordStrength::Fair => Tone::Warning,
        PasswordStrength::Strong => Tone::Success,
    }
}

/// The "Encrypt with password" switch and its two password fields.
#[derive(Clone, Copy)]
pub struct EncryptionFields {
    pub enabled: Signal<bool>,
    pub password: Signal<String>,
    pub confirmation: Signal<String>,
    offered: Memo<bool>,
}

impl EncryptionFields {
    pub fn new(cx: &mut Cx, offered: impl FnMut(&mut Runtime) -> bool + 'static) -> Self {
        Self {
            enabled: cx.signal(false),
            password: cx.signal(String::new()),
            confirmation: cx.signal(String::new()),
            offered: cx.memo(offered),
        }
    }

    pub fn is_active(self, runtime: &mut Runtime) -> bool {
        self.offered.get(runtime) && self.enabled.get(runtime)
    }

    pub fn is_ready(self, runtime: &mut Runtime) -> bool {
        !self.is_active(runtime)
            || runtime.with2(
                self.password,
                self.confirmation,
                |password, confirmation| !password.is_empty() && password == confirmation,
            )
    }

    /// Moves the password out of its fields, leaving both empty.
    pub fn take_secret(self, runtime: &mut Runtime) -> Option<Secret> {
        let active = self.is_active(runtime);
        let password = take_secret(runtime, self.password);
        drop(take_secret(runtime, self.confirmation));
        active.then_some(password)
    }

    pub fn view(self) -> impl View {
        let Self {
            enabled,
            password,
            confirmation,
            offered,
        } = self;
        show(
            offered,
            move || {
                vstack((
                    toggle(enabled).caption("Encrypt with password"),
                    show(
                        enabled,
                        move || password_fields(password, confirmation),
                        nothing,
                    ),
                ))
                .gap(GAP)
            },
            nothing,
        )
    }
}

fn password_fields(password: Signal<String>, confirmation: Signal<String>) -> impl View {
    let mismatch = bind(move |runtime| {
        runtime.with2(password, confirmation, |password, confirmation| {
            !confirmation.is_empty() && password != confirmation
        })
    });
    vstack((
        text_input(password)
            .secret()
            .placeholder("Password")
            .stretch(),
        text_input(confirmation)
            .secret()
            .placeholder("Repeat password")
            .stretch(),
        keyed(
            move |runtime| {
                password.with(runtime, |password| {
                    (!password.is_empty()).then(|| password_strength(password))
                })
            },
            |_, strength| match strength {
                Some(strength) => status(strength.label())
                    .tone(strength_tone(strength))
                    .boxed(),
                None => nothing().boxed(),
            },
        ),
        show(
            mismatch,
            || status("Passwords don't match").tone(Tone::Danger),
            nothing,
        ),
        status(FORGOTTEN_PASSWORD_WARNING),
    ))
    .gap(GAP)
}
