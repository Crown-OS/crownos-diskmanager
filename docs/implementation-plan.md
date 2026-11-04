# CrownOS Disk Manager — Implementation Plan

Status: research complete, no code written. Verified against UDisks 2.11.2 (running on the
dev host and current in Arch `extra`), CrownUI at `/e/CrownOS/crownui`, and sibling CrownOS
projects as of 2026-11-04.

---

## 0. The decisions that shape everything

| Decision | Choice | Why |
|---|---|---|
| Storage backend | **UDisks2 over D-Bus (zbus), exclusively** | It already does discovery, hotplug, mount, format, partitioning (libfdisk), resize (libblockdev), LUKS, loop devices, ATA SMART and native NVMe health. It does these as a system daemon, so a crash in our app never interrupts a running operation. |
| Privilege | **The app never runs as root. No pkexec helper, no setuid, no custom polkit policy.** | Every privileged action is a UDisks method authorized per action by polkit. `crownos-polkit-agent` already shows the CrownUI password prompt. Raw device I/O for images uses an fd that UDisks hands over through `Block.OpenDevice`. |
| Partition Move | **Not supported** | UDisks has no Move. GParted's move has no on-disk journal and can't recover from power loss. "Extend into free space after" plus "Delete and recreate" covers everyday needs safely. |
| Operation model | **Immediate single operations, each confirmed**, like Disk Utility and Windows. No pending-operations queue as in GParted or the installer. | Beginners understand "click, confirm, done". A queue adds a projection engine and a mental model nobody asked for. |
| Health data | UDisks `Drive.Ata` + `NVMe.Controller` | Verified on this host: NVMe temperature, power-on hours and critical warnings come through UDisks with no root and no smartctl. GNOME Disks has no NVMe health UI, so this is a genuine differentiator. |
| D-Bus bindings | **`udisks2` 0.4 crate on zbus 6** | Maintained (GNOME Disks depends on it). Covers every interface we use: `nvme::controller::ControllerProxy` (SMART), `BlockProxy::open_device → OwnedFd`, `EncryptedProxy::resize`, `Client` helpers. Saves ~300 lines of hand-written proxies. Only logind and the file-chooser portal need small local proxies. |
| Encryption | **Unlock/Lock, "Encrypt with password" on format, and resizing encrypted volumes are all v1** | One UDisks `Format` option (`encrypt.passphrase`, LUKS2) plus `Encrypted.Resize` in the resize sequence. |
| SMART self-tests | **Not offered** | Neither Disk Utility nor Windows offers them. Passive health covers the everyday need. |

---

## 1. Essential features and user workflows

### 1.1 What we take from each reference, and what we leave out

| Source | Adopted | Deliberately left out |
|---|---|---|
| macOS Disk Utility | Sidebar grouped **Internal / External / Disk Images**, disk → volume hierarchy, toolbar of contextual actions, usage bar, First Aid (→ *Check & Repair*), Erase sheet with format + scheme, Restore, Create Image, Mount/Unmount, Info | APFS containers / volume groups, RAID sets, "Show only volumes" mode, image encryption, sparse bundles |
| Windows Disk Management | **Graphical partition map** with free space shown, *Shrink* / *Extend into adjacent free space*, New Simple Volume as a single dialog, format right from the map | Dynamic disks, spanned/striped volumes, drive letters (mount points come from UDisks), VHD attach |
| Windows 11 Settings → Disks & volumes | **Plain-language NVMe health**: remaining life, spare, temperature, Good/Caution/Bad status | Storage Spaces, Storage Sense |
| GNOME Disks | UDisks-based implementation patterns, `.img` / `.iso` / `.xz` restore | Benchmark, mount-options/fstab editor, ATA power/APM settings, partition type GUID editor, flags editor |

### 1.2 Feature set (v1)

**Overview**
- All drives with capacity, used and available space, kind (NVMe / SSD / HDD / USB / SD card / Optical / Disk Image), and health pill.
- Total used/free across mounted volumes.

**Disk (physical drive)**
- Partition map.
- Hardware details: model, serial, firmware revision, capacity, partition table type, connection, rotation rate.
- Health section.
- Actions: Erase Disk (new table + one partition), Create Image, Restore Image, Eject / Power Off (external only).

**Free space region**
- New Partition (size, name, format in one dialog).

**Volume (partition or loop filesystem)**
- Usage bar.
- Mount / Unmount, Open in Files, Rename, Format, Resize, Check & Repair, Delete, Create Image, Restore Image.
- Encrypted volumes: Unlock / Lock.

**Filesystems**
- Offered when `Manager.CanFormat` says the tool is installed: ext4, Btrfs, XFS, FAT32, exFAT, NTFS, F2FS, swap. The list is driven by `Manager.SupportedFilesystems`, which reports 12 types on this host.
- Shown with beginner labels and a recommended default:

  | Label in UI | fs | Default when |
  |---|---|---|
  | **Linux (ext4)** | ext4 | internal disk |
  | **All devices (exFAT)** | exfat | removable drive ≥ 32 GB |
  | **Older devices (FAT32)** | vfat | removable drive < 32 GB |
  | **Windows (NTFS)** | ntfs | — |
  | *More…* → Btrfs (snapshots), XFS (large files), F2FS (flash), Swap | | |

- An unavailable fs stays visible but disabled, with "Requires the *exfatprogs* package". `CanFormat` returns the missing utility name.
- **Encrypt with password** toggle in Erase Disk, New Partition and Format: LUKS2 via `Format`'s `encrypt.passphrase` / `encrypt.type = "luks2"`.
  - Shown for Linux filesystems only, and off by default on removable drives, because other operating systems can't read them.
  - Asks for the password twice and shows a strength hint.
  - The confirmation states plainly: "If you forget this password, the data can't be recovered."

**External storage**
- Hotplug appears instantly.
- Eject unmounts every filesystem on the drive, locks LUKS, then `Drive.PowerOff` if `CanPowerOff`, else `Drive.Eject`.
- Afterwards, "Safe to remove *SanDisk Ultra*" toast.
- The sidebar shows a mount status dot per volume.

**Health** (ATA and NVMe; "Not available over this connection" for USB bridges, SD and loop devices)

**Disk images**
- Open (read-only loop mount by default).
- Create from disk or partition: raw `.img`, optionally `.img.zst` or `.img.xz`.
- Restore `.img` / `.iso` / `.raw` / `.img.xz` / `.img.zst` to a disk or partition, with read-back verification.

### 1.3 Explicitly out of scope
- Partition move.
- RAID / LVM creation (LVM and MD members are displayed read-only).
- Benchmarks.
- ATA secure erase / NVMe sanitize.
- fstab / mount-option editing.
- Partition type GUIDs or flags editing (`update-partition-type` sets them from the chosen fs).
- Creating MBR extended/logical partitions (displayed only).
- qcow2 / vhd / vmdk / dmg.
- Disk-to-disk clone (Create Image then Restore covers it).
- SMART self-tests.
- LUKS password change, header backup, key slots, encryption conversion.

### 1.4 Core workflows (each ≤ 1 dialog + 1 confirmation)

1. **Check space:** open app → Overview shows every drive's usage. Click a drive → map + volumes.
2. **Format a USB stick:** select stick in sidebar → *Erase* in toolbar → name + format (default exFAT) → confirm (tier 2) → progress → done toast.
3. **Write an ISO to USB:** select stick → *Restore Image* → pick `.iso` via portal → confirm (tier 2) → progress (write, then verify) → "Ready to boot" toast.
4. **Add a partition:** click the free-space segment in the map → *New Partition* → size slider/field, name, format → confirm (tier 1) → done.
5. **Grow a partition:** select volume → *Resize* → slider bounded by adjacent free space → confirm → partition grows, then the encryption layer (if any), then the filesystem, all automatically.
6. **Shrink a partition:** same dialog; the minimum comes from used space. The app checks the fs, shrinks the fs, then the encryption layer, then the partition. Encrypted LUKS2 volumes ask for their password inside the same dialog.
7. **Encrypted external drive:** *Erase* → name, Linux (ext4), turn on *Encrypt with password*, enter it twice → confirm → done. On the next plug-in, the volume shows as locked: *Unlock* → password → mounted.
8. **Mount / unmount / rename:** one click in the toolbar; rename is an inline dialog with per-fs label validation.
9. **Eject:** eject icon on the sidebar row, or toolbar *Eject*.
10. **Check disk health:** select disk → health pill in header → Health section with plain-language values.
11. **Open an image:** toolbar *Open Image* → appears under Disk Images, auto-mounted read-only → eject icon detaches the loop device.

---

## 2. Pages and navigation

One window, `sidebar_with_header`. There is no navigation stack; the sidebar selection *is* the page. Every action is either in the header toolbar or on the selected item's card, so no operation is more than one click and one dialog away.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│ [Overview]            │ ⏏ Unmount  🩺 Check  ✂ Partition  ⌫ Erase  ▣ Image ▾ │  ← header (contextual)
│ INTERNAL              ├──────────────────────────────────────────────────────┤
│ ▾ Samsung 980  512 GB │ ▣ Samsung MZVL2512HDJD   512 GB · NVMe · ● Good       │
│     EFI      vfat ●   │ ┌──────┬───────────────────────────────────┬───────┐ │
│     Root     ext4 ●   │ │ EFI  │ Root  ext4  475 GB  ▓▓▓▓▓░░ 62%    │ Free  │ │ ← partition map
│ ▸ Crucial P3  500 GB  │ └──────┴───────────────────────────────────┴───────┘ │
│ EXTERNAL              │ ┌ Root ─────────────────────────────────────────────┐ │
│ ▾ SanDisk 64 GB    ⏏  │ │ ext4 · mounted at /  ▓▓▓▓▓▓▓░░░ 295 GB of 475 GB  │ │ ← selected region card
│     USB      exfat ○  │ │ [Open in Files] [Rename] [Resize] [Format] [⋯]     │ │
│ DISK IMAGES           │ └───────────────────────────────────────────────────┘ │
│ ▸ ubuntu.iso       ⏏  │ Health   Temperature 39 °C · Life used 3% · …         │
│                       │ Details  Serial · Firmware · Table GPT · …            │
│ [+ Open Image]        │                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

| Sidebar entry | Page | Content |
|---|---|---|
| **Overview** (top item) | `pages/overview` | One card per drive: icon, name, kind, `segmented_bar` usage, health pill. Click → that disk. Empty state when only the system disk exists. |
| Disk (`sidebar_group`) | `pages/disk` | Header line, **partition map**, selected-region card, Health section, Details section (scrollable). Selecting a map segment does not change the sidebar selection, which keeps the map the primary surface the way Windows does. |
| Volume (`sidebar_subitem`) | `pages/disk` with that region preselected | Same page. Volumes are shortcuts into the disk page, which keeps disk vs. partition vs. volume relationships visible at all times. |
| Disk image (group under *Disk Images*) | `pages/disk` | Kind "Disk Image", backing file path, no health section. |

**Header toolbar** (contextual, built from the selection):

| Selection | Toolbar actions |
|---|---|
| Disk | Erase · Image ▾ (Create, Restore) · Eject/Power Off (external) |
| Volume | Mount/Unmount · Check & Repair · Resize · Format · Image ▾ · ⋯ (Rename, Delete) |
| Free space | New Partition |
| Locked LUKS | Unlock |
| Always | Open Image (right-aligned) |

Disabled actions stay visible, with a tooltip explaining why ("Can't resize: exFAT doesn't support resizing", "Unmount first", "This is the disk CrownOS is running from").

**Sheets** (`kit::dialog`, `dismiss_on_scrim(false)` for destructive ones):
- Erase Disk
- New Partition
- Format Volume
- Resize
- Rename
- Unlock
- Create Image
- Restore Image
- Confirm (tiered, §7)
- Progress
- Error details

**Right-click** on a map segment or sidebar row opens the same actions via `context_menu_at`.

---

## 3. CrownUI components

### 3.1 Existing kit widgets used

| Need | CrownUI widget |
|---|---|
| Window shell + toolbar | `sidebar_with_header(menus, header, content)` |
| Internal/External/Images grouping | `sidebar_section`, `sidebar_group(label, icon, open, children)`, `sidebar_item`, `sidebar_subitem` |
| Eject on sidebar row | `sidebar_item(..).trailing(icon_button(icons::ARROW_UP_FROM_LINE, "Eject").variant(ButtonVariant::Ghost))`. The kit preview `previews/sidebar_item.rs` already shows exactly this. |
| Toolbar actions | `kit::button(..).leading_icon(..).variant(Ghost)`, `popup_menu` for *Image ▾* and *⋯* |
| Usage bars (overview, volume card) | `segmented_bar` (paint-only; it has a `segmented_bar_disk_usage` preview) |
| Detail rows | `section(title, ..)`, `setting_row(label, value)`, `card`, `pane` |
| Health pill / kind chips | `status(label).tone(Tone::Success / Warning / Danger / Neutral)`, `badge`, `tag_chip` |
| Forms | `text_input`, `select`, `slider`, `kit::toggle`, `toggle_group` (GPT / MBR), `choice_list` (format picker with descriptions), `radio_group` |
| Sheets | `dialog(open, ..)` + `DialogOptions::dismiss_on_scrim(false)` |
| Progress | `progress(fraction)`, `spinner()` |
| Errors / warnings | `callout(Tone, title).body(..)` inline; `use_toasts(cx).show(..)` for completion |
| Context menus | `context_menu_at`, `menu_item`, `menu_item_disabled`, `menu_separator` |
| Hints | `tooltip(label, content)` |
| Icons | `icons::HARD_DRIVE`, `USB`, `DISC`, `DATABASE`, `LOCK`, `LOCK_OPEN`, `ERASER`, `TRASH_2`, `SCISSORS`, `STETHOSCOPE`, `WRENCH`, `HARD_DRIVE_DOWNLOAD`/`UPLOAD` (image create/restore), `CIRCLE_ALERT`, `INFO` |

### 3.2 App-local components to build (`src/components/`)

| Component | Built from | Notes |
|---|---|---|
| `partition_map` | `hstack` of `.grow(weight)` segment buttons | Clickable, keyboard-navigable (←/→), each segment `.role(Button).label("Root, ext4, 475 GB, 62 % used")`. Weight = `max(size, disk_size × 2 %)` so a 1 MiB BIOS-boot partition stays clickable. Free space is drawn neutral/outlined. Each segment holds an inner used-space fill. Selection ring uses `theme.accent`. `segmented_bar` can't be used because it has no hit-testing. |
| `usage_meter` | `segmented_bar` + caption | "295 GB used of 475 GB · 180 GB free". Uses the installer's `ui/components/meter.rs` a11y pattern (`Role::ProgressBar` + value). |
| `health_summary` | `status` + `setting_row`s | Maps `HealthStatus` → tone + sentence. |
| `byte_size_field` | `text_input` + `select` (MB/GB/TB) bound to the same `Signal<u64>` as a `slider` | Used by New Partition and Resize. Clamps to min/max and snaps to 1 MiB. |
| `confirm_sheet` | `dialog` + `callout(Danger)` + list of affected volumes + `toggle` / `text_input` + `button(..).variant(Destructive)` | Tiers in §7. |
| `progress_sheet` | `dialog` + `progress` + rate/ETA text + Cancel (only if job is cancelable) | Non-dismissable while running. |
| `encryption_fields` | `kit::toggle` + two `text_input(..).secret(true)` + strength `status` | Shared by Erase Disk, New Partition and Format. The confirm button stays disabled until both passwords match. Values are cleared from their signals as soon as the command is sent. |
| `disk_icon` | `kit::icon` chosen by `DiskKind` | |

### 3.3 CrownUI gaps to resolve (upstream or local)

1. **File chooser:** CrownUI has none. Use the XDG portal `org.freedesktop.portal.FileChooser` (CrownOS ships `xdg-desktop-portal-crownos`) through a small hand-written zbus 6 proxy (`util/portal.rs`, `OpenFile`/`SaveFile` + the `Request.Response` signal). `ashpd` 0.13 is still on zbus 5, so it would pull a second zbus into the binary.
2. **Window close interception:** we need to warn or block closing during an image copy. No such API was found in `runtime/src/config.rs`; verify or add one upstream.
3. **Categorical palette:** "every colour from `use_theme`", but `Theme` has no categorical/series colours for partition segments. Add a small `theme.series` palette upstream, or derive one from `accent` and `status`.
4. **No table / tree:** not needed. `sidebar_group` covers disk→volume nesting and `setting_row` covers key/value details.

---

## 4. Linux APIs, Rust libraries and utilities

### 4.1 Runtime interfaces

| Concern | Interface | Notes |
|---|---|---|
| Discovery + hotplug | `org.freedesktop.DBus.ObjectManager` on `/org/freedesktop/UDisks2`: `GetManagedObjects`, `InterfacesAdded`, `InterfacesRemoved`, plus `PropertiesChanged` (match rule `path_namespace=/org/freedesktop/UDisks2`) | Replaces udev/libblkid/sysfs polling entirely. Pattern to copy: `crownconnect-linux/src/bluetooth/telephony/mod.rs`. |
| Drive info | `UDisks2.Drive`: `Vendor`, `Model`, `Serial`, `WWN`, `Revision`, `Size`, `ConnectionBus`, `RotationRate`, `Removable`, `MediaRemovable`, `Ejectable`, `CanPowerOff`, `Optical`, `SortKey`; `Eject`, `PowerOff` | |
| Block info | `UDisks2.Block`: `Device`, `PreferredDevice`, `Size`, `Drive`, `IdType`, `IdUsage`, `IdLabel`, `IdUUID`, `HintSystem`, `HintIgnore`, `CryptoBackingDevice`; `Format`, `OpenDevice`, `Rescan` | `OpenForBackup`/`OpenForRestore` are deprecated since 2.7.3. Use `OpenDevice("r"/"w", {flags: O_EXCL\|O_CLOEXEC\|O_DIRECT})`. |
| Partitioning | `PartitionTable`: `Type`, `Partitions`; `CreatePartition`, `CreatePartitionAndFormat`. `Partition`: `Number`, `Offset`, `Size`, `Type`, `Name`, `UUID`, `Table`, `IsContained`, `IsContainer`; `Resize`, `Delete`, `SetName` | 1 MiB alignment is done by UDisks (libfdisk). |
| Filesystem | `Filesystem`: `MountPoints`, `Size`; `Mount`, `Unmount`, `SetLabel`, `Resize`, `Check`, `Repair` | Used/free from `statvfs` on the mount point (unprivileged). `Filesystem.Size` exists only for ext/xfs. |
| Capabilities | `Manager`: `SupportedFilesystems`, `CanFormat`, `CanResize` (bitmask OFFLINE_SHRINK=2, OFFLINE_GROW=4, ONLINE_SHRINK=8, ONLINE_GROW=16), `CanCheck`, `CanRepair`, `LoopSetup` | Drives every enable/disable decision, so nothing is hard-coded per distro. |
| Encryption | `Encrypted`: `Unlock`, `Lock`, `Resize(size, {passphrase})`, `CleartextDevice`, `HintEncryptionType`, `MetadataSize`. Creation via `Block.Format` options `encrypt.passphrase`, `encrypt.type = "luks2"` | LUKS2 resize needs the passphrase unless the volume key is already in the kernel keyring. `MetadataSize` gives the LUKS header overhead used in resize arithmetic. `SupportedEncryptionTypes` reports `luks1`, `luks2` on this host. |
| Swap | `Swapspace`: `Active` | Display only, plus a blocking guard. |
| Images | `Manager.LoopSetup(fd, {read-only})`, `Loop`: `BackingFile`, `Delete`, `SetAutoclear` | No root, polkit `loop-setup` = yes. |
| ATA health | `Drive.Ata`: `SmartSupported`, `SmartEnabled`, `SmartFailing`, `SmartTemperature` (K), `SmartPowerOnSeconds`, `SmartNumBadSectors`, `SmartNumAttributesFailing`; `SmartUpdate({nowakeup:true})` | `nowakeup` avoids spinning up sleeping HDDs. |
| NVMe health | `NVMe.Controller`: `SmartCriticalWarning`, `SmartTemperature` (K), `SmartPowerOnHours`, `SmartUpdated`; `SmartGetAttributes` → `percent_used`, `avail_spare`, `spare_thresh`, `media_errors`, `unsafe_shutdowns`, `wctemp`, `cctemp`, `total_data_written` | Verified live on the dev host (both NVMe drives). |
| Progress | `UDisks2.Job`: `Operation`, `Progress`, `ProgressValid`, `Rate`, `ExpectedEndTime`, `Objects`, `StartedByUID`, `Cancelable`; `Cancel`, signal `Completed` | Find the job whose `Objects` contains our target. |
| Sleep/shutdown safety | `org.freedesktop.login1.Manager.Inhibit("sleep:shutdown:idle", "CrownOS Disk Manager", reason, "block")` | Held only while an operation runs. The fd is dropped when it completes. |
| Usage | `statvfs` (rustix) | |
| Mount table (system-disk guard) | `/proc/self/mountinfo` | Reuse the parser idea from `crownos-files/src/fs/mounts.rs`. |
| Notifications | crownotify (`send_general_notification`), freedesktop fallback, per `crownos-screencapture/src/notify/` | Only for operations finishing while the window is unfocused. |

### 4.2 Crates

| Crate | Purpose | Status |
|---|---|---|
| `crownui` (path, features `wayland, impeller, static, tokio, accessibility`; no `mcp`) | UI | Same as installer and polkit agent. An in-process agent server has no place in a partitioning tool. |
| `udisks2` 0.4 | Typed UDisks2 proxies + `Client`/`Object` helpers | Runtime-agnostic, zbus ^6. Note: it enables zbus default features (the async-io executor) and depends on `gettext-rs`. In M0, run `cargo tree -e features` to confirm zbus picks tokio, and accept or patch out the async-io thread. |
| `zbus` 6 (`features=["tokio"]`) | Connection + the two local proxies (logind `Inhibit`, portal `FileChooser`) | Same major version as `udisks2`, so there is one zbus in the binary |
| `tokio` (`rt, sync, time, macros`) | Workers | |
| `rustix` (`fs`) | `statvfs`, `fstat`, `fsync`, aligned I/O, `posix_fadvise` | |
| `nix` (`ioctl`) | Only `BLKGETSIZE64` via `ioctl_read!(blkgetsize64, 0x12, 114, u64)` on the OpenDevice fd | GNOME Disks does the same. `fstat` doesn't give block device size. |
| `zstd` (feature `zstdmt`) | Multi-threaded image compression | Uses all cores |
| `liblzma` 0.4 | `.xz` images | Maintained fork of `xz2`, also used by GNOME Disks |
| `blake3` | Restore verification | SIMD/AVX-512 accelerated hashing |
| `gptman` 3 | Relocate GPT backup header after restoring a smaller image to a larger disk, on the already-open rw fd | Avoids shelling out to `sfdisk --relocate` as root |
| `serde` + `serde_json` | Operation journal & partition-table snapshots | |
| `zeroize` | Wipe passphrase buffers after unlock / format / resize | |
| `thiserror` 2 / `anyhow` | `storage::Error` / app main | Per CLAUDE.md |
| `crowntest`, `insta` (dev) | Headless UI + snapshot tests | |

**Not used, and why:**
- `libparted` (stale since 2021).
- `libblkid-rs` and `udev` (UDisks provides the Id* properties and hotplug).
- `sys-mount` and `loopdev` (need root).
- `libatasmart` / smartctl JSON (UDisks 2.11 covers ATA via libblockdev, and NVMe natively).
- `qcow` crates (stale).

### 4.3 System packages (CrownOS packaging work, outside this repo)

The research found **`udisks2` is not on the CrownOS ISO or install package lists**. Neither are `exfatprogs`, `f2fs-tools` or `polkit`, and `crownos-polkit-agent` has no PKGBUILD yet. The `crownos-diskmanager` PKGBUILD must depend on:

- `udisks2` (pulls libblockdev fs/part/crypto/nvme/smart plugins, libatasmart, libnvme, cryptsetup)
- `polkit`, `crownos-polkit-agent`
- `e2fsprogs`, `dosfstools`, `exfatprogs`, `ntfs-3g` (mkfs.ntfs + ntfsresize), `btrfs-progs`, `xfsprogs`
- optional: `f2fs-tools`

---

## 5. Architecture

### 5.1 Process and threads

```
┌──────────── crownos-diskmanager (user, unprivileged) ────────────┐
│ UI thread (CrownUI)                                              │
│   Store { inventory: Signal<Arc<Inventory>>, selection, ops… }  │
│        ▲ events (batched 64)                │ commands           │
│ crownui-tasks thread (tokio current-thread) ▼                    │
│   storage worker ── zbus system connection ──────────────┐       │
│   ├─ watcher: ObjectManager + PropertiesChanged           │       │
│   ├─ operation runner (one destructive op per disk)       │       │
│   └─ image copier → spawn_blocking (fd from OpenDevice)   │       │
└───────────────────────────────────────────────────────────┼───────┘
                                                            ▼
                     udisksd (root) ── polkitd ── crownos-polkit-agent (CrownUI prompt)
```

The UI ↔ worker bridge is `src/util/worker.rs` (`spawn_worker`, `MessageProxy`), copied from `crownos-settings/src/util/worker.rs`, which is the canonical copy. The command/event/state-struct pattern follows `crownos-settings/src/net/`.

### 5.2 Module layout

This follows the crownos-settings layering (each layer knows only the one below it), with backend naming after the installer's `engine/` precedent:

```
src/
  main.rs                    AppConfig + crownui::run
  app/          mod, root, navigation, theme
  sidebar.rs                 sections → groups → items from Inventory
  pages/        mod, overview.rs, disk/{mod, header, region_card, health, details}.rs
  dialogs/      mod, erase_disk, new_partition, format, resize, rename, unlock,
                create_image, restore_image, confirm, progress, error
  components/   partition_map, usage_meter, health_summary, byte_size_field, disk_icon
  state.rs                   Store (Copy struct of Signals)
  storage/
    mod.rs                   re-exports only
    error.rs                 StorageError (thiserror, wraps udisks2::Error) → user message
    client.rs                udisks2::Client setup, ObjectManager stream
    model/                   ids.rs, disk.rs, region.rs, volume.rs, filesystem_kind.rs,
                             disk_kind.rs, health.rs        ← pure types, no I/O
    snapshot.rs              ObjectMap → Inventory          ← pure, fixture-tested
    watcher.rs               signals → ObjectMap → debounced Inventory
    guard.rs                 safety policy                  ← pure, fixture-tested
    limits.rs                resize min/max, label rules    ← pure
    operations/              mount, eject, format, partition, resize, check, encryption, image
    journal.rs               interrupted-op records + table snapshots
    worker.rs                Command / Event enums, dispatch
  util/         worker.rs, bytes.rs (human sizes), inhibit.rs (logind proxy),
                portal.rs (FileChooser proxy), notify.rs
```

### 5.3 Data model (pure, `storage/model`)

```
Inventory  { disks: Vec<Disk>, system: SystemMounts }
Disk       { id: DiskId, kind: DiskKind, name, model, serial, size: ByteSize,
             connection: Connection, removal: Removal, table: Option<TableKind>,
             layout: Vec<Region>, health: Health, backing_file: Option<PathBuf> }
Region     = Partition(Partition) | Free { offset, size }
Partition  { id: BlockId, number, offset, size, name, content: Content }
Content    = Filesystem(Volume) | Encrypted { kind: LuksVersion, header_size,
                                        unlocked: Option<(BlockId, Box<Content>)> }
           | Swap { active } | Member(LvmOrRaid) | Unknown | Empty
Volume     { fs: FilesystemKind, label, uuid, mount_points: Vec<PathBuf>, usage: Option<Usage> }
DiskKind   = Nvme | Ssd | Hdd | Usb | SdCard | Optical | Image
Health     = Unavailable | Reported { status: HealthStatus, temperature_c, power_on_hours,
             life_used_percent, spare_percent, media_errors, bad_sectors, notes: Vec<HealthNote> }
HealthStatus = Good | Caution | Failing
```

`DiskId` and `BlockId` are newtypes over the D-Bus object path. Paths are never passed around as strings.

**Disk kind classification** (verified: `ConnectionBus` is `""` for NVMe on this host):

| Condition | Kind |
|---|---|
| `Loop` interface present | Image |
| `Optical` true | Optical |
| `ConnectionBus` = `usb` | Usb |
| `ConnectionBus` = `sdio` / mmcblk | SdCard |
| has `NVMe.Controller` | Nvme |
| `RotationRate` = 0 | Ssd |
| otherwise (`RotationRate` > 0 or −1) | Hdd |

**Free regions** are computed from the sorted partition offsets against the table's usable range. Gaps under 1 MiB are ignored.

**Health status** is derived in a pure fn and unit-tested:
- **Failing**:
  - NVMe: any critical warning other than temperature, or available spare < threshold.
  - ATA: `SmartFailing`.
- **Caution**:
  - Temperature ≥ `wctemp`, or ≥ 60 °C on ATA.
  - NVMe: `percent_used` ≥ 100, or `media_errors` > 0.
  - ATA: bad sectors > 0, or any attribute failing now or in the past.
- **Good**: otherwise.
- **Unavailable**: no SMART interface.

Kelvin → °C happens once, in the model.

### 5.4 Discovery and updates

1. On start: `GetManagedObjects` → `ObjectMap` (`HashMap<OwnedObjectPath, InterfaceProps>`) → `snapshot::build(&map, &mounts)` → `Arc<Inventory>` → UI.
2. Subscribe to `InterfacesAdded`, `InterfacesRemoved`, and `PropertiesChanged` on the UDisks namespace. Each signal patches the `ObjectMap`.
3. Rebuild the inventory with a 50 ms debounce. A typical system has tens of objects, so a full rebuild is microseconds. It is simpler and more correct than incremental diffing, and the keyed `for_each` in the UI keeps re-render minimal.
4. Usage (`statvfs`) is refreshed on mount changes, after operations, and every 30 s for mounted volumes while the window is visible.
5. Health: read cached properties instantly. Call `SmartUpdate({nowakeup})` when a disk page opens, at most once per 5 min per drive.
6. Lost `udisksd` (`NameOwnerChanged`) → "Storage service isn't running" full-page state with Retry. It never crashes.

### 5.5 Commands, events, operations

```
Command = Mount(BlockId) | Unmount(BlockId) | Eject(DiskId) | Rename{id, label}
        | Unlock{id, passphrase} | Lock(BlockId) | Check(BlockId) | Repair(BlockId)
        | Format{target, spec} | EraseDisk{disk, scheme, spec} | CreatePartition{disk, offset, size, spec}
        | DeletePartition(BlockId) | Resize{id, new_size, passphrase: Option<Secret>}
        | CreateImage{source, dest, compression}
        | RestoreImage{image, target, verify} | OpenImage(PathBuf) | Cancel(OperationId)
        | RefreshHealth(DiskId)
Event   = Inventory(Arc<Inventory>) | Started{op, label} | Progress{op, phase, fraction, rate, eta}
        | Finished{op, outcome: Result<Summary, UserError>} | Interrupted(Vec<JournalEntry>)
        | ServiceUnavailable
```

Every D-Bus call passes `auth.no_user_interaction = false`. `UserError` holds a short title, a plain-language explanation, an optional action ("Unmount and retry"), and raw details behind a disclosure. Mapping:

| UDisks error | User sees |
|---|---|
| `Error.NotAuthorizedDismissed` | Nothing (the user cancelled the password prompt), and the op silently returns to idle |
| `Error.NotAuthorized` | "You don't have permission to change this disk." |
| `Error.DeviceBusy` | "*Root* is in use. Close files and apps using it, then try again." |
| `Error.Cancelled` | "Cancelled. The disk was left unchanged." (or op-specific recovery text) |
| `Error.NotSupported` / missing tool | "Formatting as exFAT requires the *exfatprogs* package." |
| `Error.Failed` + stderr | Op-specific title + stderr in details |

Multi-step operations are **ordered so every intermediate state is consistent**:
- **Shrink:** `Filesystem.Check` (abort on errors and offer Repair) → `Filesystem.Resize(new − luks_header)` → `Encrypted.Resize(new − luks_header)` *(encrypted only)* → `Partition.Resize(new)`. Each layer is shrunk from the inside out. If interrupted anywhere, every inner layer is smaller than its container, which is harmless.
- **Grow:** `Partition.Resize(new)` → `Encrypted.Resize(0 = max)` *(encrypted only)* → `Filesystem.Resize(0 = max)`. Each layer is grown from the outside in. If interrupted, every outer layer is bigger than its contents, which is harmless. Resume = grow the remaining inner layers to max.
- **Encrypted volumes** must be unlocked to resize. The Resize sheet offers Unlock inline when locked, and asks for the LUKS2 passphrase (passed to `Encrypted.Resize`) in the same sheet. `luks_header` comes from `Encrypted.MetadataSize`.
- **Erase disk:** `Format("gpt"|"dos", tear-down)` → `CreatePartitionAndFormat(0, 0 = max, …)`.
- **Eject:** unmount all → lock LUKS → PowerOff/Eject. A failure at any step leaves everything still mounted, so nothing is lost.

**Resize bounds** (`limits.rs`):
- Max = partition end + adjacent free region after it.
- Min = used space + max(10 %, 1 GiB), rounded to 1 MiB. If the volume is unmounted and usage is unknown, it is mounted read-only to measure, then unmounted. resize2fs/ntfsresize refuse safely if the minimum is too small.
- Allowed directions come from `CanResize` flags matched against the current mount state:
  - ext4: grow online or offline, shrink offline.
  - xfs: grow only.
  - btrfs: online only, so it is temporarily mounted if needed.
  - vfat, ntfs, f2fs: offline.
  - exfat, udf: not resizable.
- For an encrypted volume, the inner filesystem's rules apply. The bounds subtract the LUKS header.

**Image copy** (`operations/image.rs`, runs in `spawn_blocking`):
- **Source/target fd:** `Block.OpenDevice("r"|"w", O_EXCL|O_DIRECT)`. polkit `open-device` is `auth_admin_keep`.
- **Size:** `BLKGETSIZE64`.
- **I/O loop:** 4 MiB page-aligned buffers, with a reader thread and a writer thread handing off over a bounded channel of 2 buffers so read and write overlap.
- **Create:** all-zero blocks are skipped with `seek`, producing sparse `.img` files (faster, smaller on disk). zstd runs multi-threaded. Writes to `dest.part`, then `fsync`, then rename on success.
- **Restore:**
  - Refuse if decompressed size > target.
  - Tear down the target: unmount children, lock LUKS, deactivate swap.
  - Stream-decompress (xz / zstd / raw).
  - Write and `fsync`.
  - Optional verify: re-read N bytes and compare a BLAKE3 hash computed during the write.
  - If the image's GPT is shorter than the disk, relocate the backup header with `gptman` on the same fd.
  - `Block.Rescan`.
- **Progress:** throttled to 200 ms. Cancel is checked between buffers.

---

## 6. Security and permission model

| Principle | Implementation |
|---|---|
| Least privilege | The app runs as the logged-in user and refuses to start as root (Wayland + root is wrong, and it is never needed). |
| One privileged component, already audited | All mutations go through `udisksd`. We ship **no** helper, no setuid binary, no `.policy` file. |
| Per-action authorization | polkit decides per UDisks action. As shipped (active local session): |

| Action (`org.freedesktop.udisks2.*`) | Default | Triggers |
|---|---|---|
| `filesystem-mount`, `eject-media`, `power-off-drive`, `modify-device`, `loop-setup`, `encrypted-unlock`, `ata-smart-update`, `nvme-smart-update`, `rescan`, `cancel-job` | allowed | Removable / non-system devices: no prompt |
| `filesystem-mount-system`, `modify-device-system`, `encrypted-unlock-system`, `filesystem-unmount-others` | auth_admin_keep | Internal disks: password once, cached ~5 min, so multi-step resize prompts once |
| `open-device`, `open-device-system` | auth_admin_keep | Image create/restore |
| inactive / remote sessions | auth_admin | |

- **Prompt UI**: `crownos-polkit-agent` (registered per logind session) shows the CrownUI password dialog. The app needs zero polkit code. Packaging must make sure the agent is installed and started (`graphical-session.target`).
- **fd scoping**: image I/O gets one fd to one device, opened `O_EXCL` by udisksd. The app can't touch other devices even though it is doing raw I/O.
- **Passphrases** (unlock, encrypt-on-format, LUKS2 resize):
  - Held in a `zeroize` buffer and passed once in the D-Bus call.
  - Cleared from UI signals as soon as the command is sent.
  - Never logged, persisted, or stored in the journal.
  - Each passphrase travels only as far as udisksd, on the local system bus.
- **Local data** (journal, table snapshots) lives in `$XDG_STATE_HOME/crownos/diskmanager/`, mode 0600. It holds layout metadata only, never file contents.

---

## 7. Safety mechanisms for destructive operations

1. **Protected targets (`guard.rs`, pure, unit-tested from fixtures).**
   - The *system disk* is any disk backing `/`, `/boot`, `/efi`, `/home`, `/usr`, `/var`, active swap, or the live medium (`/run/archiso/bootmnt`, the installer's `IsLiveMedium` rule).
   - On it, these are **disabled, with an explanation** rather than hidden: erase disk, restore image, delete/format/shrink of in-use partitions.
   - Still allowed: grow, rename, check (read-only check of a mounted fs is not offered), and creating partitions in free space.
   - `HintSystem` / `HintIgnore` from UDisks are also honoured.
2. **Tiered confirmation**, scaled to how much is lost:

   | Tier | Operations | Confirmation |
   |---|---|---|
   | 0 | mount, unmount, rename, eject, unlock, open image, create image, grow | none |
   | 1 | create partition, shrink, check & repair | sheet with Primary button |
   | 2 | format volume, delete partition, erase **removable** disk, restore to removable disk | sheet listing **everything that will be lost** (each volume: label, fs, used space). Destructive red button reads "Erase *SanDisk Ultra*", not "OK". Focus starts on Cancel, Enter does not confirm, scrim click does not dismiss. |
   | 3 | erase / restore an **internal** disk | tier 2 + type the disk name to enable the button |

3. **Target identity revalidation.** UDisks block paths are derived from kernel names (`block_devices/sdb`), so a *different* USB stick plugged in after the first one is removed can reuse the same path. When a confirmation opens, the app captures a fingerprint: drive serial/WWN, size, table UUID, partition UUID + offset. Immediately before execution the worker re-reads the fingerprint and aborts if it changed ("The disk changed while this window was open"). A hotplug removal of the target also closes the sheet.
4. **UDisks safety flags**: `Format` always uses `tear-down = true` and `dry-run-first = true`. `Unmount` never uses `force`.
5. **Mounted-filesystem restrictions**: busy targets are shown in the confirmation ("*Root* will be unmounted"), and the unmount happens as an explicit first step. If it is busy, the user gets the DeviceBusy message and nothing destructive has happened.
6. **Partition table snapshot before every table change.** The layout (offsets, sizes, type GUIDs, partition UUIDs, names, table type) is read from UDisks properties, so no privilege is needed. It is written to `state/tables/<serial>-<timestamp>.json`, keeping the last 10 per disk, as a record for manual recovery.
   - **No Undo for Delete Partition.** `PartitionTable.CreatePartition` wipes filesystem signatures on the new partition, so recreating a deleted partition through UDisks would hide its data rather than restore it. Delete relies on the tier-2 confirmation instead.
7. **Interrupted-operation journal** (`journal.rs`).
   - Before starting, `{op, fingerprint, step, started_at}` is written atomically. It is updated per step and removed on completion.
   - On launch, leftover entries produce a callout on the affected disk with op-specific recovery:
     - *Shrink/grow interrupted after step 1* → "Finish resizing" (safe, idempotent).
     - *Restore/erase interrupted* → "The disk's contents are incomplete. Restore again or erase it."
     - *Format interrupted* → "Format again."
   - UDisks jobs run in `udisksd`, so they survive an app crash. On launch the worker also re-attaches to live `Job` objects with `StartedByUID == getuid()` and resumes the progress sheet.
8. **Interruption prevention.**
   - A logind `sleep:shutdown:idle` block inhibitor is held for the duration of any tier ≥ 1 op or image copy.
   - Window close during an image copy asks to cancel (pending CrownUI support, §3.3).
   - On battery < 20 % (UPower `DisplayDevice`), long ops show a warning.
9. **One destructive op per disk at a time.** Toolbar actions for that disk are disabled while it is busy. Non-destructive ops on *other* disks remain available.
10. **Check before shrink.** `Filesystem.Check` runs first. On errors the app stops and offers *Repair* instead of resizing a damaged fs.

---

## 8. Implementation roadmap

Each milestone ends with `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` and a commit (CLAUDE.md). Use `CARGO_INCREMENTAL=0` because the `/e` volume is nearly full.

**M0 — Scaffold**
- Cargo deps (`udisks2` 0.4, `zbus` 6 with tokio). Verify the zbus executor choice with `cargo tree -e features`.
- Lints (`unwrap_used`/`expect_used` warn, `clippy.toml` allow-in-tests).
- Release profile matching siblings (`lto="fat"`, `codegen-units=1`, `panic="abort"`, `strip`).
- `main.rs` + `AppConfig`, `provide_configured_theme`, empty `sidebar_with_header`.
- `util/worker.rs` copied from crownos-settings.
- `storage/error.rs`.
- Packaging issue filed: add `udisks2`, `polkit`, `crownos-polkit-agent`, `exfatprogs`, `ntfs-3g`, `xfsprogs`, `btrfs-progs` to CrownOS.

**M1 — Read-only inventory** *(the app is already useful after this)*
1. `storage/client.rs` on `udisks2::Client`. The ObjectManager stream converts `udisks2` objects into the pure `ObjectMap`, so model tests never need D-Bus.
2. Record `GetManagedObjects` from real machines (this host's two NVMe drives, a USB stick, an ISO loop) into `tests/fixtures/*.json`.
3. Model + `snapshot::build` + `DiskKind` classification + free-region computation, unit-tested against the fixtures.
4. Watcher with debounce. `statvfs` usage.
5. Sidebar (Internal / External / Disk Images), Overview, Disk page, `partition_map`, `usage_meter`, Details section.
6. crowntest snapshots of every page using `fixtures::populate`, the settings pattern since `cx.spawn` doesn't run in tests.

**M2 — Everyday operations**
- Operation runner, Job tracking, `progress_sheet`, error mapping, toasts.
- Mount, Unmount, Open in Files, Rename (with per-fs label rules: FAT 11 chars, ext4 16 bytes, exFAT 15, NTFS 32, XFS 12, Btrfs 255), Unlock/Lock, Eject/Power Off.
- Verify the polkit prompt flow end-to-end with `crownos-polkit-agent`.

**M3 — Health**
- `udisks2::ata` + `udisks2::nvme::controller::ControllerProxy`, `SmartUpdate` throttling. No self-tests.
- `HealthStatus` derivation (unit-tested table), header pill, Health section, plain-language copy.

**M4 — Destructive operations I**
- `guard.rs` (system disk, live medium, busy targets) with exhaustive fixture tests.
- Fingerprint revalidation, tiered `confirm_sheet`, table snapshots, inhibitor, journal skeleton.
- Format volume (format picker driven by `CanFormat`), Erase disk (GPT default, MBR option), New Partition in free space, Delete Partition, Check & Repair.
- `encryption_fields` component and LUKS2 encrypt-on-format in all three dialogs.
- Loop-device integration tests for: encrypt, lock, unlock, then mount.

**M5 — Resize**
- `limits.rs` (bounds + `CanResize` matrix), `byte_size_field`, Resize sheet.
- Ordered shrink/grow steps (fs ↔ LUKS ↔ partition) with journal steps, temporary mount for btrfs/measurement.
- Inline unlock + LUKS2 passphrase in the Resize sheet.
- Loop-device tests for every layer combination, including interrupting between steps and resuming.
- Recovery callouts for interrupted resizes.

**M6 — Disk images**
- Portal file chooser.
- Open Image (`LoopSetup`, read-only), Create Image (sparse raw / zstd / xz), Restore Image (raw / iso / xz / zst, BLAKE3 verify, GPT backup relocation).
- Close-window guard.

**M7 — Polish and release**
- Keyboard navigation of the map and full semantics labels.
- Empty, error and service-unavailable states.
- Background-completion notifications via crownotify.
- `.desktop` file, PKGBUILD in crownos-pkgbuilds.

**Testing strategy across milestones:**
- **Pure layers** (`snapshot`, `guard`, `limits`, health, label rules) are unit-tested from recorded fixtures. This is where the safety rules live, so it is where test effort goes.
- **UI**: crowntest semantic + layout snapshots of every page and every dialog in each state (idle, busy, error, disabled-with-reason).
- **Real-world**: `#[ignore]` integration tests that create a sparse file, attach it with `Manager.LoopSetup`, and run the real operations against it: partition, format each fs, grow/shrink, image round-trip with hash comparison. Loop devices give a real UDisks target that can't touch a real disk. Run these in a VM / CI only.

---

## 9. Resolved decisions (2026-11-04)

- **zbus 6 + `udisks2` 0.4.** Verified on docs.rs: it provides `nvme::controller::ControllerProxy` (SMART), `BlockProxy::open_device → OwnedFd` and `EncryptedProxy::resize`. The portal and logind proxies are written locally on zbus 6, because `ashpd` 0.13 is still on zbus 5.
- **Encryption in v1:** encrypt-on-format (LUKS2) and resizing encrypted volumes, on top of unlock/lock.
- **SMART self-tests:** left out.

### Remaining risks to check during implementation
- `udisks2` enables zbus default features (async-io executor) and pulls `gettext-rs`. Confirm the binary uses tokio only and measure the size and thread cost in M0.
- CrownUI window-close interception (§3.3) is needed before M6.
