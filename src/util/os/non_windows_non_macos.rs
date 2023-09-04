use std::os::unix::fs::{PermissionsExt, FileTypeExt};
use libc::{O_CLOEXEC, O_RDONLY, close, ioctl, open};
use std::os::unix::ffi::OsStrExt;
use std::fs::{FileType, Metadata};
use std::ffi::CString;
use std::path::Path;


include!(concat!(env!("OUT_DIR"), "/ioctl-data/ioctl.rs"));


/// OS-specific check for fileness
pub fn is_device(tp: &FileType) -> bool {
    tp.is_block_device() || tp.is_char_device() || tp.is_fifo() || tp.is_socket()
}

/// Check file length responsibly
#[inline(always)]
pub fn file_length<P: AsRef<Path>>(meta: &Metadata, path: &P) -> u64 {
    file_length_impl(meta, path.as_ref())
}

fn file_length_impl(meta: &Metadata, path: &Path) -> u64 {
    if meta.file_type().is_block_device() {
        let path_c = CString::new(path.as_os_str().as_bytes()).unwrap();
        let dev_file = unsafe { open(path_c.as_ptr(), O_RDONLY | O_CLOEXEC) };
        if dev_file != -1 {
            let mut size: u64 = 0;
            let ok = unsafe { ioctl(dev_file, BLKGETSIZE64 as _, &mut size as *mut _) } == 0;
            unsafe { close(dev_file) };

            if ok {
                return size;
            }
        }
    }

    meta.len()
}

/// Check if file is marked executable
pub fn file_executable(meta: &Metadata) -> bool {
    (meta.permissions().mode() & 0o111) != 0
}
