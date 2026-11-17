//! Shared-memory frame slots.
//!
//! The app creates one region with a few slots, each big enough for the largest frame, and the
//! helper inherits it: on Unix as file descriptor 3 (a POSIX shared memory object unlinked the
//! moment it is open, so it has no name another process could open), on Windows as an inherited
//! handle to an unnamed file mapping. The helper paints into a free slot, names it in a
//! `Frame` message, and the app hands it back with `Release` once it has copied the pixels out.
//! Ownership of each slot passes back and forth through those messages, so no locks are needed.

use anyhow::{Context as _, Result, bail, ensure};
use std::process::Command;

const MAGIC: u64 = u64::from_le_bytes(*b"UDXSPIKE");
const VERSION: u32 = 1;
const HEADER_BYTES: usize = 4096;

#[repr(C)]
#[derive(Clone, Copy)]
struct Header {
    magic: u64,
    version: u32,
    slots: u32,
    slot_bytes: u64,
    max_width: u32,
    max_height: u32,
}

pub struct SharedFrames {
    base: *mut u8,
    len: usize,
    slots: u32,
    slot_bytes: usize,
    max_width: u32,
    max_height: u32,
    os: os::Region,
}

// The region is plain memory; which side may touch which slot is decided by the wire protocol.
unsafe impl Send for SharedFrames {}
unsafe impl Sync for SharedFrames {}

impl SharedFrames {
    /// Creates a region (app side) for `slots` frames of up to `max_width` by `max_height`
    /// device pixels.
    pub fn create(max_width: u32, max_height: u32, slots: u32) -> Result<Self> {
        ensure!((2..=8).contains(&slots), "between 2 and 8 slots");
        let slot_bytes = round_up(max_width as usize * max_height as usize * 4, 4096);
        let len = HEADER_BYTES + slot_bytes * slots as usize;
        let (os, base) = os::Region::create(len)?;
        let header = Header {
            magic: MAGIC,
            version: VERSION,
            slots,
            slot_bytes: slot_bytes as u64,
            max_width,
            max_height,
        };
        // SAFETY: the mapping is at least HEADER_BYTES long and page aligned.
        unsafe { (base as *mut Header).write(header) };
        Ok(Self {
            base,
            len,
            slots,
            slot_bytes,
            max_width,
            max_height,
            os,
        })
    }

    /// Arranges for `command`'s child to inherit the region and tells it how through arguments.
    pub fn share_with(&self, command: &mut Command) {
        self.os.share_with(command, self.len);
    }

    /// Opens the region inherited from the app (helper side), from the arguments
    /// [`SharedFrames::share_with`] added.
    pub fn open_inherited(args: &[String]) -> Result<Self> {
        let (os, base, len) = os::Region::open_inherited(args)?;
        ensure!(len >= HEADER_BYTES, "region too small");
        // SAFETY: the mapping is at least HEADER_BYTES long and page aligned.
        let header = unsafe { (base as *const Header).read() };
        ensure!(header.magic == MAGIC, "not a frame region");
        ensure!(header.version == VERSION, "unknown frame region version");
        let slot_bytes = header.slot_bytes as usize;
        ensure!(
            HEADER_BYTES + slot_bytes * header.slots as usize <= len,
            "header describes more than the region holds"
        );
        Ok(Self {
            base,
            len,
            slots: header.slots,
            slot_bytes,
            max_width: header.max_width,
            max_height: header.max_height,
            os,
        })
    }

    pub fn slots(&self) -> u32 {
        self.slots
    }

    pub fn max_size(&self) -> (u32, u32) {
        (self.max_width, self.max_height)
    }

    /// Checks a frame description from the peer before any pixel is touched.
    pub fn check(&self, slot: u32, width: u32, height: u32) -> Result<usize> {
        if slot >= self.slots {
            bail!("slot {slot} out of range");
        }
        if width == 0 || height == 0 || width > self.max_width || height > self.max_height {
            bail!(
                "frame size {width}x{height} outside 1..={}x{}",
                self.max_width,
                self.max_height
            );
        }
        Ok(width as usize * height as usize * 4)
    }

    /// Copies a frame out of `slot` (app side). The caller owns the slot until it releases it.
    pub fn copy_out(&self, slot: u32, width: u32, height: u32) -> Result<Vec<u8>> {
        let bytes = self.check(slot, width, height)?;
        let mut out = Vec::with_capacity(bytes);
        // SAFETY: `check` keeps the range inside the slot, and the slot is the app's until it
        // sends Release, so the helper is not writing it.
        unsafe {
            std::ptr::copy_nonoverlapping(self.slot_ptr(slot), out.as_mut_ptr(), bytes);
            out.set_len(bytes);
        }
        Ok(out)
    }

    /// The pixels of `slot` for painting (helper side), `width * height * 4` bytes.
    ///
    /// # Safety
    /// The caller must own the slot: it has not been sent to the app, or the app released it.
    // Shared memory: exclusive access comes from slot ownership, not from `&mut self`.
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn slot_mut(&self, slot: u32, width: u32, height: u32) -> Result<&mut [u8]> {
        let bytes = self.check(slot, width, height)?;
        Ok(unsafe { std::slice::from_raw_parts_mut(self.slot_ptr(slot), bytes) })
    }

    fn slot_ptr(&self, slot: u32) -> *mut u8 {
        // SAFETY: callers check `slot < self.slots`, and the region holds every slot.
        unsafe {
            self.base
                .add(HEADER_BYTES + slot as usize * self.slot_bytes)
        }
    }
}

impl Drop for SharedFrames {
    fn drop(&mut self) {
        self.os.unmap(self.base, self.len);
    }
}

fn round_up(value: usize, to: usize) -> usize {
    value.div_ceil(to) * to
}

#[cfg(unix)]
mod os {
    use anyhow::{Context as _, Result, bail};
    use std::ffi::CString;
    use std::os::unix::process::CommandExt as _;
    use std::process::Command;

    /// The descriptor number the helper finds the region on.
    const CHILD_FD: i32 = 3;

    pub struct Region {
        fd: i32,
    }

    impl Region {
        pub fn create(len: usize) -> Result<(Self, *mut u8)> {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.subsec_nanos())
                .unwrap_or(0);
            // macOS limits these names to 31 bytes.
            let name = CString::new(format!("/udx-spike-{}-{nanos:08x}", std::process::id()))?;
            // shm_open is variadic on macOS, so the mode is passed as an unsigned int there.
            #[cfg(target_os = "macos")]
            let mode = 0o600 as libc::c_uint;
            #[cfg(not(target_os = "macos"))]
            let mode = 0o600 as libc::mode_t;
            // SAFETY: plain libc calls on a valid C string; results are checked.
            let fd = unsafe {
                libc::shm_open(
                    name.as_ptr(),
                    libc::O_CREAT | libc::O_EXCL | libc::O_RDWR,
                    mode,
                )
            };
            if fd < 0 {
                bail!("shm_open: {}", std::io::Error::last_os_error());
            }
            // Remove the name at once: from here on only inheritance reaches the object.
            unsafe { libc::shm_unlink(name.as_ptr()) };
            if unsafe { libc::ftruncate(fd, len as libc::off_t) } != 0 {
                let error = std::io::Error::last_os_error();
                unsafe { libc::close(fd) };
                bail!("ftruncate: {error}");
            }
            let base = map(fd, len)?;
            Ok((Self { fd }, base))
        }

        pub fn share_with(&self, command: &mut Command, len: usize) {
            let fd = self.fd;
            command.args([
                "--shm-fd",
                &CHILD_FD.to_string(),
                "--shm-size",
                &len.to_string(),
            ]);
            // SAFETY: only async-signal-safe calls (dup2, fcntl) run between fork and exec.
            unsafe {
                command.pre_exec(move || {
                    if fd == CHILD_FD {
                        // dup2 onto itself would leave close-on-exec set.
                        if libc::fcntl(fd, libc::F_SETFD, 0) != 0 {
                            return Err(std::io::Error::last_os_error());
                        }
                    } else if libc::dup2(fd, CHILD_FD) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
        }

        pub fn open_inherited(args: &[String]) -> Result<(Self, *mut u8, usize)> {
            let fd: i32 = super::arg(args, "--shm-fd")?.parse().context("--shm-fd")?;
            let len: usize = super::arg(args, "--shm-size")?
                .parse()
                .context("--shm-size")?;
            let base = map(fd, len)?;
            Ok((Self { fd }, base, len))
        }

        pub fn unmap(&self, base: *mut u8, len: usize) {
            unsafe {
                libc::munmap(base.cast(), len);
                libc::close(self.fd);
            }
        }
    }

    fn map(fd: i32, len: usize) -> Result<*mut u8> {
        // SAFETY: a shared read-write mapping of an object at least `len` long.
        let base = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                fd,
                0,
            )
        };
        if base == libc::MAP_FAILED {
            bail!("mmap: {}", std::io::Error::last_os_error());
        }
        Ok(base.cast())
    }
}

#[cfg(windows)]
mod os {
    use anyhow::{Context as _, Result, bail};
    use std::process::Command;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_ALL_ACCESS, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
        PAGE_READWRITE, UnmapViewOfFile,
    };

    pub struct Region {
        handle: HANDLE,
    }

    impl Region {
        pub fn create(len: usize) -> Result<(Self, *mut u8)> {
            // Inheritable and unnamed: only a child started with handle inheritance gets it.
            let attributes = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: std::ptr::null_mut(),
                bInheritHandle: 1,
            };
            // SAFETY: a pagefile-backed mapping with no name; the result is checked.
            let handle = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    &attributes,
                    PAGE_READWRITE,
                    (len as u64 >> 32) as u32,
                    len as u32,
                    std::ptr::null(),
                )
            };
            if handle.is_null() {
                bail!("CreateFileMappingW: {}", std::io::Error::last_os_error());
            }
            let base = map(handle, len)?;
            Ok((Self { handle }, base))
        }

        pub fn share_with(&self, command: &mut Command, len: usize) {
            // The standard library starts children with handle inheritance on, so the
            // inheritable handle arrives with the same value.
            command.args([
                "--shm-handle",
                &(self.handle as usize).to_string(),
                "--shm-size",
                &len.to_string(),
            ]);
        }

        pub fn open_inherited(args: &[String]) -> Result<(Self, *mut u8, usize)> {
            let value: usize = super::arg(args, "--shm-handle")?
                .parse()
                .context("--shm-handle")?;
            let len: usize = super::arg(args, "--shm-size")?
                .parse()
                .context("--shm-size")?;
            let handle = value as HANDLE;
            let base = map(handle, len)?;
            Ok((Self { handle }, base, len))
        }

        pub fn unmap(&self, base: *mut u8, _len: usize) {
            unsafe {
                UnmapViewOfFile(MEMORY_MAPPED_VIEW_ADDRESS { Value: base.cast() });
                CloseHandle(self.handle);
            }
        }
    }

    fn map(handle: HANDLE, len: usize) -> Result<*mut u8> {
        // SAFETY: maps the whole mapping read-write; the result is checked.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, len) };
        if view.Value.is_null() {
            bail!("MapViewOfFile: {}", std::io::Error::last_os_error());
        }
        Ok(view.Value.cast())
    }
}

fn arg<'a>(args: &'a [String], name: &str) -> Result<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .with_context(|| format!("missing {name}"))
}
