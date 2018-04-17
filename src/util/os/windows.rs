use std::fs::FileType;


/// OS-specific check for fileness
pub fn is_device(_: &FileType) -> bool {
    false
}
