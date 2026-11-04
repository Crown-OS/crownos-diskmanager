//! Records this machine's UDisks2 state as fixture JSON (read-only D-Bus calls).
//!
//! `cargo run -p crownos-disks --example record_fixture -- tests/fixtures/desktop.json`
//! Without a path the JSON goes to stdout.

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
    let Some(path) = std::env::args_os().nth(1) else {
        println!("{json}");
        return ExitCode::SUCCESS;
    };
    match std::fs::write(&path, json + "\n") {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Couldn't write {}: {error}", path.to_string_lossy());
            ExitCode::FAILURE
        }
    }
}
