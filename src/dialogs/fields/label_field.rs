use crownos_disks::limits::validate_label;
use crownos_disks::model::FilesystemKind;
use crownui::kit::{StatusOptions, TextInputKind, TextInputOptions, Tone, status, text_input};
use crownui::prelude::{
    Cx, Element, Listeners, Memo, Runtime, Semantic, Signal, Styled, View, bind, show, vstack,
};

use crate::util::nothing::nothing;

const GAP: f32 = 6.0;

/// A volume name, checked live against the rules of the filesystem it is for.
#[derive(Clone, Copy)]
pub struct LabelField {
    pub value: Signal<String>,
    problem: Memo<Option<String>>,
}

impl LabelField {
    pub fn new(
        cx: &mut Cx,
        initial: String,
        mut filesystem: impl FnMut(&mut Runtime) -> FilesystemKind + 'static,
    ) -> Self {
        let value = cx.signal(initial);
        let problem = cx.memo(move |runtime| {
            let filesystem = filesystem(runtime);
            value.with(runtime, |label| {
                validate_label(&filesystem, label)
                    .err()
                    .map(|error| error.to_string())
            })
        });
        Self { value, problem }
    }

    pub fn is_valid(self, runtime: &mut Runtime) -> bool {
        self.problem.with(runtime, Option::is_none)
    }

    fn input(self, placeholder: &'static str) -> Element<TextInputKind> {
        text_input(self.value)
            .placeholder(placeholder)
            .label("Name")
            .stretch()
    }

    pub fn view(self, placeholder: &'static str) -> impl View {
        self.layout(self.input(placeholder))
    }

    /// The field, running `on_submit` when Enter is pressed in it.
    pub fn view_with_submit(
        self,
        placeholder: &'static str,
        mut on_submit: impl FnMut(&mut Cx) + 'static,
    ) -> impl View {
        self.layout(
            self.input(placeholder)
                .on_submit(move |cx, _| on_submit(cx)),
        )
    }

    fn layout(self, input: impl View) -> impl View {
        let problem = self.problem;
        vstack((
            input,
            show(
                bind(move |runtime| problem.with(runtime, Option::is_some)),
                move || {
                    status(bind(move |runtime| {
                        problem.get(runtime).unwrap_or_default()
                    }))
                    .tone(Tone::Danger)
                },
                nothing,
            ),
        ))
        .gap(GAP)
    }
}
