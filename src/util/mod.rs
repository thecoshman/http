//! Module containing various utility functions.


mod os;
mod content_encoding;

use base64;
use std::path::Path;
use percent_encoding;
use std::borrow::Cow;
use rfsapi::RawFileData;
use std::{cmp, f64, fmt};
use iron::headers::UserAgent;
use std::collections::HashMap;
use time::{self, Duration, Tm};
use iron::{mime, Headers, Url};
use std::io::{BufReader, BufRead};
use base64::display::Base64Display;
use std::fs::{self, FileType, File};
use iron::headers::{HeaderFormat, Header};
use iron::error::HttpResult as HyperResult;
use mime_guess::{guess_mime_type_opt, get_mime_type_str};

pub use self::os::*;
pub use self::content_encoding::*;


/// The generic HTML page to use as response to errors.
pub static ERROR_HTML: &'static str = include_str!("../../assets/error.html");

/// The HTML page to use as template for a requested directory's listing.
pub static DIRECTORY_LISTING_HTML: &'static str = include_str!("../../assets/directory_listing.html");

/// The HTML page to use as template for a requested directory's listing for mobile devices.
pub static MOBILE_DIRECTORY_LISTING_HTML: &'static str = include_str!("../../assets/directory_listing_mobile.html");

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
                               Base64Display::with_config(&include_bytes!("../../assets/icons/directory_icon.gif")[..], base64::STANDARD))));
        ass.insert("file_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_icon.gif")[..], base64::STANDARD))));
        ass.insert("file_binary_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_binary_icon.gif")[..], base64::STANDARD))));
        ass.insert("file_image_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_image_icon.gif")[..], base64::STANDARD))));
        ass.insert("file_text_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/file_text_icon.gif")[..], base64::STANDARD))));
        ass.insert("back_arrow_icon",
            Cow::Owned(format!("data:{};base64,{}",
                               get_mime_type_str("gif").unwrap(),
                               Base64Display::with_config(&include_bytes!("../../assets/icons/back_arrow_icon.gif")[..], base64::STANDARD))));
        ass.insert("date", Cow::Borrowed(include_str!("../../assets/date.js")));
        ass.insert("upload", Cow::Borrowed(include_str!("../../assets/upload.js")));
        ass.insert("adjust_tz", Cow::Borrowed(include_str!("../../assets/adjust_tz.js")));
        ass
    };
}

/// The port to start scanning from if no ports were given.
pub static PORT_SCAN_LOWEST: u16 = 8000;

/// The port to end scanning at if no ports were given.
pub static PORT_SCAN_HIGHEST: u16 = 9999;

/// The app name and version to use with User-Agent or Server response header.
pub static USER_AGENT: &'static str = concat!("http/", env!("CARGO_PKG_VERSION"));

/// Index file extensions to look for if `-i` was not specified.
pub static INDEX_EXTENSIONS: &'static [&'static str] = &["html", "htm", "shtml"];


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
    let path = path.as_ref();
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
pub fn file_time_modified(f: &Path) -> Tm {
    match f.metadata().expect("Failed to get file metadata").modified().expect("Failed to get file last modified date").elapsed() {
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
pub fn is_actually_file(tp: &FileType) -> bool {
    tp.is_file() || is_device(tp)
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
pub fn is_nonexistant_descendant_of<Pw: AsRef<Path>, Po: AsRef<Path>>(who: Pw, of_whom: Po) -> bool {
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
    let f = f.as_ref();
    RawFileData {
        mime_type: guess_mime_type_opt(f).unwrap_or_else(|| if file_binary(f) {
            "application/octet-stream".parse().unwrap()
        } else {
            "text/plain".parse().unwrap()
        }),
        name: f.file_name().unwrap().to_str().expect("Failed to get requested file name").to_string(),
        last_modified: file_time_modified(f),
        size: f.metadata().expect("Failed to get requested file metadata").len(),
        is_file: true,
    }
}
