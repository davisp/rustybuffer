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
    bytes_allocated: usize,
    bytes_in_use: usize,
    max_total_size: usize,
    max_buffer_size: usize,
}

impl RustyBuffers {
    fn new() -> Self {
        Self {
            available: Vec::new(),
            in_use: HashMap::new(),
            bytes_allocated: 0,
            bytes_in_use: 0,
            max_total_size: 1024 * 1024 * 1024, // 1 GiB
            max_buffer_size: 10 * 1024 * 1024,  // 10MiB
        }
    }

    fn configure(
        &mut self,
        max_total_size: usize,
        max_buffer_size: usize,
    ) -> Result<()> {
        self.max_total_size = max_total_size;
        self.max_buffer_size = max_buffer_size;

        println!(
            "[Rust]: Max Total Size: {} Max Buffer Size: {}",
            self.max_total_size, self.max_buffer_size
        );

        Ok(())
    }

    fn acquire(&mut self, size: usize) -> Result<*mut std::ffi::c_uchar> {
        println!("[Rust]: Acquiring Minimum Bytes: {}", size);

        if size > self.max_buffer_size {
            return Err(RBError::SizeTooBig);
        }

        let mut buffer = if !self.available.is_empty() {
            // We can unwrap unconditionally here since we've just asserted
            // that the self.available is not empty.
            self.available.pop().unwrap()
        } else if self.can_allocate(size) {
            let buf = RBEntry::new(size);
            self.bytes_allocated += size;
            buf
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

    fn can_allocate(&mut self, size: usize) -> bool {
        if self.bytes_allocated + size <= self.max_total_size {
            return true;
        }

        let can_free = self.bytes_allocated - self.bytes_in_use;
        if can_free + size <= self.max_total_size {
            let free_at_least =
                size - (self.max_total_size - self.bytes_allocated);
            assert!(free_at_least <= can_free);

            // Gonna setup configuration before I go further with this.
            // free enough space
            // return true;
        }

        false
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

fn rustybuffer_config_impl(
    max_total_size: std::ffi::c_ulonglong,
    max_buffer_size: std::ffi::c_ulonglong,
) -> Result<()> {
    let mut rb = RUSTY_BUFFERS.lock().unwrap();
    rb.configure(max_total_size as usize, max_buffer_size as usize)?;
    Ok(())
}

fn rustybuffer_acquire_impl(
    size: std::ffi::c_ulonglong,
    data: *mut *mut std::ffi::c_uchar,
) -> Result<()> {
    // Idiomatic Rust locking unwraps unconditionally because if we fail here
    // its due to the lock being poisoned which means this process is toast.
    let mut rb = RUSTY_BUFFERS.lock().unwrap();
    let res = rb.acquire(size as usize)?;
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
pub extern "C" fn rustybuffer_config(
    max_total_size: std::ffi::c_ulonglong,
    max_buffer_size: std::ffi::c_ulonglong,
) -> std::ffi::c_uchar {
    handle_result(rustybuffer_config_impl(max_total_size, max_buffer_size))
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
