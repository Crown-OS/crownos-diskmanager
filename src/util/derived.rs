use crownui::prelude::{Cx, Runtime, Signal};

pub fn derived<T: Clone + PartialEq + 'static>(
    cx: &mut Cx,
    mut read: impl FnMut(&mut Runtime) -> T + 'static,
) -> Signal<T> {
    let initial = cx.untrack(&mut read);
    let signal = cx.signal(initial);
    cx.effect(move |runtime| {
        let value = read(runtime);
        if signal.with_untracked(runtime, |current| *current != value) {
            signal.set(runtime, value);
        }
    });
    signal
}
