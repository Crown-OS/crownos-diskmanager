mod dialogs;
mod empty;
mod forms;
mod menus;
mod pages;
mod progress;

use anyhow::Result;
use capture::Carrier;
use crownos_disks::Event;
use crownos_disks::command::{OperationId, QuitImpact, Target};

use crate::window::Window;

pub use dialogs::dialogs;
pub use empty::empty;
pub use forms::forms;
pub use menus::menus;
pub use pages::pages;
pub use progress::progress;

fn shot(
    carrier: &mut Carrier,
    window: &mut Window,
    group: &str,
    name: &str,
    description: &str,
) -> Result<()> {
    carrier.capture_state(&mut window.app, group, name, description)?;
    Ok(())
}

/// An operation that shows no sheet of its own: not cancelable and without a fraction.
fn background_operation(id: u64, label: &str, target: Target, quit_impact: QuitImpact) -> Event {
    Event::OperationStarted {
        id: OperationId(id),
        label: label.to_owned(),
        target,
        cancelable: false,
        quit_impact,
    }
}
