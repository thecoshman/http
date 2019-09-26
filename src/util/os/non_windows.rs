use self::super::super::is_actually_file;
use os_str_generic::OsStrGenericExt;
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

    if !is_actually_file(&meta.file_type()) {
        attr |= FILE_ATTRIBUTE_DIRECTORY;
    } else {
        // this is the 'Archive' bit, which is set by
        // default on _all_ files on creation and on
        // modification.
        attr |= FILE_ATTRIBUTE_ARCHIVE;
    }

    attr
}
