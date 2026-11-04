//! Prints the inventory this machine would show, from read-only D-Bus calls.
//!
//! `cargo run -p crownos-disks --example print_inventory`

use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let json = match crownos_disks::recording::record_system().await {
        Ok(json) => json,
        Err(error) => {
            eprintln!("Couldn't read UDisks2: {error}");
            return ExitCode::FAILURE;
        }
    };
    let uid = rustix::process::getuid().as_raw();
    match crownos_disks::recording::inventory_from_json(&json, uid) {
        Ok(inventory) => {
            println!("{inventory:#?}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("Couldn't decode the recording: {error}");
            ExitCode::FAILURE
        }
    }
}
