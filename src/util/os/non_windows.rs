use libc::{AT_SYMLINK_NOFOLLOW, UTIME_OMIT, AT_FDCWD, utimensat, timespec};
use std::os::unix::fs::{PermissionsExt, MetadataExt};
use self::super::super::is_actually_file;
use os_str_generic::OsStrGenericExt;
use std::os::unix::ffi::OsStrExt;
use std::fs::Metadata;
use std::path::Path;


const FILE_ATTRIBUTE_READONLY: u32 = 0x01;
const FILE_ATTRIBUTE_HIDDEN: u32 = 0x02;
const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x10;
const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;


/// Get windows-style attributes for the specified file
///
/// https://docs.microsoft.com/en-gb/windows/win32/fileio/file-attribute-constants
pub fn win32_file_attributes(meta: &Metadata, path: &Path) -> u32 {
    let mut attr = 0;

    if meta.permissions().readonly() {
        attr |= FILE_ATTRIBUTE_READONLY;
    }

    if path.file_name().map(|n| n.starts_with(".")).unwrap_or(false) {
        attr |= FILE_ATTRIBUTE_HIDDEN;
    }

    if !is_actually_file(&meta.file_type(), &path) {
        attr |= FILE_ATTRIBUTE_DIRECTORY;
    } else {
        // this is the 'Archive' bit, which is set by
        // default on _all_ files on creation and on
        // modification.
        attr |= FILE_ATTRIBUTE_ARCHIVE;
    }

    attr
}


/// `st_dev`-`st_ino`-`st_mtime`
pub fn file_etag(m: &Metadata) -> String {
    format!("{:x}-{}-{}.{}", m.dev(), m.ino(), m.mtime(), m.mtime_nsec())
}


/// Check if file is marked executable
pub fn file_executable(meta: &Metadata) -> bool {
    (meta.permissions().mode() & 0o111) != 0
}


pub fn set_mtime(f: &Path, ms: u64) {
    unsafe {
        utimensat(AT_FDCWD,
                  f.as_os_str().as_bytes().as_ptr() as *const _,
                  [timespec {
                       tv_sec: 0,
                       tv_nsec: UTIME_OMIT,
                   },
                   timespec {
                       tv_sec: (ms / 1000) as i64,
                       tv_nsec: ((ms % 1000) * 1000_000) as i64,
                   }]
                      .as_ptr(),
                  AT_SYMLINK_NOFOLLOW);
    }
}
