use std::os::unix::fs::{PermissionsExt, FileTypeExt};
use libc::{O_RDONLY, c_ulong, close, ioctl, open};
use std::os::unix::ffi::OsStrExt;
use std::fs::{FileType, Metadata};
use std::ffi::CString;
use std::path::Path;


include!(concat!(env!("OUT_DIR"), "/ioctl-data/ioctl.rs"));


// Stolen from https://unix.superglobalmegacorp.com/Net2/newsrc/sys/stat.h.html
/// X for owner
const S_IXUSR: u32 = 0o000100;
/// X for group
const S_IXGRP: u32 = 0o000010;
/// X for other
const S_IXOTH: u32 = 0o000001;


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
    if is_device(&meta.file_type()) {
        let mut block_count: c_ulong = 0;

        let path_c = CString::new(path.as_os_str().as_bytes()).unwrap();
        let dev_file = unsafe { open(path_c.as_ptr(), O_RDONLY) };
        if dev_file >= 0 {
            let ok = unsafe { ioctl(dev_file, BLKGETSIZE, &mut block_count as *mut c_ulong) } == 0;
            unsafe { close(dev_file) };

            if ok {
                return block_count as u64 * 512;
            }
        }
    }

    meta.len()
}

/// Check if file is marked executable
pub fn file_executable(meta: &Metadata) -> bool {
    (meta.permissions().mode() & (S_IXUSR | S_IXGRP | S_IXOTH)) != 0
}
