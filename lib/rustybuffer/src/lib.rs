use std::collections::{BTreeSet, HashMap};
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
    buffers: HashMap<u64, RBEntry>,
    available: BTreeSet<(usize, u64)>,
    bytes_allocated: usize,
    bytes_in_use: usize,
    max_total_size: usize,
    max_buffer_size: usize,
}

impl RustyBuffers {
    fn new() -> Self {
        Self {
            buffers: HashMap::new(),
            available: BTreeSet::new(),
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

        // println!(
        //     "[Rust]: Max Total Size: {} Max Buffer Size: {}",
        //     self.max_total_size, self.max_buffer_size
        // );

        Ok(())
    }

    fn acquire(&mut self, size: usize) -> Result<*mut std::ffi::c_uchar> {
        //println!("[Rust]: Acquiring Minimum Bytes: {}", size);

        if size > self.max_buffer_size {
            return Err(RBError::SizeTooBig);
        }

        // First the easy case when we have an existing buffer that can handle
        // the request.
        if !self.available.is_empty() {
            // Search for the smallest buffer that can hold size bytes
            let buffer = self.available.range((size, 0)..).next();
            if buffer.is_some() {
                let (buff_size, buff_id) = *buffer.expect("Rust is broken.");
                self.bytes_in_use += buff_size;
                assert!(self.available.remove(&(buff_size, buff_id)));
                let buffer = self
                    .buffers
                    .get_mut(&buff_id)
                    .expect("Available buffers out of sync with buffer list");
                buffer.data.fill(0);
                return Ok(buffer.data.as_ptr() as *mut std::ffi::c_uchar);
            };
        }

        // Next, see if we can allocate a new buffer for this request.
        if self.can_allocate(size) {
            let buffer = RBEntry::new(size);
            let ret = buffer.data.as_ptr() as *mut std::ffi::c_uchar;
            let buff_id = ret as u64;

            self.bytes_allocated += size;
            self.bytes_in_use += size;
            assert!(self.bytes_allocated <= self.max_total_size);
            assert!(self.bytes_in_use <= self.bytes_allocated);

            self.buffers.insert(buff_id, buffer);

            // No need to zero here as we created this buffer fresh which
            // automatically zeros the buffer.
            return Ok(ret);
        }

        Err(RBError::NoBufferAvailable)
    }

    /// Check if its possible to allocate a buffer of the given size. If
    /// available buffers (i.e., buffers not being used) need to be freed to
    /// keep us under the total maximum bytes, they will be freed by this
    /// method.
    fn can_allocate(&mut self, size: usize) -> bool {
        if self.bytes_allocated + size <= self.max_total_size {
            return true;
        }

        let can_free = self.bytes_allocated - self.bytes_in_use;
        if self.bytes_allocated - can_free + size > self.max_total_size {
            return false;
        }

        let free_at_least = size - (self.max_total_size - self.bytes_allocated);
        assert!(free_at_least <= can_free);
        assert!(!self.available.is_empty());

        // This is freeing the largest and then smallest buffers we have
        // available until we have freed enough space. The vague idea behind
        // using max/min is that we'll have some "average" buffer size and
        // this way we remove the outliers without affecting the commonly used
        // buffers.
        //
        // I have no idea if this will work, but it sounds nice in theory.
        let mut bytes_freed = 0;
        while bytes_freed < free_at_least {
            let (buff_size, buff_id) = self.available.pop_last().expect(
                "Not enough buffers to free (large), broken pre-condition",
            );
            self.buffers
                .remove(&buff_id)
                .expect("Unknown buffer id in available.");
            bytes_freed += buff_size;

            if bytes_freed >= free_at_least {
                break;
            }

            let (buff_size, buff_id) = self.available.pop_first().expect(
                "Not enough buffers to free (small), broken pre-condition",
            );
            self.buffers
                .remove(&buff_id)
                .expect("Unknown buffer id in available.");
            bytes_freed += buff_size;
        }

        self.bytes_allocated -= bytes_freed;
        assert!(self.bytes_in_use <= self.bytes_allocated);

        true
    }

    fn release(&mut self, data: *mut std::ffi::c_uchar) -> Result<()> {
        //println!("[Rust]: Released: {:p}", data);
        let buff_id = data as u64;

        let buffer = self.buffers.get(&buff_id);
        if buffer.is_none() {
            return Err(RBError::InvalidPointer);
        }

        let buff_size = buffer.unwrap().data.len();
        self.available.insert((buff_size, buff_id));
        self.bytes_in_use -= buff_size;

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
    let mut rb = RUSTY_BUFFERS.lock().expect("Mutex was poisoned.");
    let res = rb.acquire(size as usize)?;
    unsafe {
        *data = res;
    }

    Ok(())
}

fn rustybuffer_release_impl(data: *mut std::ffi::c_uchar) -> Result<()> {
    let mut rb = RUSTY_BUFFERS.lock().expect("Mutex was poisoned.");
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
        res.expect_err("Rust is broken.") as std::ffi::c_uchar
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    use rand::distributions::Distribution;
    use rand::Rng;

    // The test strategy here is to configure 1GiB max total size, and then
    // have 10 threads attempt to hold on to 100MiB of buffers so that we're
    // allocating and deallocating around the threshold lots.
    #[test]
    fn check_allocation_test() {
        // Configure the buffers
        rustybuffer_config(1024 * 1024 * 1024, 15 * 1024 * 1024);
        let mut handles = Vec::new();

        for _ in 0..10 {
            let handle = thread::spawn(run_thread);
            handles.push(handle);
        }

        let mut acquired: usize = 0;
        let mut acquire_success: u64 = 0;
        let mut acquire_failures: u64 = 0;
        let mut total_releases: u64 = 0;

        for handle in handles.into_iter() {
            let result = handle.join().expect("Thread failed.");
            acquired += result.0;
            acquire_success += result.1;
            acquire_failures += result.2;
            total_releases += result.3;
        }

        println!("Bytes Acquired: {}", acquired);
        println!("Successful Acquires: {}", acquire_success);
        println!("Failed Acquires: {}", acquire_failures);
        println!("Total Releases: {}", total_releases);
    }

    enum Action {
        Acquire,
        Release,
    }

    fn run_thread() -> (usize, u64, u64, u64) {
        let mut total_acquired: usize = 0;
        let mut acquire_successes: u64 = 0;
        let mut acquire_failures: u64 = 0;
        let mut total_releases: u64 = 0;

        let mut rng = rand::thread_rng();
        let size_rng = rand::distributions::Uniform::new_inclusive(
            5 * 1024 * 1024,
            16 * 1025 * 1024,
        );

        let mut allocations: Vec<*mut std::ffi::c_uchar> = Vec::new();

        for _ in 0..100_000 {
            // We're trying to keep between 10 and 15 allocations at all
            // times.
            let action = if allocations.len() < 10 {
                Action::Acquire
            } else if allocations.len() > 15 {
                Action::Release
            } else if rng.gen_bool(0.5) {
                Action::Acquire
            } else {
                Action::Release
            };

            match action {
                Action::Acquire => {
                    let size: std::ffi::c_ulonglong = size_rng.sample(&mut rng);
                    let mut ptr: *mut std::ffi::c_uchar =
                        std::ptr::null_mut::<std::ffi::c_uchar>();
                    let res = rustybuffer_acquire(size, &mut ptr);
                    if res == 0 {
                        allocations.push(ptr);
                        total_acquired += size as usize;
                        acquire_successes += 1;
                    } else {
                        acquire_failures += 1;
                    }
                }
                Action::Release => {
                    let idx = rng.gen_range(0..allocations.len());
                    let ptr = allocations.remove(idx);
                    assert!(rustybuffer_release(ptr) == 0);
                    total_releases += 1;
                }
            }
        }

        (
            total_acquired,
            acquire_successes,
            acquire_failures,
            total_releases,
        )
    }
}
