use std::fs::{FileType, Metadata};
use std::path::Path;


/// OS-specific check for fileness
pub fn is_device(_: &FileType) -> bool {
    false
}

/// Check file length responsibly
pub fn file_length<P: AsRef<Path>>(meta: &Metadata, _: &P) -> u64 {
    meta.len()
}
