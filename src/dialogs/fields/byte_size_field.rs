use crownos_disks::model::ByteSize;
use crownui::kit::{SliderOptions, TextInputOptions, select, slider, text_input};
use crownui::prelude::{
    Cx, Listeners, Runtime, Semantic, Signal, Styled, View, hstack, spacer, vstack,
};

use crate::dialogs::sheets::layout::muted_text;
use crate::util::byte_size_parse::{SizeUnit, format_in_unit, parse_size, snap_size};

const NUMBER_WIDTH: f32 = 120.0;
const UNIT_WIDTH: f32 = 90.0;
const GAP: f32 = 8.0;
const SLIDER_STEP: f32 = 1.0;

fn mebibytes(size: ByteSize) -> f32 {
    (size.bytes() / ByteSize::MIB.bytes()) as f32
}

fn from_mebibytes(mebibytes: f32) -> ByteSize {
    ByteSize(mebibytes.max(0.0).round() as u64 * ByteSize::MIB.bytes())
}

/// One size, edited through a slider, a number and a unit, always within `min..=max` and on
/// whole MiB once changed.
#[derive(Clone, Copy)]
pub struct ByteSizeField {
    pub size: Signal<ByteSize>,
    min: ByteSize,
    max: ByteSize,
    slider_position: Signal<f32>,
    typed: Signal<String>,
    unit: Signal<Option<usize>>,
    unit_names: Signal<Vec<String>>,
}

impl ByteSizeField {
    pub fn new(cx: &mut Cx, initial: ByteSize, min: ByteSize, max: ByteSize) -> Self {
        let unit = SizeUnit::fitting(max);
        let field = Self {
            size: cx.signal(initial),
            min,
            max,
            slider_position: cx.signal(mebibytes(initial)),
            typed: cx.signal(format_in_unit(initial, unit)),
            unit: cx.signal(Some(unit.index())),
            unit_names: cx.signal(
                SizeUnit::ALL
                    .iter()
                    .map(|unit| unit.label().to_owned())
                    .collect(),
            ),
        };
        field.follow_slider(cx);
        field.follow_size(cx);
        field.follow_unit(cx);
        field
    }

    fn selected_unit(self, runtime: &Runtime) -> SizeUnit {
        self.unit.with_untracked(runtime, |unit| {
            SizeUnit::from_index(unit.unwrap_or_default())
        })
    }

    fn follow_slider(self, cx: &mut Cx) {
        cx.effect(move |runtime| {
            let wanted = snap_size(
                from_mebibytes(self.slider_position.get(runtime)),
                self.min,
                self.max,
            );
            if self
                .size
                .with_untracked(runtime, |size| size.align_down(ByteSize::MIB) != wanted)
            {
                self.size.set(runtime, wanted);
            }
        });
    }

    fn follow_size(self, cx: &mut Cx) {
        cx.effect(move |runtime| {
            let size = self.size.get(runtime);
            let position = mebibytes(size);
            if self
                .slider_position
                .with_untracked(runtime, |current| *current != position)
            {
                self.slider_position.set(runtime, position);
            }
            let text = format_in_unit(size, self.selected_unit(runtime));
            if self.typed.with_untracked(runtime, |typed| *typed != text) {
                self.typed.set(runtime, text);
            }
        });
    }

    fn follow_unit(self, cx: &mut Cx) {
        cx.effect(move |runtime| {
            let unit = SizeUnit::from_index(self.unit.get(runtime).unwrap_or_default());
            let text = self
                .size
                .with_untracked(runtime, |size| format_in_unit(*size, unit));
            self.typed.set(runtime, text);
        });
    }

    fn commit_typed(self, runtime: &mut Runtime) {
        let unit = self.selected_unit(runtime);
        let current = self.size.with_untracked(runtime, |size| *size);
        let typed = self.typed.with_untracked(runtime, Clone::clone);
        if typed == format_in_unit(current, unit) {
            return;
        }
        match parse_size(&typed, unit) {
            Some(size) => self.size.set(runtime, snap_size(size, self.min, self.max)),
            None => self.typed.set(runtime, format_in_unit(current, unit)),
        }
    }

    pub fn view(self) -> impl View {
        let range = format!("Between {} and {}", self.min, self.max);
        vstack((
            slider(self.slider_position)
                .range(mebibytes(self.min), mebibytes(self.max))
                .step(SLIDER_STEP)
                .label("Size"),
            hstack((
                text_input(self.typed)
                    .width(NUMBER_WIDTH)
                    .label("Size")
                    .on_change(move |cx, _| self.commit_typed(cx)),
                select(self.unit_names, self.unit)
                    .label("Unit")
                    .w(UNIT_WIDTH),
                spacer(),
                muted_text(range),
            ))
            .gap(GAP)
            .items_center(),
        ))
        .gap(GAP)
    }
}
