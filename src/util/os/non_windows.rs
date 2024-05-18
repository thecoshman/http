use libc::{AT_SYMLINK_NOFOLLOW, UTIME_OMIT, AT_FDCWD, utimensat, timespec, umask};
use std::os::unix::fs::{PermissionsExt, MetadataExt};
use self::super::super::is_actually_file;
use std::os::unix::ffi::OsStrExt;
use std::fs::{self, Metadata};
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

    if path.file_name().map(|n| n.as_bytes().starts_with(b".")).unwrap_or(false) {
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


static mut UMASK: u32 = 0;

// as seen in https://docs.rs/ctor/latest/ctor/attr.ctor.html
#[used]
#[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".init_array")]
#[cfg_attr(target_os = "freebsd", link_section = ".init_array")]
#[cfg_attr(target_os = "netbsd", link_section = ".init_array")]
#[cfg_attr(target_os = "openbsd", link_section = ".init_array")]
#[cfg_attr(target_os = "illumos", link_section = ".init_array")]
#[cfg_attr(any(target_os = "macos", target_os = "ios", target_os = "tvos"), link_section = "__DATA_CONST,__mod_init_func")]
#[cfg_attr(target_os = "windows", link_section = ".CRT$XCU")]
static LOAD_UMASK: unsafe extern "C" fn() = {
    #[cfg_attr(any(target_os = "linux", target_os = "android"), link_section = ".text.startup")]
    unsafe extern "C" fn load_umask() {
        UMASK = umask(0o777);
        umask(UMASK);
    }
    load_umask
};

pub fn set_executable(f: &Path, ex: bool) {
    let mut perm = match fs::metadata(f) {
        Ok(meta) => meta.permissions(),
        Err(_) => return,
    };
    if ex {
        perm.set_mode(perm.mode() | (0o111 & unsafe { !UMASK }));
    } else {
        perm.set_mode(perm.mode() & !0o111);
    }
    let _ = fs::set_permissions(f, perm);
}


const NO_TIMESPEC: timespec = timespec {
    tv_sec: 0,
    tv_nsec: UTIME_OMIT,
};

pub fn set_mtime(f: &Path, ms: u64) {
    set_times(f, Some(ms), None, None)
}

pub fn set_times(f: &Path, mtime_ms: Option<u64>, atime_ms: Option<u64>, _: Option<u64>) {
    if mtime_ms.is_some() || atime_ms.is_some() {
        unsafe {
            utimensat(AT_FDCWD,
                      f.as_os_str().as_bytes().as_ptr() as *const _,
                      [atime_ms.map(ms_to_timespec).unwrap_or(NO_TIMESPEC), mtime_ms.map(ms_to_timespec).unwrap_or(NO_TIMESPEC)].as_ptr(),
                      AT_SYMLINK_NOFOLLOW);
        }
    }
}

fn ms_to_timespec(ms: u64) -> timespec {
    timespec {
        tv_sec: (ms / 1000) as i64,
        tv_nsec: ((ms % 1000) * 1000_000) as i64,
    }
}
