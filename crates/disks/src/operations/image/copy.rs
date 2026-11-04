use std::io::{self, Read};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::thread;
use std::time::Instant;

use super::progress::ProgressMeter;
use crate::error::StorageError;
use crate::operations::OperationContext;
use crate::util::aligned_buffer::AlignedBuffer;

pub(super) const BUFFER_SIZE: usize = 4 << 20;
const BUFFER_COUNT: usize = 3;

/// Fills a buffer as far as possible; a short fill marks the end of the data.
pub(super) trait ChunkSource {
    fn fill(&mut self, buffer: &mut [u8]) -> io::Result<usize>;
}

impl<R: Read> ChunkSource for R {
    fn fill(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut filled = 0;
        while filled < buffer.len() {
            match self.read(&mut buffer[filled..]) {
                Ok(0) => break,
                Ok(read) => filled += read,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) => return Err(error),
            }
        }
        Ok(filled)
    }
}

pub(super) trait ChunkSink {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), StorageError>;
}

impl<W: io::Write> ChunkSink for W {
    fn write_chunk(&mut self, chunk: &[u8]) -> Result<(), StorageError> {
        Ok(self.write_all(chunk)?)
    }
}

struct Chunk {
    buffer: AlignedBuffer,
    len: usize,
}

/// One streaming copy: a reader thread fills page-aligned buffers while the calling thread
/// drains them into the sink, so reading and writing overlap.
pub(super) struct Transfer<'a> {
    pub context: &'a OperationContext,
    pub phase: &'a str,
    pub expected_bytes: Option<u64>,
    pub buffer_size: usize,
}

impl Transfer<'_> {
    pub(super) fn run(
        &self,
        source: impl ChunkSource + Send,
        sink: &mut impl ChunkSink,
    ) -> Result<u64, StorageError> {
        let (free_sender, free_receiver) = mpsc::channel();
        let (filled_sender, filled_receiver) = mpsc::sync_channel(BUFFER_COUNT);
        let spare = (0..BUFFER_COUNT)
            .map(|_| AlignedBuffer::new(self.buffer_size))
            .collect();
        let context = self.context;
        thread::scope(|scope| {
            scope
                .spawn(move || read_chunks(source, spare, &free_receiver, &filled_sender, context));
            self.write_chunks(filled_receiver, free_sender, sink)
        })
    }

    fn write_chunks(
        &self,
        filled: Receiver<Result<Chunk, StorageError>>,
        free: Sender<AlignedBuffer>,
        sink: &mut impl ChunkSink,
    ) -> Result<u64, StorageError> {
        let mut meter = ProgressMeter::new(self.expected_bytes, Instant::now());
        let mut done = 0;
        for chunk in filled {
            let chunk = chunk?;
            self.context.ensure_not_cancelled()?;
            sink.write_chunk(&chunk.buffer[..chunk.len])?;
            done += chunk.len as u64;
            if let Some(progress) = meter.sample(done, Instant::now()) {
                self.context.report(self.phase, progress);
            }
            free.send(chunk.buffer).ok();
        }
        Ok(done)
    }
}

fn read_chunks(
    mut source: impl ChunkSource,
    mut spare: Vec<AlignedBuffer>,
    free: &Receiver<AlignedBuffer>,
    filled: &SyncSender<Result<Chunk, StorageError>>,
    context: &OperationContext,
) {
    while let Some(mut buffer) = spare.pop().or_else(|| free.recv().ok()) {
        let outcome = context
            .ensure_not_cancelled()
            .and_then(|()| source.fill(&mut buffer).map_err(StorageError::from));
        let message = match outcome {
            Ok(0) => return,
            Ok(len) => Ok(Chunk { buffer, len }),
            Err(error) => Err(error),
        };
        let last = !matches!(&message, Ok(chunk) if chunk.len == chunk.buffer.len());
        if filled.send(message).is_err() || last {
            return;
        }
    }
}
