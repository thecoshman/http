use std::fs::{FileType, Metadata};
use std::path::Path;


/// OS-specific check for fileness
#[inline(always)]
pub fn is_device(_: &FileType) -> bool {
    false
}

/// Check file length responsibly
#[inline(always)]
pub fn file_length<P: AsRef<Path>>(meta: &Metadata, _: &P) -> u64 {
    meta.len()
}

/// Check if file is marked executable
#[inline(always)]
pub fn file_executable(_: &Metadata) -> bool {
    true
}
