use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

lazy_static! {
    static ref RUSTY_BUFFERS: Arc<Mutex<RustyBuffers>> =
        Arc::new(Mutex::new(RustyBuffers::new()));
}

type Result<T> = core::result::Result<T, RBError>;

#[derive(Debug)]
#[repr(u8)]
enum RBError {
    NoBufferAvailable = 1,
    SizeTooBig = 2,
    InvalidPointer = 3,
}

impl fmt::Display for RBError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let as_str = match self {
            Self::NoBufferAvailable => "No Buffer Available",
            Self::SizeTooBig => "Size Too Big",
            Self::InvalidPointer => "Invalid Pointer",
        };
        write!(f, "{}", as_str)
    }
}

struct RBEntry {
    data: Box<[u8]>,
}

impl RBEntry {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size].into_boxed_slice(),
        }
    }
}

struct RustyBuffers {
    available: Vec<RBEntry>,
    in_use: HashMap<u64, RBEntry>,
    max_buffers: usize,
    max_buffer_size: usize,
}

impl RustyBuffers {
    fn new() -> Self {
        Self {
            available: Vec::new(),
            in_use: HashMap::new(),
            max_buffers: 1,
            max_buffer_size: 1024,
        }
    }

    fn acquire(&mut self, size: usize) -> Result<*mut std::ffi::c_uchar> {
        println!("[Rust]: Acquiring Minimum Bytes: {}", size);

        let mut buffer = if !self.available.is_empty() {
            // We can unwrap unconditionally here since we've just asserted
            // that the self.available is not empty.
            self.available.pop().unwrap()
        } else if self.available.len() + self.in_use.len() <= self.max_buffers {
            RBEntry::new(size)
        } else {
            return Err(RBError::NoBufferAvailable);
        };

        if buffer.data.len() < size {
            buffer.data = vec![0; size].into_boxed_slice()
        }

        let ptr = buffer.data.as_ptr() as *mut std::ffi::c_uchar;
        let buff_id = ptr as u64;
        self.in_use.insert(buff_id, buffer);

        println!("[Rust]: Acquired: {:p}", ptr);
        Ok(ptr)
    }

    fn release(&mut self, data: *mut std::ffi::c_uchar) -> Result<()> {
        println!("[Rust]: Released: {:p}", data);

        let buff_id = data as u64;
        let buff = self.in_use.remove(&buff_id);

        if buff.is_none() {
            return Err(RBError::InvalidPointer);
        }

        self.available.push(buff.unwrap());

        Ok(())
    }
}

fn rustybuffer_acquire_impl(
    size: std::ffi::c_ulonglong,
    data: *mut *mut std::ffi::c_uchar,
) -> Result<()> {
    let size = size as usize;

    // Idiomatic Rust locking unwraps unconditionally because if we fail here
    // its due to the lock being poisoned which means this process is toast.
    let mut rb = RUSTY_BUFFERS.lock().unwrap();

    if size > rb.max_buffer_size {
        return Err(RBError::SizeTooBig);
    }

    let res = rb.acquire(size)?;
    unsafe {
        *data = res;
    }

    Ok(())
}

fn rustybuffer_release_impl(data: *mut std::ffi::c_uchar) -> Result<()> {
    let mut rb = RUSTY_BUFFERS.lock().unwrap();
    rb.release(data)?;
    Ok(())
}

#[no_mangle]
pub extern "C" fn rustybuffer_acquire(
    size: std::ffi::c_ulonglong,
    data: *mut *mut std::ffi::c_uchar,
) -> std::ffi::c_uchar {
    handle_result(rustybuffer_acquire_impl(size, data))
}

#[no_mangle]
pub extern "C" fn rustybuffer_release(
    data: *mut std::ffi::c_uchar,
) -> std::ffi::c_uchar {
    handle_result(rustybuffer_release_impl(data))
}

fn handle_result(res: Result<()>) -> std::ffi::c_uchar {
    if res.is_ok() {
        0
    } else {
        res.err().unwrap() as std::ffi::c_uchar
    }
}
