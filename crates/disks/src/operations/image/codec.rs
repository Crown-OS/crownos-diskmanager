use std::fs::File;
use std::io::{self, BufReader, Read};
use std::num::NonZero;
use std::thread;

use liblzma::read::XzDecoder;
use liblzma::stream::{Check, MtStreamBuilder};
use liblzma::write::XzEncoder;

const ZSTD_LEVEL: i32 = 3;
const ZSTD_MAX_WINDOW_LOG: u32 = 31;
const XZ_PRESET: u32 = 6;
const XZ_MEMORY_BUDGET: u64 = 512 << 20;
const XZ_DECODER_MEMORY_LIMIT: u64 = 1 << 30;
const COMPRESSED_READ_BUFFER: usize = 1 << 20;

pub(super) fn zstd_encoder(
    file: &File,
    source_size: u64,
) -> io::Result<zstd::Encoder<'static, &File>> {
    let mut encoder = zstd::Encoder::new(file, ZSTD_LEVEL)?;
    encoder.multithread(worker_count())?;
    encoder.include_contentsize(true)?;
    encoder.set_pledged_src_size(Some(source_size))?;
    Ok(encoder)
}

pub(super) fn xz_encoder(file: &File) -> io::Result<XzEncoder<&File>> {
    let mut builder = MtStreamBuilder::new();
    builder.preset(XZ_PRESET).check(Check::Crc64);
    let threads = (1..=worker_count())
        .rev()
        .find(|&threads| builder.threads(threads).memusage() <= XZ_MEMORY_BUDGET)
        .unwrap_or(1);
    let stream = builder
        .threads(threads)
        .encoder()
        .map_err(io::Error::other)?;
    Ok(XzEncoder::new_stream(file, stream))
}

pub(super) fn zstd_decoder(file: File) -> io::Result<impl Read + Send> {
    let mut decoder =
        zstd::Decoder::with_buffer(BufReader::with_capacity(COMPRESSED_READ_BUFFER, file))?;
    decoder.window_log_max(ZSTD_MAX_WINDOW_LOG)?;
    Ok(decoder)
}

pub(super) fn xz_decoder(file: File) -> io::Result<impl Read + Send> {
    let stream = MtStreamBuilder::new()
        .threads(worker_count())
        .memlimit_threading(XZ_DECODER_MEMORY_LIMIT)
        .memlimit_stop(u64::MAX)
        .decoder()
        .map_err(io::Error::other)?;
    Ok(XzDecoder::new_stream(
        BufReader::with_capacity(COMPRESSED_READ_BUFFER, file),
        stream,
    ))
}

fn worker_count() -> u32 {
    thread::available_parallelism()
        .map_or(1, NonZero::get)
        .try_into()
        .unwrap_or(u32::MAX)
}
