use anyhow::{Context, Result};
use capture::Carrier;
use crownos_diskmanager::state::{DialogRequest, Selection};
use crownos_disks::command::{OperationId, QuitImpact, Target};
use crownos_disks::model::{BlockId, HealthStatus, Inventory};
use crownos_disks::{Event, UserError};
use crowntest::{Role, role};

use super::{background_operation, shot};
use crate::fake_disks::{
    self, DATA_VOLUME, USB_STICK, WORKSPACE_DISK, free_region, health_status, labelled, name_where,
    partition_where, partition_where_any,
};
use crate::window::Window;

const GROUP: &str = "Forms";
const PASSWORD: &str = "Correct-Horse-42";
const SHRUNK_SIZE_GB: &str = "80";

pub fn forms(carrier: &mut Carrier) -> Result<()> {
    format_forms(carrier)?;
    rename_forms(carrier)?;
    new_partition_forms(carrier)?;
    resize_forms(carrier)?;
    unlock_forms(carrier)
}

fn opened(
    inventory: &Inventory,
    disk: &str,
    block: &BlockId,
    request: DialogRequest,
) -> Result<Window> {
    let (disk, _) = partition_where(inventory, disk, |partition| &partition.block == block)
        .context("partition")?;
    let mut window = Window::with(inventory.clone())?;
    window.select(Selection::Partition {
        disk,
        block: block.clone(),
    });
    window.open(request);
    Ok(window)
}

fn format_forms(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::removable();
    let (_, sandisk) =
        partition_where(&inventory, USB_STICK, labelled("SANDISK")).context("SANDISK")?;
    {
        let mut window = opened(
            &inventory,
            USB_STICK,
            &sandisk,
            DialogRequest::Format(sandisk.clone()),
        )?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Format",
            "Name and format for a volume, everyday formats first",
        )?;
        window.click_button("More formats");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Format all formats",
            "Every format, including the Linux-only ones",
        )?;
    }
    {
        let missing_tools = fake_disks::removable_missing_tools();
        let mut window = opened(
            &missing_tools,
            USB_STICK,
            &sandisk,
            DialogRequest::Format(sandisk.clone()),
        )?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Format missing tools",
            "Formats whose tools aren't installed name the package to add",
        )?;
    }
    let (_, workspace) =
        partition_where(&inventory, WORKSPACE_DISK, labelled("Workspace")).context("Workspace")?;
    {
        let mut window = opened(
            &inventory,
            WORKSPACE_DISK,
            &workspace,
            DialogRequest::Format(workspace.clone()),
        )?;
        window
            .app
            .click(role(Role::Switch).name("Encrypt with password"));
        window.app.settle();
        window
            .type_into("Password", PASSWORD)
            .type_into("Repeat password", "Correct-Horse");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Format with encryption",
            "Password strength and a mismatched repeat",
        )?;
    }
    let mut window = opened(
        &inventory,
        WORKSPACE_DISK,
        &workspace,
        DialogRequest::Format(workspace.clone()),
    )?;
    window
        .app
        .click(role(Role::Switch).name("Encrypt with password"));
    window.app.settle();
    window
        .type_into("Password", PASSWORD)
        .type_into("Repeat password", PASSWORD)
        .click_button("Format…");
    shot(
        carrier,
        &mut window,
        GROUP,
        "Format confirmation",
        "Destructive confirmation with the password warning",
    )
}

fn rename_forms(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::failing();
    let (disk, data) = partition_where_any(&inventory, labelled(DATA_VOLUME)).context("Data")?;
    for (typed, name, description) in [
        (
            " Archive",
            "Rename",
            "A new volume name, checked against the format's rules",
        ),
        (
            " from the old laptop",
            "Rename invalid name",
            "A name too long for ext4 explains itself",
        ),
    ] {
        let mut window = Window::with(inventory.clone())?;
        window.select(Selection::Partition {
            disk: disk.clone(),
            block: data.clone(),
        });
        window
            .click_button("More Actions")
            .click_menu_item("Rename");
        window.type_into("Name", typed);
        shot(carrier, &mut window, GROUP, name, description)?;
    }
    Ok(())
}

fn new_partition_forms(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::failing();
    let roomy = name_where(&inventory, |disk| {
        health_status(disk) != Some(HealthStatus::Failing)
            && free_region(&inventory, &disk.name).is_some()
    })
    .context("a healthy disk with free space")?;
    let (disk, offset, _) = free_region(&inventory, &roomy).context("free space")?;
    let mut window = Window::with(inventory)?;
    window.select(Selection::Free { disk, offset });
    window.click_button("New Partition");
    shot(
        carrier,
        &mut window,
        GROUP,
        "New partition",
        "Size, name and format for a partition in free space",
    )?;
    window.click_button("Create…");
    shot(
        carrier,
        &mut window,
        GROUP,
        "New partition confirmation",
        "A simple confirmation naming what is created",
    )
}

fn resize_forms(carrier: &mut Carrier) -> Result<()> {
    let failing = fake_disks::failing_with_known_usage();
    let data_disk = name_where(&failing, |disk| {
        disk.partitions()
            .any(|partition| labelled(DATA_VOLUME)(partition))
    })
    .context("Data disk")?;
    let (_, data) = partition_where(&failing, &data_disk, labelled(DATA_VOLUME)).context("Data")?;
    {
        let mut window = opened(
            &failing,
            &data_disk,
            &data,
            DialogRequest::Resize(data.clone()),
        )?;
        shot(
            carrier,
            &mut window,
            GROUP,
            "Resize",
            "A slider and a typed size between the volume's limits",
        )?;
        window.app.click(role(Role::TextField).name("Size"));
        window.app.press_key("Ctrl+A");
        window.app.type_text(SHRUNK_SIZE_GB);
        window.app.press_key("Enter").settle();
        shot(
            carrier,
            &mut window,
            GROUP,
            "Resize shrinking",
            "A smaller size, summarised before it is applied",
        )?;
        window.click_button("Resize");
        shot(
            carrier,
            &mut window,
            GROUP,
            "Shrink confirmation",
            "Shrinking asks first",
        )?;
    }
    let encrypted = fake_disks::encrypted();
    let drive = name_where(&encrypted, |disk| {
        disk.partitions()
            .any(|partition| labelled("Vault")(partition))
    })
    .context("encrypted drive")?;
    for (label, name, description) in [
        (
            "Vault",
            "Resize locked volume",
            "Opened directly (the toolbar offers Unlock instead): a locked volume must be unlocked first",
        ),
        (
            "Photos",
            "Resize encrypted volume",
            "An unlocked LUKS2 volume asks for its password",
        ),
    ] {
        let (_, block) =
            partition_where(&encrypted, &drive, labelled(label)).context("encrypted partition")?;
        let mut window = opened(
            &encrypted,
            &drive,
            &block,
            DialogRequest::Resize(block.clone()),
        )?;
        shot(carrier, &mut window, GROUP, name, description)?;
    }
    let removable = fake_disks::removable();
    let (_, sandisk) =
        partition_where(&removable, USB_STICK, labelled("SANDISK")).context("SANDISK")?;
    let mut window = opened(
        &removable,
        USB_STICK,
        &sandisk,
        DialogRequest::Resize(sandisk.clone()),
    )?;
    shot(
        carrier,
        &mut window,
        GROUP,
        "Resize refused",
        "Opened directly (the toolbar disables Resize): exFAT can't be resized, and the sheet says why",
    )
}

fn unlock_forms(carrier: &mut Carrier) -> Result<()> {
    let inventory = fake_disks::encrypted();
    let drive = name_where(&inventory, |disk| {
        disk.partitions()
            .any(|partition| labelled("Vault")(partition))
    })
    .context("encrypted drive")?;
    let (_, vault) = partition_where(&inventory, &drive, labelled("Vault")).context("Vault")?;
    let mut window = opened(
        &inventory,
        &drive,
        &vault,
        DialogRequest::Unlock(vault.clone()),
    )?;
    shot(
        carrier,
        &mut window,
        GROUP,
        "Unlock",
        "The password for a locked volume",
    )?;
    window.type_into("Password", PASSWORD);
    window.app.press_key("Enter");
    window.app.settle();
    shot(
        carrier,
        &mut window,
        GROUP,
        "Unlocking",
        "Waiting for the volume to unlock",
    )?;
    window.apply(background_operation(
        7,
        "Unlocking Vault",
        Target::Block(vault),
        QuitImpact::Harmless,
    ));
    window.apply(Event::OperationFinished {
        id: OperationId(7),
        result: Err(UserError::new(
            "Wrong password",
            "The password for Vault is incorrect.",
        )),
    });
    shot(
        carrier,
        &mut window,
        GROUP,
        "Unlock wrong password",
        "A rejected password, shown inside the sheet",
    )
}
