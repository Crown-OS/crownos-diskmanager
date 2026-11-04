use std::io;
use std::path::Path;
use std::process::Command;
use std::thread;

const OPENER: &str = "xdg-open";

pub fn open_in_files(path: &Path) -> io::Result<()> {
    let mut child = Command::new(OPENER).arg(path).spawn()?;
    thread::spawn(move || child.wait());
    Ok(())
}
