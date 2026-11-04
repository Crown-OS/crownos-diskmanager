use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::os::unix::fs::{FileExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use super::copy::{ChunkSink, Transfer};
use super::device::{self, DeviceWriter, Verification};
use super::format::{ImageFile, ImageFormat};
use super::save::save_image;
use super::target::TargetLayout;
use super::write::{RestorePlan, verify, write_image};
use crate::command::{Compression, Event, OperationId};
use crate::error::StorageError;
use crate::operations::OperationContext;
use crate::util::partial_file::partial_path;

const TEST_BUFFER: usize = 128 << 10;
const SECTOR: u64 = 512;

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(name: &str) -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "crownos-image-{name}-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&path)?;
        Ok(Self(path))
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

fn silent_context() -> OperationContext {
    OperationContext::new(OperationId(1), Arc::new(|_event| {}))
}

fn recording_context() -> (OperationContext, Arc<Mutex<Vec<Event>>>) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink_events = Arc::clone(&events);
    let context = OperationContext::new(
        OperationId(2),
        Arc::new(move |event| sink_events.lock().unwrap().push(event)),
    );
    (context, events)
}

/// Patterned data with a long zero run in the middle and an odd tail, spanning many buffers.
fn sample_disk() -> Vec<u8> {
    let len = 10 * TEST_BUFFER + 4321;
    let mut data: Vec<u8> = (0..len).map(|index| (index * 31 % 251) as u8).collect();
    data[2 * TEST_BUFFER..6 * TEST_BUFFER].fill(0);
    data
}

fn write_file(path: &Path, data: &[u8]) -> io::Result<()> {
    File::create(path)?.write_all(data)
}

fn device_file(path: &Path, len: u64) -> io::Result<File> {
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)?;
    file.set_len(len)?;
    Ok(file)
}

fn decompress(path: &Path) -> io::Result<(ImageFormat, Option<u64>, Vec<u8>)> {
    let image = ImageFile::open(path)?;
    let (format, size) = (image.format, image.decompressed_size);
    let mut data = Vec::new();
    image.into_reader()?.read_to_end(&mut data)?;
    Ok((format, size, data))
}

#[test]
fn raw_images_are_sparse_and_identical() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("sparse")?;
    let data = sample_disk();
    let destination = scratch.join("disk.img");
    save_image(
        data.as_slice(),
        data.len() as u64,
        &destination,
        Compression::None,
        &silent_context(),
        TEST_BUFFER,
    )?;

    assert_eq!(fs::read(&destination)?, data);
    let allocated = fs::metadata(&destination)?.blocks() * SECTOR;
    assert!(
        allocated < data.len() as u64,
        "{allocated} bytes allocated for {}",
        data.len()
    );
    assert!(!partial_path(&destination).exists());
    Ok(())
}

#[test]
fn trailing_zeros_keep_the_full_length() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("tail")?;
    let mut data = sample_disk();
    let len = data.len();
    data[len - 3 * TEST_BUFFER..].fill(0);
    let destination = scratch.join("disk.img");
    save_image(
        data.as_slice(),
        len as u64,
        &destination,
        Compression::None,
        &silent_context(),
        TEST_BUFFER,
    )?;
    assert_eq!(fs::read(&destination)?, data);
    Ok(())
}

#[test]
fn zstd_round_trip_records_the_size() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("zstd")?;
    let data = sample_disk();
    let destination = scratch.join("disk.img.zst");
    save_image(
        data.as_slice(),
        data.len() as u64,
        &destination,
        Compression::Zstd,
        &silent_context(),
        TEST_BUFFER,
    )?;

    let (format, size, restored) = decompress(&destination)?;
    assert_eq!(format, ImageFormat::Zstd);
    assert_eq!(size, Some(data.len() as u64));
    assert_eq!(restored, data);
    Ok(())
}

#[test]
fn xz_round_trip_reads_the_size_from_the_index() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("xz")?;
    let data = sample_disk();
    let destination = scratch.join("disk.img.xz");
    save_image(
        data.as_slice(),
        data.len() as u64,
        &destination,
        Compression::Xz,
        &silent_context(),
        TEST_BUFFER,
    )?;

    let (format, size, restored) = decompress(&destination)?;
    assert_eq!(format, ImageFormat::Xz);
    assert_eq!(size, Some(data.len() as u64));
    assert_eq!(restored, data);
    Ok(())
}

#[test]
fn raw_files_report_their_length() -> io::Result<()> {
    let scratch = ScratchDir::new("raw")?;
    let path = scratch.join("ubuntu.iso");
    write_file(&path, b"CD001 not really an iso")?;
    let image = ImageFile::open(&path)?;
    assert_eq!(image.format, ImageFormat::Raw);
    assert_eq!(image.decompressed_size, Some(23));
    Ok(())
}

#[test]
fn restore_writes_and_verifies() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("restore")?;
    let data = sample_disk();
    let image_path = scratch.join("disk.img.zst");
    save_image(
        data.as_slice(),
        data.len() as u64,
        &image_path,
        Compression::Zstd,
        &silent_context(),
        TEST_BUFFER,
    )?;
    let device = device_file(&scratch.join("device"), 2 * data.len() as u64)?;
    let plan = RestorePlan {
        verification: Verification::ReadBack,
        layout: TargetLayout::Partition,
        buffer_size: TEST_BUFFER,
    };
    let (context, events) = recording_context();

    let written = write_image(ImageFile::open(&image_path)?, &device, plan, &context)?;

    assert_eq!(written, data.len() as u64);
    let mut on_device = vec![0; data.len()];
    device.read_exact_at(&mut on_device, 0)?;
    assert_eq!(on_device, data);
    let phases: Vec<String> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            Event::OperationProgress { phase, .. } => Some(phase.clone()),
            _ => None,
        })
        .collect();
    assert!(phases.iter().any(|phase| phase == "Writing"));
    assert!(phases.iter().any(|phase| phase == "Verifying"));
    Ok(())
}

#[test]
fn oversized_images_are_refused_before_writing() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("oversize")?;
    let image_path = scratch.join("disk.img");
    write_file(&image_path, &sample_disk())?;
    let device = device_file(&scratch.join("device"), TEST_BUFFER as u64)?;
    let plan = RestorePlan {
        verification: Verification::Skip,
        layout: TargetLayout::WholeDisk,
        buffer_size: TEST_BUFFER,
    };

    let result = write_image(
        ImageFile::open(&image_path)?,
        &device,
        plan,
        &silent_context(),
    );

    assert!(matches!(result, Err(StorageError::ImageTooLarge { .. })));
    assert!(device.metadata()?.blocks() == 0);
    Ok(())
}

#[test]
fn writes_past_the_end_are_refused_when_the_size_is_unknown() -> io::Result<()> {
    let scratch = ScratchDir::new("past-end")?;
    let device = device_file(&scratch.join("device"), 1000)?;
    let mut writer = DeviceWriter::new(&device, 1000, Verification::Skip);
    assert!(writer.write_chunk(&[1; 600]).is_ok());
    assert!(matches!(
        writer.write_chunk(&[1; 600]),
        Err(StorageError::ImageTooLarge { .. })
    ));
    Ok(())
}

#[test]
fn verification_detects_a_corrupted_byte() -> Result<(), StorageError> {
    let scratch = ScratchDir::new("verify")?;
    let data = sample_disk();
    let device = device_file(&scratch.join("device"), data.len() as u64)?;
    let mut writer = DeviceWriter::new(&device, data.len() as u64, Verification::ReadBack);
    writer.write_chunk(&data)?;
    let digest = writer.digest().ok_or(StorageError::VerificationFailed)?;
    device::flush(&device, data.len() as u64)?;
    let context = silent_context();

    assert!(verify(&device, data.len() as u64, digest, &context, TEST_BUFFER).is_ok());
    device.write_all_at(&[0xAA], 7 * TEST_BUFFER as u64 + 5)?;
    let result = verify(&device, data.len() as u64, digest, &context, TEST_BUFFER);
    assert!(matches!(result, Err(StorageError::VerificationFailed)));
    Ok(())
}

struct CancelAfterFirstRead<'a> {
    data: &'a [u8],
    context: &'a OperationContext,
}

impl Read for CancelAfterFirstRead<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let read = self.data.read(buffer)?;
        self.context.cancel();
        Ok(read)
    }
}

#[test]
fn cancelling_mid_copy_leaves_no_partial_file() -> io::Result<()> {
    let scratch = ScratchDir::new("cancel")?;
    let data = sample_disk();
    let destination = scratch.join("disk.img.zst");
    let context = silent_context();
    let source = CancelAfterFirstRead {
        data: &data,
        context: &context,
    };

    let result = save_image(
        source,
        data.len() as u64,
        &destination,
        Compression::Zstd,
        &context,
        TEST_BUFFER,
    );

    assert!(matches!(result, Err(StorageError::Cancelled)));
    assert!(!destination.exists());
    assert!(!partial_path(&destination).exists());
    Ok(())
}

#[test]
fn transfers_count_every_byte_across_buffers() -> Result<(), StorageError> {
    let data = sample_disk();
    let context = silent_context();
    let transfer = Transfer {
        context: &context,
        phase: "Copying",
        expected_bytes: None,
        buffer_size: TEST_BUFFER,
    };
    let mut copied = Vec::new();
    assert_eq!(
        transfer.run(data.as_slice(), &mut copied)?,
        data.len() as u64
    );
    assert_eq!(copied, data);
    Ok(())
}

#[test]
fn smaller_gpt_images_get_their_backup_header_moved_to_the_end()
-> Result<(), Box<dyn std::error::Error>> {
    let scratch = ScratchDir::new("gpt")?;
    let image_len = 2048 * SECTOR;
    let image_path = scratch.join("gpt.img");
    let mut image = device_file(&image_path, image_len)?;
    let mut table = gptman::GPT::new_from(&mut image, SECTOR, [7; 16])?;
    gptman::GPT::write_protective_mbr_into(&mut image, SECTOR)?;
    table.write_into(&mut image)?;
    drop(image);
    let device_len = 8192 * SECTOR;
    let device = device_file(&scratch.join("device"), device_len)?;
    let plan = RestorePlan {
        verification: Verification::ReadBack,
        layout: TargetLayout::WholeDisk,
        buffer_size: TEST_BUFFER,
    };

    write_image(
        ImageFile::open(&image_path)?,
        &device,
        plan,
        &silent_context(),
    )?;

    let mut device_io = &device;
    let relocated = gptman::GPT::find_from(&mut device_io)?;
    assert_eq!(relocated.header.backup_lba, device_len / SECTOR - 1);
    let mut signature = [0; 8];
    device.read_exact_at(&mut signature, device_len - SECTOR)?;
    assert_eq!(&signature, b"EFI PART");
    Ok(())
}
