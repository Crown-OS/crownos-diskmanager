//! Captures every Disk Manager state for the CrownOS-Design Figma file over sample disks. Run
//! both appearances with `../crownui/capture/scripts/capture-app.sh .`; output lands in
//! `$CROWN_DESIGN_DIR`.

mod fake_disks;
mod states;
mod window;

use capture::Carrier;

fn main() -> anyhow::Result<()> {
    let mut carrier = Carrier::new(
        "Disk Manager",
        "crownos-diskmanager: disks, partition maps, health, every action sheet, progress and \
         error state over sample NVMe, USB, encrypted and failing drives, captured headlessly \
         from the current source.",
    );
    states::pages(&mut carrier)?;
    states::menus(&mut carrier)?;
    states::dialogs(&mut carrier)?;
    states::forms(&mut carrier)?;
    states::progress(&mut carrier)?;
    states::empty(&mut carrier)?;
    let path = carrier.write(None)?;
    println!("{}", path.display());
    capture::finish()
}
