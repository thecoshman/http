use std::os::unix::fs::{OpenOptionsExt, FileTypeExt};
use std::fs::{FileType, Metadata};
use std::os::fd::AsRawFd;
use std::os::raw::c_int;
use libc::O_NONBLOCK;
use std::path::Path;
use std::fs::OpenOptions;


extern "C" {
    fn http_blkgetsize(fd: c_int) -> u64;
}


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
    if meta.file_type().is_block_device() || meta.file_type().is_char_device() {
        if let Ok(f) = OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(path) {
            let size = unsafe { http_blkgetsize(f.as_raw_fd()) };
            if size != u64::MAX {
                return size;
            }
        }
    }

    meta.len()
}
