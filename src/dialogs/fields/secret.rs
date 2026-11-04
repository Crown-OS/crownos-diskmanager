use crownos_disks::command::Secret;
use crownui::prelude::{Runtime, Signal};

/// Moves a typed password out of its field into a buffer that is wiped when dropped.
pub fn take_secret(runtime: &mut Runtime, field: Signal<String>) -> Secret {
    let mut taken = String::new();
    field.update(runtime, |value| std::mem::swap(value, &mut taken));
    Secret::new(taken)
}
