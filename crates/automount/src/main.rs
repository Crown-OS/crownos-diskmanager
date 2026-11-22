mod automounter;
mod eligibility;
mod util;

use anyhow::{Result, bail};
use crownos_disks::{Command, Event, Interaction, UserError};
use tokio::sync::mpsc::unbounded_channel;

use crate::automounter::Automounter;
use crate::util::fstab::FstabSources;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    if rustix::process::geteuid().is_root() {
        bail!("crownos-automount mounts drives for the signed-in user; run it as that user.");
    }
    let (commands, command_receiver) = unbounded_channel();
    let (events, mut event_receiver) = unbounded_channel();
    tokio::spawn(crownos_disks::run(
        move |event| drop(events.send(event)),
        command_receiver,
    ));
    let mut automounter = Automounter::new(FstabSources::read());
    while let Some(event) = event_receiver.recv().await {
        match event {
            Event::Inventory(inventory) => {
                for block in automounter.observe(&inventory) {
                    let interaction = Interaction::Forbidden;
                    commands.send(Command::Mount { block, interaction })?;
                }
            }
            Event::OperationFinished {
                result: Err(error), ..
            } => report(&error),
            _ => {}
        }
    }
    Ok(())
}

fn report(error: &UserError) {
    let details = error.details.as_deref().unwrap_or(&error.message);
    eprintln!("{}: {details}", error.title);
}
