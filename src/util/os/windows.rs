use winapi::um::fileapi::{GetFileAttributesW, SetFileTime};
use winapi::shared::minwindef::FILETIME;
use std::os::windows::io::AsRawHandle;
use std::os::windows::fs::MetadataExt;
use std::os::windows::ffi::OsStrExt;
use std::fs::{Metadata, File};
use std::path::Path;


/// Get windows-style attributes for the specified file
///
/// https://docs.microsoft.com/en-gb/windows/win32/fileio/file-attribute-constants
pub fn win32_file_attributes(_: &Metadata, path: &Path) -> u32 {
    let mut buf: Vec<_> = path.as_os_str().encode_wide().collect();
    buf.push(0);

    unsafe { GetFileAttributesW(buf.as_ptr()) }
}


/// `st_dev`-`st_ino`-`st_mtim`
pub fn file_etag(m: &Metadata) -> String {
    format!("{:x}-{}-{}",
            m.volume_serial_number().unwrap_or(0),
            m.file_index().unwrap_or(0),
            m.last_write_time())
}


/// Check if file is marked executable
#[inline(always)]
pub fn file_executable(_: &Metadata) -> bool {
    true
}

#[inline(always)]
pub fn set_executable(_: &Path, _: bool) {}


pub fn set_mtime(f: &Path, ms: u64) {
    set_times(f, Some(ms), None, None)
}

pub fn set_mtime_f(f: &File, ms: u64) {
    set_times_f(f, Some(ms), None, None)
}


const NO_FILETIME: FILETIME = FILETIME {
    dwLowDateTime: 0,
    dwHighDateTime: 0,
};

pub fn set_times_f(f: &File, mtime_ms: Option<u64>, atime_ms: Option<u64>, ctime_ms: Option<u64>) {
    if mtime_ms.is_some() || atime_ms.is_some() || ctime_ms.is_some() {
        unsafe {
            SetFileTime(f.as_raw_handle() as _,
                        &ctime_ms.map(ms_to_FILETIME).unwrap_or(NO_FILETIME),
                        &atime_ms.map(ms_to_FILETIME).unwrap_or(NO_FILETIME),
                        &mtime_ms.map(ms_to_FILETIME).unwrap_or(NO_FILETIME));
        }
    }
}

pub fn set_times(f: &Path, mtime_ms: Option<u64>, atime_ms: Option<u64>, ctime_ms: Option<u64>) {
    if mtime_ms.is_some() || atime_ms.is_some() || ctime_ms.is_some() {
        if let Ok(f) = File::options().write(true).open(f) {
            set_times_f(&f, mtime_ms, atime_ms, ctime_ms);
        }
    }
}

/// FILETIME is in increments of 100ns, and in the Win32 epoch
#[allow(non_snake_case)]
fn ms_to_FILETIME(ms: u64) -> FILETIME {
    let ft = (ms * 1000_0) + 116444736000000000;
    FILETIME {
        dwLowDateTime: (ft & 0xFFFFFFFF) as u32,
        dwHighDateTime: (ft >> 32) as u32,
    }
}
