//! Module containing various utility functions.


mod os;
mod webdav;
mod content_encoding;

use base64;
use std::path::Path;
use percent_encoding;
use walkdir::WalkDir;
use std::borrow::Cow;
use rfsapi::RawFileData;
use std::{cmp, f64, str};
use std::time::SystemTime;
use std::collections::HashMap;
use time::{self, Duration, Tm};
use iron::{mime, Headers, Url};
use base64::display::Base64Display;
use std::fmt::{self, Write as FmtWrite};
use iron::error::HttpResult as HyperResult;
use std::fs::{self, FileType, Metadata, File};
use iron::headers::{HeaderFormat, UserAgent, Header};
use mime_guess::{guess_mime_type_opt, get_mime_type_str};
use xml::name::{OwnedName as OwnedXmlName, Name as XmlName};
use std::io::{ErrorKind as IoErrorKind, BufReader, BufRead, Result as IoResult, Error as IoError};

pub use self::os::*;
pub use self::webdav::*;
pub use self::content_encoding::*;


/// The generic HTML page to use as response to errors.
pub const ERROR_HTML: &str = include_str!("../../assets/error.html");

/// The HTML page to use as template for a requested directory's listing.
pub const DIRECTORY_LISTING_HTML: &str = include_str!("../../assets/directory_listing.html");

/// The HTML page to use as template for a requested directory's listing for mobile devices.
pub const MOBILE_DIRECTORY_LISTING_HTML: &str = include_str!("../../assets/directory_listing_mobile.html");

lazy_static! {
    /// Collection of data to be injected into generated responses.
    pub static ref ASSETS: HashMap<&'static str, Cow<'static, str>> = {
        let mut ass = HashMap::with_capacity(10);
        ass.insert("favicon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("ico").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/favicon.ico")[..], base64::STANDARD))));
        ass.insert("dir_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/directory.gif")[..], base64::STANDARD))));
        ass.insert("file_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file.gif")[..], base64::STANDARD))));
        ass.insert("file_binary_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_binary.gif")[..], base64::STANDARD))));
        ass.insert("file_image_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_image.gif")[..], base64::STANDARD))));
        ass.insert("file_text_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_text.gif")[..], base64::STANDARD))));
        ass.insert("back_arrow_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/back_arrow.gif")[..], base64::STANDARD))));
        ass.insert("new_dir_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/new_directory.gif")[..], base64::STANDARD))));
        ass.insert("delete_file_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("png").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/delete_file.png")[..], base64::STANDARD))));
        ass.insert("rename_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("png").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/rename.png")[..], base64::STANDARD))));
        ass.insert("confirm_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("png").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/confirm.png")[..], base64::STANDARD))));
        ass.insert("date", Cow::Borrowed(include_str!("../../assets/date.js")));
        ass.insert("manage", Cow::Borrowed(include_str!("../../assets/manage.js")));
        ass.insert("manage_mobile", Cow::Borrowed(include_str!("../../assets/manage_mobile.js")));
        ass.insert("manage_desktop", Cow::Borrowed(include_str!("../../assets/manage_desktop.js")));
        ass.insert("upload", Cow::Borrowed(include_str!("../../assets/upload.js")));
        ass.insert("adjust_tz", Cow::Borrowed(include_str!("../../assets/adjust_tz.js")));
        ass
    };
}

/// The port to start scanning from if no ports were given.
pub const PORT_SCAN_LOWEST: u16 = 8000;

/// The port to end scanning at if no ports were given.
pub const PORT_SCAN_HIGHEST: u16 = 9999;

/// The app name and version to use with User-Agent or Server response header.
pub const USER_AGENT: &str = concat!("http/", env!("CARGO_PKG_VERSION"));

/// Index file extensions to look for if `-i` was not specified and strippable extensions to look for if `-x` was specified.
pub const INDEX_EXTENSIONS: &[&str] = &["html", "htm", "shtml"];


/// The [WWW-Authenticate header](https://tools.ietf.org/html/rfc7235#section-4.1), without parsing.
///
/// We don't ever receive this header, only ever send it, so this is fine.
#[derive(Debug, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct WwwAuthenticate(pub Cow<'static, str>);

impl Header for WwwAuthenticate {
    fn header_name() -> &'static str {
        "WWW-Authenticate"
    }

    /// Dummy impl returning an empty value, since we're only ever sending these
    fn parse_header(_: &[Vec<u8>]) -> HyperResult<WwwAuthenticate> {
        Ok(WwwAuthenticate("".into()))
    }
}

impl HeaderFormat for WwwAuthenticate {
    fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct CommaList<D: fmt::Display, I: Iterator<Item = D>>(pub I);

impl<D: fmt::Display, I: Iterator<Item = D> + Clone> fmt::Display for CommaList<D, I> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut itr = self.0.clone();
        if let Some(item) = itr.next() {
            item.fmt(f)?;

            for item in itr {
                f.write_str(", ")?;
                item.fmt(f)?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct DisplayThree<Df: fmt::Display, Ds: fmt::Display, Dt: fmt::Display>(pub Df, pub Ds, pub Dt);

impl<Df: fmt::Display, Ds: fmt::Display, Dt: fmt::Display> fmt::Display for DisplayThree<Df, Ds, Dt> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)?;
        self.1.fmt(f)?;
        self.2.fmt(f)?;
        Ok(())
    }
}


/// `xml`'s `OwnedName::borrow()` returns a value not a reference, so it cannot be used with the libstd `Borrow` trait
pub trait BorrowXmlName<'n> {
    fn borrow_xml_name(&'n self) -> XmlName<'n>;
}

impl<'n> BorrowXmlName<'n> for XmlName<'n> {
    #[inline(always)]
    fn borrow_xml_name(&'n self) -> XmlName<'n> {
        *self
    }
}

impl<'n> BorrowXmlName<'n> for OwnedXmlName {
    #[inline(always)]
    fn borrow_xml_name(&'n self) -> XmlName<'n> {
        self.borrow()
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialOrd, Ord, PartialEq, Eq)]
pub struct Spaces(pub usize);

impl fmt::Display for Spaces {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for _ in 0..self.0 {
            f.write_char(' ')?;
        }
        Ok(())
    }
}



/// Uppercase the first character of the supplied string.
///
/// Based on http://stackoverflow.com/a/38406885/2851815
///
/// # Examples
///
/// ```
/// # use https::util::uppercase_first;
/// assert_eq!(uppercase_first("abolish"), "Abolish".to_string());
/// ```
pub fn uppercase_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

/// Percent-encode the last character if it's white space
///
/// Firefox treats, e.g. `href="http://henlo/menlo   "` as `href="http://henlo/menlo"`,
/// but that final whitespace is significant, so this turns it into `href="http://henlo/menlo  %20"`
pub fn encode_tail_if_trimmed(mut s: String) -> String {
    let c = s.chars().rev().next();
    if c.map(|c| c.is_whitespace()).unwrap_or(false) {
        let c = c.unwrap();

        s.pop();
        s.push('%');

        let mut cb = [0u8; 4];
        c.encode_utf8(&mut cb);
        for b in cb.iter().take(c.len_utf8()) {
            write!(s, "{:02X}", b).expect("Couldn't allocate two more characters?");
        }

        s
    } else {
        s
    }
}

/// %-escape special characters in an URL
pub fn escape_specials<S: AsRef<str>>(s: S) -> String {
    let s = s.as_ref();
    let mut ret = Vec::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'%' => ret.extend(b"%25"),
            b'#' => ret.extend(b"%23"),
            b'?' => ret.extend(b"%3F"),
            b'[' => ret.extend(b"%5B"),
            b']' => ret.extend(b"%5D"),
            _ => ret.push(b),
        }
    }
    unsafe { String::from_utf8_unchecked(ret) }
}

/// Check if the specified file is to be considered "binary".
///
/// Basically checks is a file is UTF-8.
///
/// # Examples
///
/// ```
/// # use https::util::file_binary;
/// # #[cfg(target_os = "windows")]
/// # assert!(file_binary("target/debug/http.exe"));
/// # #[cfg(not(target_os = "windows"))]
/// assert!(file_binary("target/debug/http"));
/// assert!(!file_binary("Cargo.toml"));
/// ```
pub fn file_binary<P: AsRef<Path>>(path: P) -> bool {
    file_binary_impl(path.as_ref())
}

fn file_binary_impl(path: &Path) -> bool {
    path.metadata()
        .map(|m| is_device(&m.file_type()) || File::open(path).and_then(|f| BufReader::new(f).read_line(&mut String::new())).is_err())
        .unwrap_or(true)
}

/// Fill out an HTML template.
///
/// All fields must be addressed even if formatted to be empty.
///
/// # Examples
///
/// ```
/// # use https::util::{html_response, NOT_IMPLEMENTED_HTML};
/// println!(html_response(NOT_IMPLEMENTED_HTML, &["<p>Abolish the burgeoisie!</p>"]));
/// ```
pub fn html_response<S: AsRef<str>>(data: &str, format_strings: &[S]) -> String {
    ASSETS.iter().fold(format_strings.iter().enumerate().fold(data.to_string(), |d, (i, s)| d.replace(&format!("{{{}}}", i), s.as_ref())),
                       |d, (k, v)| d.replace(&format!("{{{}}}", k), v))
}

/// Return the path part of the URL.
///
/// # Example
///
/// ```
/// # extern crate iron;
/// # extern crate https;
/// # use iron::Url;
/// # use https::util::url_path;
/// let url = Url::parse("127.0.0.1:8000/capitalism/русский/");
/// assert_eq!(url_path(&url), "capitalism/русский/");
/// ```
pub fn url_path(url: &Url) -> String {
    let path = url.path();
    if path == [""] {
        "/".to_string()
    } else {
        path.into_iter().fold("".to_string(),
                              |cur, pp| format!("{}/{}", cur, percent_decode(pp).unwrap_or(Cow::Borrowed("<incorrect UTF8>"))))
            [1..]
            .to_string()
    }
}

/// Decode a percent-encoded string (like a part of a URL).
///
/// # Example
///
/// ```
/// # use https::util::percent_decode;
/// # use std::borrow::Cow;
/// assert_eq!(percent_decode("%D0%B0%D1%81%D0%B4%D1%84%20fdsa"), Some(Cow::Owned("асдф fdsa".to_string())));
/// assert_eq!(percent_decode("%D0%D1%81%D0%B4%D1%84%20fdsa"), None);
/// ```
pub fn percent_decode(s: &str) -> Option<Cow<str>> {
    percent_encoding::percent_decode(s.as_bytes()).decode_utf8().ok()
}

/// Get the timestamp of the file's last modification as a `time::Tm` in UTC.
pub fn file_time_modified_p(f: &Path) -> Tm {
    file_time_modified(&f.metadata().expect("Failed to get file metadata"))
}

/// Get the timestamp of the file's last modification as a `time::Tm` in UTC.
pub fn file_time_created_p(f: &Path) -> Tm {
    file_time_created(&f.metadata().expect("Failed to get file metadata"))
}

/// Get the timestamp of the file's last access as a `time::Tm` in UTC.
pub fn file_time_accessed_p(f: &Path) -> Tm {
    file_time_accessed(&f.metadata().expect("Failed to get file metadata"))
}

/// Get the timestamp of the file's last modification as a `time::Tm` in UTC.
pub fn file_time_modified(m: &Metadata) -> Tm {
    file_time_impl(m.modified().expect("Failed to get file last modified date"))
}

/// Get the timestamp of the file's last modification as a `time::Tm` in UTC.
pub fn file_time_created(m: &Metadata) -> Tm {
    file_time_impl(m.created().or_else(|_| m.modified()).expect("Failed to get file created date"))
}

/// Get the timestamp of the file's last access as a `time::Tm` in UTC.
pub fn file_time_accessed(m: &Metadata) -> Tm {
    file_time_impl(m.accessed().expect("Failed to get file accessed date"))
}

fn file_time_impl(time: SystemTime) -> Tm {
    match time.elapsed() {
        Ok(dur) => time::now_utc() - Duration::from_std(dur).unwrap(),
        Err(ste) => time::now_utc() + Duration::from_std(ste.duration()).unwrap(),
    }
}

/// Check, whether, in any place of the path, a file is treated like a directory.
///
/// A file is treated like a directory when it is treated as if it had a subpath, e.g., given:
///
/// ```sh
/// tree .
/// | dir0
/// | dir1
///   | file01
/// ```
///
/// This function would return true for `./dir1/file01/file010`, `./dir1/file01/dir010/file0100`, etc., but not
/// for `./dir0/file00`, `./dir0/dir00/file000`, `./dir1/file02/`, `./dir1/dir010/file0100`.
pub fn detect_file_as_dir(mut p: &Path) -> bool {
    while let Some(pnt) = p.parent() {
        if pnt.is_file() {
            return true;
        }

        p = pnt;
    }

    false
}

/// Check if a path refers to a symlink in a way that also works on Windows.
pub fn is_symlink<P: AsRef<Path>>(p: P) -> bool {
    p.as_ref().read_link().is_ok()
}

/// Check if a path refers to a file in a way that includes Unix devices and Windows symlinks.
pub fn is_actually_file<P: AsRef<Path>>(tp: &FileType, p: P) -> bool {
    tp.is_file() || (tp.is_symlink() && fs::metadata(p).map(|m| is_actually_file(&m.file_type(), "")).unwrap_or(false)) || is_device(tp)
}

/// Check if the specified path is a direct descendant (or an equal) of the specified path.
pub fn is_descendant_of<Pw: AsRef<Path>, Po: AsRef<Path>>(who: Pw, of_whom: Po) -> bool {
    let (mut who, of_whom) = if let Ok(p) = fs::canonicalize(who).and_then(|w| fs::canonicalize(of_whom).map(|o| (w, o))) {
        p
    } else {
        return false;
    };

    if who == of_whom {
        return true;
    }

    while let Some(who_p) = who.parent().map(|p| p.to_path_buf()) {
        who = who_p;

        if who == of_whom {
            return true;
        }
    }

    false
}

/// Check if the specified path is a direct descendant (or an equal) of the specified path, without without requiring it to
/// exist in the first place.
pub fn is_nonexistent_descendant_of<Pw: AsRef<Path>, Po: AsRef<Path>>(who: Pw, of_whom: Po) -> bool {
    let mut who = fs::canonicalize(&who).unwrap_or_else(|_| who.as_ref().to_path_buf());
    let of_whom = if let Ok(p) = fs::canonicalize(of_whom) {
        p
    } else {
        return false;
    };

    if who == of_whom {
        return true;
    }

    while let Some(who_p) = who.parent().map(|p| p.to_path_buf()) {
        who = if let Ok(p) = fs::canonicalize(&who_p) {
            p
        } else {
            who_p
        };

        if who == of_whom {
            return true;
        }
    }

    false
}

/// Construct string representing a human-readable size.
///
/// Stolen, adapted and inlined from [fielsize.js](http://filesizejs.com).
pub fn human_readable_size(s: u64) -> String {
    lazy_static! {
        static ref LN_KIB: f64 = 1024f64.log(f64::consts::E);
    }

    if s == 0 {
        "0 B".to_string()
    } else {
        let num = s as f64;
        let exp = cmp::min(cmp::max((num.log(f64::consts::E) / *LN_KIB) as i32, 0), 8);

        let val = num / 2f64.powi(exp * 10);

        if exp > 0 {
                (val * 10f64).round() / 10f64
            } else {
                val.round()
            }
            .to_string() + " " + ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"][cmp::max(exp, 0) as usize]
    }
}

/// Check if, given the request headers, the client should be considered a mobile device.
pub fn client_mobile(hdr: &Headers) -> bool {
    hdr.get::<UserAgent>().map(|s| s.contains("Mobi") || s.contains("mobi")).unwrap_or(false)
}

/// Check if, given the request headers, the client should be treated as Microsoft software.
///
/// Based on https://github.com/miquels/webdav-handler-rs/blob/02433c1acfccd848a7de26889f6857cbad559076/src/handle_props.rs#L529
pub fn client_microsoft(hdr: &Headers) -> bool {
    hdr.get::<UserAgent>().map(|s| s.contains("Microsoft") || s.contains("microsoft")).unwrap_or(false)
}

/// Get the suffix for the icon to use to represent the given file.
pub fn file_icon_suffix<P: AsRef<Path>>(f: P, is_file: bool) -> &'static str {
    if is_file {
        match guess_mime_type_opt(&f) {
            Some(mime::Mime(mime::TopLevel::Image, ..)) |
            Some(mime::Mime(mime::TopLevel::Video, ..)) => "_image",
            Some(mime::Mime(mime::TopLevel::Text, ..)) => "_text",
            Some(mime::Mime(mime::TopLevel::Application, ..)) => "_binary",
            None => if file_binary(&f) { "" } else { "_text" },
            _ => "",
        }
    } else {
        ""
    }
}

/// Get the metadata of the specified file.
///
/// The specified path must point to a file.
pub fn get_raw_fs_metadata<P: AsRef<Path>>(f: P) -> RawFileData {
    get_raw_fs_metadata_impl(f.as_ref())
}

fn get_raw_fs_metadata_impl(f: &Path) -> RawFileData {
    let meta = f.metadata().expect("Failed to get requested file metadata");
    RawFileData {
        mime_type: guess_mime_type_opt(f).unwrap_or_else(|| if file_binary(f) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        }),
        name: f.file_name().unwrap().to_str().expect("Failed to get requested file name").to_string(),
        last_modified: file_time_modified(&meta),
        size: file_length(&meta, &f),
        is_file: true,
    }
}

/// Recursively copy a directory
///
/// Stolen from https://github.com/mdunsmuir/copy_dir/blob/0.1.2/src/lib.rs
pub fn copy_dir(from: &Path, to: &Path) -> IoResult<Vec<(IoError, String)>> {
    macro_rules! push_error {
        ($vec:ident, $path:ident, $expr:expr) => {
            match $expr {
                Ok(_) => (),
                Err(e) => $vec.push((e, $path.to_string_lossy().into_owned())),
            }
        };
    }

    let mut errors = Vec::new();

    fs::create_dir(&to)?;

    // The approach taken by this code (i.e. walkdir) will not gracefully
    // handle copying a directory into itself, so we're going to simply
    // disallow it by checking the paths. This is a thornier problem than I
    // wish it was, and I'd like to find a better solution, but for now I
    // would prefer to return an error rather than having the copy blow up
    // in users' faces. Ultimately I think a solution to this will involve
    // not using walkdir at all, and might come along with better handling
    // of hard links.
    if from.canonicalize().and_then(|fc| to.canonicalize().map(|tc| (fc, tc))).map(|(fc, tc)| tc.starts_with(fc))? {
        fs::remove_dir(&to)?;

        return Err(IoError::new(IoErrorKind::Other, "cannot copy to a path prefixed by the source path"));
    }

    for entry in WalkDir::new(&from).min_depth(1).into_iter().flatten() {
        let source_metadata = match entry.metadata() {
            Ok(md) => md,
            Err(err) => {
                errors.push((err.into(), entry.path().to_string_lossy().into_owned()));
                continue;
            }
        };

        let relative_path = entry.path().strip_prefix(&from).expect("strip_prefix failed; this is a probably a bug in copy_dir");

        let target_path = to.join(relative_path);

        if !is_actually_file(&source_metadata.file_type(), entry.path()) {
            push_error!(errors, relative_path, fs::create_dir(&target_path));
            push_error!(errors, relative_path, fs::set_permissions(&target_path, source_metadata.permissions()));
        } else {
            push_error!(errors, relative_path, fs::copy(entry.path(), &target_path));
        }
    }

    Ok(errors)
}
