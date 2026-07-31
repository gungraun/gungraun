//! Generates controlled minor page faults for Linux perf system tests.
//!
//! Linux backs a private anonymous mapping on demand. The first write to an untouched page makes
//! the kernel allocate and zero a physical page and install its page-table entry. Because this
//! requires no file-system I/O, [`getrusage(2)`] accounts it as a minor page fault.
//!
//! The methodology is:
//!
//! 1. Create a fresh private anonymous mapping with [`mmap(2)`]. A fresh mapping ensures that each
//!    requested page is initially untouched.
//! 2. Apply `MADV_NOHUGEPAGE` with [`madvise(2)`]. Transparent and multi-size huge pages can
//!    satisfy multiple base pages with one fault, so disabling them preserves the per-base-page
//!    behavior.
//! 3. Write one byte at every base-page offset. Volatile writes keep these otherwise unobservable
//!    stores from being removed by the compiler.
//! 4. Unmap the region explicitly so cleanup failures are reported and every invocation starts with
//!    a new mapping.
//!
//! Prefaulting mechanisms such as `MAP_POPULATE` are intentionally not used because the helper
//! needs the page accesses themselves to induce the faults.
//!
//! [`getrusage(2)`]: https://man7.org/linux/man-pages/man2/getrusage.2.html
//! [`madvise(2)`]: https://man7.org/linux/man-pages/man2/madvise.2.html
//! [`mmap(2)`]: https://man7.org/linux/man-pages/man2/mmap.2.html
//! [Transparent and multi-size huge pages]: https://docs.kernel.org/admin-guide/mm/transhuge.html

use std::io;
use std::ptr::NonNull;

use nix::libc;

struct AnonymousMapping {
    address: Option<NonNull<libc::c_void>>,
    length: usize,
}

impl AnonymousMapping {
    fn new(length: usize) -> io::Result<Self> {
        // SAFETY: Category 8 (FFI boundary). The requested mapping has no file descriptor or
        // caller-provided pointer, and `MAP_FAILED` is checked before the address is retained.
        let address = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                length,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if address == libc::MAP_FAILED {
            return Err(io::Error::last_os_error());
        }
        let Some(address) = NonNull::new(address) else {
            // SAFETY: Categories 8/12 (FFI boundary and valid free). A successful zero-address
            // mapping is live for exactly `length` bytes and cannot be represented by `NonNull`.
            let result = unsafe { libc::munmap(address, length) };
            return match result {
                0 => Err(io::Error::other("mmap returned address zero")),
                _ => Err(io::Error::last_os_error()),
            };
        };

        Ok(Self {
            address: Some(address),
            length,
        })
    }

    fn address(&self) -> NonNull<libc::c_void> {
        self.address
            .expect("a live anonymous mapping always has an address")
    }

    fn disable_huge_pages(&self) -> io::Result<()> {
        // SAFETY: Category 8 (FFI boundary). `address` is page-aligned and denotes the complete
        // live mapping of `length` bytes created by `mmap`.
        let result =
            unsafe { libc::madvise(self.address().as_ptr(), self.length, libc::MADV_NOHUGEPAGE) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }

        Ok(())
    }

    fn touch_pages(&mut self, page_size: usize) {
        let base = self.address().cast::<u8>().as_ptr();
        for offset in (0..self.length).step_by(page_size) {
            // SAFETY: Categories 10/11 (bounds and provenance). `base` comes from the live
            // mapping, every offset is below `length`, and writing one byte stays in bounds.
            unsafe { base.add(offset).write_volatile(0) };
        }
    }

    fn unmap(&mut self) -> io::Result<()> {
        let Some(address) = self.address else {
            return Ok(());
        };

        // SAFETY: Categories 8/12 (FFI boundary and valid free). This exact address and length
        // describe a live `mmap` allocation, and the address is cleared after successful release.
        let result = unsafe { libc::munmap(address.as_ptr(), self.length) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        self.address = None;
        Ok(())
    }

    fn close(mut self) -> io::Result<()> {
        self.unmap()
    }
}

impl Drop for AnonymousMapping {
    fn drop(&mut self) {
        let _ = self.unmap();
    }
}

/// Touches each page of a fresh anonymous mapping to induce minor page faults.
///
/// # Errors
///
/// Returns an error if `page_count` is zero, the mapping size overflows, or a Linux memory
/// operation fails.
pub fn cause_minor_page_faults(page_count: usize) -> io::Result<()> {
    if page_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "page count must be greater than zero",
        ));
    }

    // SAFETY: Category 8 (FFI boundary). `_SC_PAGESIZE` requires no pointer arguments and has no
    // side effects beyond querying the process's system configuration.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    let page_size = usize::try_from(page_size)
        .ok()
        .filter(|page_size| *page_size > 0)
        .ok_or_else(|| io::Error::other("failed to determine the system page size"))?;
    let length = page_count
        .checked_mul(page_size)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "mapping length overflowed"))?;

    let mut mapping = AnonymousMapping::new(length)?;
    mapping.disable_huge_pages()?;
    mapping.touch_pages(page_size);
    mapping.close()
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::mem::MaybeUninit;

    use nix::libc;

    use super::cause_minor_page_faults;

    fn minor_page_fault_count() -> io::Result<i64> {
        let mut usage = MaybeUninit::<libc::rusage>::uninit();
        // SAFETY: Category 8 (FFI boundary). `usage` points to writable storage for one
        // `rusage`, and the return value is checked before the initialized value is read.
        let result = unsafe { libc::getrusage(libc::RUSAGE_THREAD, usage.as_mut_ptr()) };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }

        // SAFETY: Category 4 (uninitialized memory). A successful `getrusage` call initialized
        // the complete `rusage` value at `usage`.
        let usage = unsafe { usage.assume_init() };
        Ok(usage.ru_minflt)
    }

    #[test]
    fn causes_at_least_one_minor_fault_per_requested_page() -> io::Result<()> {
        const PAGE_COUNT: usize = 256;
        let faults_before = minor_page_fault_count()?;

        cause_minor_page_faults(PAGE_COUNT)?;

        let faults_after = minor_page_fault_count()?;
        let expected_faults = i64::try_from(PAGE_COUNT).expect("page count fits in i64");
        assert!(faults_after - faults_before >= expected_faults);
        Ok(())
    }

    #[test]
    fn rejects_zero_pages() {
        let error = cause_minor_page_faults(0).expect_err("zero pages should be rejected");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn rejects_mapping_length_overflow() {
        let error =
            cause_minor_page_faults(usize::MAX).expect_err("mapping length should overflow");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
