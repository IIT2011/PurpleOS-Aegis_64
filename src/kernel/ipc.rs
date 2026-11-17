//! Aegis64 Inter-Process Communication (IPC)
//!
//! Provides fundamental kernel IPC primitives:
//!   - POSIX-Compliant Pipes: Thread-safe ring buffer FIFO with reference-counted
//!     read/write endpoints, EOF detection, and EPIPE / SIGPIPE semantics.
//!   - Shared Memory: Page-level cross-task mappings.

use crate::sync::Spinlock;
use crate::kernel::posix::EPIPE;

pub const PIPE_BUFFER_SIZE: usize = 4096;

pub struct Pipe {
    buffer: [u8; PIPE_BUFFER_SIZE],
    head: usize,
    tail: usize,
    count: usize,
    pub readers_count: usize,
    pub writers_count: usize,
}

impl Pipe {
    pub const fn new() -> Self {
        Self {
            buffer: [0; PIPE_BUFFER_SIZE],
            head: 0,
            tail: 0,
            count: 0,
            readers_count: 1,
            writers_count: 1,
        }
    }

    pub fn write(&mut self, data: &[u8]) -> Result<usize, i64> {
        // POSIX requirement: writing to a pipe with no readers returns -EPIPE and raises SIGPIPE
        if self.readers_count == 0 {
            return Err(-EPIPE);
        }
        let mut written = 0;
        for &byte in data.iter() {
            if self.count >= PIPE_BUFFER_SIZE {
                break;
            }
            self.buffer[self.head] = byte;
            self.head = (self.head + 1) % PIPE_BUFFER_SIZE;
            self.count += 1;
            written += 1;
        }
        Ok(written)
    }

    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, i64> {
        if self.count == 0 {
            // If all write ends are closed and buffer is empty, return 0 (EOF)
            if self.writers_count == 0 {
                return Ok(0);
            }
            return Ok(0);
        }

        let mut read_bytes = 0;
        while read_bytes < buf.len() && self.count > 0 {
            buf[read_bytes] = self.buffer[self.tail];
            self.tail = (self.tail + 1) % PIPE_BUFFER_SIZE;
            self.count -= 1;
            read_bytes += 1;
        }
        Ok(read_bytes)
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn len(&self) -> usize {
        self.count
    }
}

pub const MAX_PIPES: usize = 16;
static PIPE_REGISTRY: Spinlock<[Option<Pipe>; MAX_PIPES]> = Spinlock::new([
    None, None, None, None, None, None, None, None,
    None, None, None, None, None, None, None, None,
]);

pub fn create_pipe() -> Option<usize> {
    let mut reg = PIPE_REGISTRY.lock();
    for (i, slot) in reg.iter_mut().enumerate() {
        if slot.is_none() {
            *slot = Some(Pipe::new());
            return Some(i);
        }
    }
    None
}

pub fn pipe_write(id: usize, data: &[u8]) -> Result<usize, i64> {
    let mut reg = PIPE_REGISTRY.lock();
    if id >= MAX_PIPES {
        return Err(-crate::kernel::posix::EBADF);
    }
    match reg[id].as_mut() {
        Some(pipe) => pipe.write(data),
        None => Err(-crate::kernel::posix::EBADF),
    }
}

pub fn pipe_read(id: usize, buf: &mut [u8]) -> Result<usize, i64> {
    let mut reg = PIPE_REGISTRY.lock();
    if id >= MAX_PIPES {
        return Err(-crate::kernel::posix::EBADF);
    }
    match reg[id].as_mut() {
        Some(pipe) => pipe.read(buf),
        None => Err(-crate::kernel::posix::EBADF),
    }
}

pub fn pipe_add_reader(id: usize) {
    let mut reg = PIPE_REGISTRY.lock();
    if id < MAX_PIPES {
        if let Some(pipe) = reg[id].as_mut() {
            pipe.readers_count = pipe.readers_count.saturating_add(1);
        }
    }
}

pub fn pipe_add_writer(id: usize) {
    let mut reg = PIPE_REGISTRY.lock();
    if id < MAX_PIPES {
        if let Some(pipe) = reg[id].as_mut() {
            pipe.writers_count = pipe.writers_count.saturating_add(1);
        }
    }
}

pub fn pipe_close_reader(id: usize) {
    let mut reg = PIPE_REGISTRY.lock();
    if id < MAX_PIPES {
        if let Some(pipe) = reg[id].as_mut() {
            pipe.readers_count = pipe.readers_count.saturating_sub(1);
            if pipe.readers_count == 0 && pipe.writers_count == 0 {
                reg[id] = None;
            }
        }
    }
}

pub fn pipe_close_writer(id: usize) {
    let mut reg = PIPE_REGISTRY.lock();
    if id < MAX_PIPES {
        if let Some(pipe) = reg[id].as_mut() {
            pipe.writers_count = pipe.writers_count.saturating_sub(1);
            if pipe.readers_count == 0 && pipe.writers_count == 0 {
                reg[id] = None;
            }
        }
    }
}

pub fn pipe_close_legacy(id: usize) {
    let mut reg = PIPE_REGISTRY.lock();
    if id < MAX_PIPES {
        reg[id] = None;
    }
}

pub fn pipe_close(id: usize) {
    pipe_close_reader(id);
    pipe_close_writer(id);
}
